//! The simulation's store and the systems that run over it.
//!
//! The register is a `bevy_ecs::World`: every thing that exists is an `Entity` with
//! components, reached by id and never by pointer. What a square holds is found through a
//! grid resource rebuilt from the World, never kept as a second authority.

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::shadowcasting::FourQuadrants;
use std::collections::BTreeMap;

/// Where a thing stands. The only authoritative position, because it lives on the thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Position {
    /// The square's column.
    pub x: i32,
    /// The square's row.
    pub y: i32,
}

/// The derived index: which things stand on each square.
///
/// Reference: D2. Rebuilt from the World, never edited directly and never authoritative: if it
/// disagrees with the World, it is wrong.
#[derive(Debug, Default, Resource)]
pub struct PositionIndex {
    squares: BTreeMap<(i32, i32), Vec<Entity>>,
}

impl PositionIndex {
    /// Rebuilds the index from the World's `Position` components.
    pub fn rebuild(world: &mut World) {
        let mut squares: BTreeMap<(i32, i32), Vec<Entity>> = BTreeMap::new();
        let mut query = world.query::<(Entity, &Position)>();
        for (entity, position) in query.iter(world) {
            squares
                .entry((position.x, position.y))
                .or_default()
                .push(entity);
        }
        world.insert_resource(Self { squares });
    }

    /// Everything standing on a square, in spawn order.
    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> &[Entity] {
        self.squares.get(&(x, y)).map_or(&[], Vec::as_slice)
    }

    /// How many squares hold anything at all.
    #[must_use]
    pub fn occupied_squares(&self) -> usize {
        self.squares.len()
    }
}

/// A patch of something lying on a square: smoke, fire, blood, acid.
///
/// A field is a thing, not a property of the ground: it has a square, a strength and an age,
/// and the field system acts on it every turn. The entry keeps the reference's decay rules
/// (`src/field.cpp`), which were ported and gated before the storage moved here.
#[derive(Debug, Clone, Component)]
pub struct Field {
    /// Which kind of field this is.
    pub id: FieldId,
    /// Its strength and age, with the reference's decay semantics.
    pub entry: FieldEntry,
    /// Draw and merge priority among the fields on one square.
    pub priority: i32,
}

/// Ages every field by one turn and removes the ones that are gone.
///
/// Reference: `map::process_fields_in_submap` (`src/map_field.cpp:160-460`), whose per-field
/// work lives in [`cdda_core_types::field::FieldEntry::do_decay`]. Fields are processed in a
/// fixed order - the reference walks submaps, then squares in x-then-y order, then each
/// square's fields in insertion order - and that order is observable, because a decaying field
/// draws from the same dice as everything else.
pub fn process_fields(world: &mut World, now: i64, rolls: &mut cdda_core_types::rng::Rolls) {
    let mut order: Vec<(i32, i32, Entity)> = Vec::new();
    let mut query = world.query::<(Entity, &Position)>();
    for (entity, position) in query.iter(world) {
        order.push((position.x, position.y, entity));
    }
    // x outer, y inner: the reference's walk over a submap.
    order.sort_by_key(|&(x, y, entity)| (x, y, entity.to_bits()));
    for (_, _, entity) in order {
        let Some(mut field) = world.get_mut::<Field>(entity) else {
            continue;
        };
        field.entry.do_decay(now, rolls);
        if !field.entry.is_alive() {
            world.despawn(entity);
        }
    }
}

/// A thing that gives off light: a torch, a fire, a lamp.
#[derive(Debug, Clone, Copy, Component)]
pub struct LightSource {
    /// How bright it is, in the reference's luminance units.
    pub luminance: f32,
}

/// The brightness of each square, in the reference's four quarters.
///
/// An answer, not a thing: written by the lighting system whenever something that casts light
/// has moved or died, and thrown away when it is stale.
#[derive(Debug, Default, Resource)]
pub struct Light {
    squares: BTreeMap<(i32, i32), FourQuadrants>,
}

impl Light {
    /// The quarters of a square, if anything lights it.
    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> Option<FourQuadrants> {
        self.squares.get(&(x, y)).copied()
    }

    /// How bright a square is at its brightest corner.
    #[must_use]
    pub fn max_at(&self, x: i32, y: i32) -> f32 {
        self.squares.get(&(x, y)).map_or(0.0, FourQuadrants::max)
    }

    /// How many squares anything lights.
    #[must_use]
    pub fn lit_squares(&self) -> usize {
        self.squares.len()
    }

    /// Raises every square by what another answer says, taking the brighter corner.
    ///
    /// Light sources and the sun do not add up into something brighter than either: the
    /// reference keeps the maximum per quarter (`update_light_quadrants`,
    /// `src/shadowcasting.h:56-62`), so a torch in daylight does not make a square "more than
    /// daytime".
    fn merge(&mut self, other: BTreeMap<(i32, i32), FourQuadrants>) {
        for (square, quarters) in other {
            match self.squares.get(&square) {
                Some(existing) => {
                    self.squares.insert(square, existing.max_with(quarters));
                }
                None => {
                    self.squares.insert(square, quarters);
                }
            }
        }
    }
}

/// Reference: `map::generate_lightmap` (`src/lightmap.cpp:443-560`), which clears the map,
/// casts every source and applies the sun. The store is the authority for what is where: the
/// terrain comes from the level resource, and smoke dims a square by being an entity standing
/// on it, not by being a copy of the level's own map.
#[derive(Debug, Resource)]
pub struct Map(pub cdda_core_types::level::Level);

impl std::ops::Deref for Map {
    type Target = cdda_core_types::level::Level;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Map {
    /// The terrain transparency rule the lighting system asks about.
    #[must_use]
    pub fn terrain_transparency(terrain: u32) -> f32 {
        // Solid terrain blocks; everything else is open air. The reference's per-terrain
        // transparency (`src/mapdata.cpp`) is not ported yet, so this is the honest default and
        // the ledger says so.
        if terrain == SOLID_TERRAIN {
            0.0
        } else {
            cdda_core_types::light::TRANSPARENCY_OPEN_AIR
        }
    }
}

/// Terrain that light cannot pass.
pub const SOLID_TERRAIN: u32 = 0;

/// Works out the light every source on the level casts, and writes it to the [`Light`] resource.
pub fn cast_lights(
    world: &mut World,
    terrain_transparency: impl Fn(u32) -> f32,
    field_opacity: impl Fn(FieldId, i32) -> f32,
) {
    // Fields dim a square: the most opaque thing on it wins, as the reference's transparency
    // cache does when it folds fields into terrain.
    let mut dimming: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    let mut field_query = world.query::<(&Position, &Field)>();
    for (position, field) in field_query.iter(world) {
        let value = field_opacity(field.id, field.entry.intensity());
        let entry = dimming.entry((position.x, position.y)).or_insert(0.0);
        if value > *entry {
            *entry = value;
        }
    }

    let mut sources: Vec<(i32, i32, f32)> = Vec::new();
    let mut source_query = world.query::<(&Position, &LightSource)>();
    for (position, source) in source_query.iter(world) {
        sources.push((position.x, position.y, source.luminance));
    }

    let mut light = BTreeMap::new();
    if let Some(mut map) = world.get_resource_mut::<Map>() {
        let level = &mut map.0;
        for (x, y, luminance) in sources {
            level.add_light_source(
                (x, y),
                luminance,
                &terrain_transparency,
                |field_x, field_y| dimming.get(&(field_x, field_y)).copied().unwrap_or(0.0),
                &mut light,
            );
        }
    }
    // Sources merge rather than replace: the sun may already have written an answer this
    // turn, and a torch does not undo the daylight.
    if let Some(mut existing) = world.get_resource_mut::<Light>() {
        existing.merge(light);
    } else {
        let mut answer = Light::default();
        answer.merge(light);
        world.insert_resource(answer);
    }
}

/// Works out the sunlight on the level and raises [`Light`] by it.
///
/// Reference: the sun half of `map::generate_lightmap` (`src/lightmap.cpp:443-470`) calling
/// `build_sunlight_cache`, and the ambient pass that lifts the quarters of a tile facing open
/// sky. Daylight and torches meet here, in one answer, rather than in two that have to be
/// reconciled by whoever reads them.
pub fn cast_sunlight(
    world: &mut World,
    turns: f64,
    indoors: f32,
    outside: impl Fn(i32, i32) -> bool,
) {
    let daylight = match world.get_resource::<Map>() {
        Some(map) => map.0.daylight(turns, indoors, outside),
        None => BTreeMap::new(),
    };
    if let Some(mut light) = world.get_resource_mut::<Light>() {
        light.merge(daylight);
    } else {
        let mut light = Light::default();
        light.merge(daylight);
        world.insert_resource(light);
    }
}

/// Clears the answer before a turn recomputes it, so nothing survives from the last turn.
pub fn clear_light(world: &mut World) {
    if let Some(mut light) = world.get_resource_mut::<Light>() {
        *light = Light::default();
    }
}

/// Moves part of a field onto a neighbouring square: the arithmetic of `map::gas_spread_to`
/// (`src/map_field.cpp:250-273`).
///
/// Gas thins as it spreads and gives up its share of the age: the fraction moved is the age
/// divided by the intensity. A square that already holds the same gas gets thicker by one, so a
/// fire's smoke accumulates downwind instead of sitting in a single tile.
///
/// The direction is the caller's decision. In the reference it comes from wind and a dice roll
/// (`map::spread_gas`), neither of which is ported yet, so this is the arithmetic that the
/// roll feeds into - tested on its own so that wiring the wind in later cannot quietly change
/// it.
pub fn gas_spread_to(world: &mut World, source: Entity, target: (i32, i32)) {
    let Some(source_field) = world.get::<Field>(source) else {
        return;
    };
    let intensity = source_field.entry.intensity();
    // The reference bails below two on the *caller's* side; keeping the guard here as well
    // means a single unit of gas cannot be squeezed into nothing by a second caller.
    if intensity <= 1 {
        return;
    }
    let age = source_field.entry.age();
    let age_fraction = age / i64::from(intensity);
    let id = source_field.id;
    let priority = source_field.priority;

    // Find the square's existing field of the same kind, if any.
    let mut existing = None;
    let mut query = world.query::<(Entity, &Position, &Field)>();
    for (entity, position, field) in query.iter(world) {
        if position.x == target.0 && position.y == target.1 && field.id == id {
            existing = Some(entity);
            break;
        }
    }

    if let Some(entity) = existing {
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            let raised = field.entry.intensity() + 1;
            field.entry.set_intensity(raised);
            let share = field.entry.age() + age_fraction;
            field.entry.set_age(share);
        }
    } else {
        let mut entry = FieldEntry::new(3, 100, true, false);
        entry.set_intensity(1);
        entry.set_age(age_fraction);
        world.spawn((
            Position {
                x: target.0,
                y: target.1,
            },
            Field {
                id,
                entry,
                priority,
            },
        ));
    }

    if let Some(mut field) = world.get_mut::<Field>(source) {
        field.entry.set_intensity(intensity - 1);
        field.entry.set_age(age - age_fraction);
    }
}

/// The squares a wind direction makes spread-resistant, as the reference's table has them.
///
/// Reference: `map::get_wind_blockers` (`src/map_field.cpp:2081-2105`). Matched by descending
/// threshold like every other wind table in the reference, which is why 330 and 0 agree.
#[must_use]
pub fn wind_blocker_offsets(angle: i32) -> [(i32, i32); 3] {
    const TABLE: [(i32, [(i32, i32); 3]); 9] = [
        (330, [(1, 0), (1, -1), (1, 1)]),
        (301, [(1, 1), (1, 0), (0, 1)]),
        (240, [(0, 1), (-1, 1), (1, 1)]),
        (211, [(-1, 1), (-1, 0), (0, 1)]),
        (150, [(-1, 0), (-1, -1), (-1, 1)]),
        (121, [(-1, -1), (0, -1), (-1, 0)]),
        (60, [(0, -1), (-1, -1), (1, -1)]),
        (31, [(1, -1), (1, 0), (0, -1)]),
        (0, [(1, 0), (1, -1), (1, 1)]),
    ];
    TABLE
        .iter()
        .find(|(threshold, _)| angle >= *threshold)
        .map_or([(0, 0); 3], |(_, offsets)| *offsets)
}

/// The eight neighbours in the reference's order: north-west first, then clockwise to
/// south-east (`eight_horizontal_neighbors`, `src/point.h:450-460`).
/// The eight neighbours, in the reference's order: north first, clockwise.
pub const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Chooses where a gas spreads to and moves it there.
///
/// Reference: `map::spread_gas` (`src/map_field.cpp:276-368`). The order of the dice is the
/// order the reference uses, because every roll advances one shared stream:
/// the chance to spread, then the starting neighbour, then the coin for a plain spread.
/// A square that cannot be entered is skipped; when the wind is strong, the squares facing it
/// are only taken on a `x_in_y(1, max(2, windpower))` chance.
///
/// Not ported, and declared: the reference first tries *down*, then a neighbour, then *up*.
/// The port has no z-levels here, so only the neighbour branch runs.
pub fn spread_gas(
    world: &mut World,
    source: Entity,
    percent_spread: i32,
    windpower: i32,
    wind_angle: i32,
    can_enter: impl Fn(i32, i32) -> bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(source) else {
        return;
    };
    let intensity = field.entry.intensity();
    if intensity <= 1 {
        return;
    }
    let Some(position) = world.get::<Position>(source).copied() else {
        return;
    };
    let id = field.id;
    let ceiling = 100 - windpower;
    if ceiling < 1 || rolls.rng(1, ceiling) > percent_spread {
        return;
    }

    let end_it = rolls.rng(0, 7);
    let mut spread: Vec<(i32, i32)> = Vec::new();
    for step in 1..=8 {
        let Some(&(dx, dy)) =
            NEIGHBOURS.get(usize::from(u8::try_from((end_it + step) % 8).unwrap_or(0)))
        else {
            continue;
        };
        let (x, y) = (position.x + dx, position.y + dy);
        if !can_enter(x, y) {
            continue;
        }
        // A neighbour already holding thicker gas cannot take any of this one.
        let mut query = world.query::<(&Position, &Field)>();
        let thicker = query
            .iter(world)
            .any(|(p, f)| p.x == x && p.y == y && f.id == id && f.entry.intensity() >= intensity);
        if !thicker {
            spread.push((x, y));
        }
    }
    if spread.is_empty() {
        return;
    }

    let target = if rolls.one_in(i32::try_from(spread.len()).unwrap_or(1)) {
        rolls.random_entry(&spread).copied()
    } else {
        let blockers = wind_blocker_offsets(wind_angle);
        let mut candidates: Vec<(i32, i32)> = Vec::new();
        for &(x, y) in &spread {
            let blocked = blockers
                .iter()
                .any(|&(dx, dy)| position.x + dx == x && position.y + dy == y);
            let chance = f64::from(windpower.max(2));
            if !blocked || rolls.x_in_y(1.0, chance) {
                candidates.push((x, y));
            }
        }
        rolls.random_entry(&candidates).copied()
    };

    if let Some(target) = target {
        gas_spread_to(world, source, target);
    }
}

/// Replaces one field on a square with another kind, carrying the age across.
///
/// The reference does this by setting the old field's intensity to zero and adding the new one
/// on the same square; the store hands out ids, so "replace" is a spawn and a despawn. Doing it
/// here keeps the two processors below readable and makes the age rule impossible to forget.
fn transform_field(
    world: &mut World,
    entity: Entity,
    new_id: FieldId,
    new_intensity: i32,
    age: i64,
    is_fire: bool,
) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let priority = world.get::<Field>(entity).map_or(0, |field| field.priority);
    let mut entry = FieldEntry::new(3, 100, true, is_fire);
    entry.set_intensity(new_intensity);
    entry.set_age(age);
    world.despawn(entity);
    world.spawn((
        Position {
            x: position.x,
            y: position.y,
        },
        Field {
            id: new_id,
            entry,
            priority,
        },
    ));
}

/// Field ids used by the two processors below.
///
/// These are the loader's ids, not placeholders: `fd_flame_burst` is the 28th field in the
/// reference's dump (plus one, so zero stays free for the null field), and `crates/sim/tests/
/// field_ids.rs` checks every constant here against the registry the data produces. The previous
/// values were 900 and up, and moving them exposed two tests that had quietly assumed the
/// placeholders' *ordering* - one filtering an inclusive range, one filtering a numeric span.
pub const FLAME_BURST: FieldId = FieldId(28);
/// See [`FLAME_BURST`].
pub const FIRE_VENT: FieldId = FieldId(26);

/// A flame burst burns down, and when it has burnt out it becomes a fire vent.
///
/// Reference: `field_processor_fd_flame_burst` (`src/map_field.cpp:640-650`). The reference also
/// makes hot air on every step that thins it; hot air is not ported, so that is missing rather
/// than forgotten.
pub fn process_flame_burst(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    let age = field.entry.age();
    if intensity > 1 {
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.set_intensity(intensity - 1);
        }
        // The reference makes hot air on the step that thins it, at the new intensity.
        if let Some(position) = world.get::<Position>(entity).copied() {
            create_hot_air(world, (position.x, position.y), intensity - 1, rolls);
        }
    } else {
        transform_field(world, entity, FIRE_VENT, 3, age, true);
    }
}

/// A fire vent thins one step in three, and when it is spent it becomes a flame burst.
///
/// Reference: `field_processor_fd_fire_vent` (`src/map_field.cpp:626-637`). The dice are drawn
/// only when the field is strong, exactly as the reference does - a spent vent draws nothing,
/// so a replay does not shift.
pub fn process_fire_vent(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    let age = field.entry.age();
    if intensity > 1 {
        if rolls.one_in(3)
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(intensity - 1);
        }
        if let Some(position) = world.get::<Position>(entity).copied() {
            create_hot_air(world, (position.x, position.y), intensity - 1, rolls);
        }
    } else {
        transform_field(world, entity, FLAME_BURST, 3, age, true);
    }
}

/// Places five thin wisps of hot air around a square.
///
/// Reference: `map::create_hot_air` (`src/map_field.cpp:375-400`). The intensity argument picks
/// *which* hot air this is - four different field types, not four strengths - and each of the
/// five attempts lands on the square itself or one of its eight neighbours, drawing the offset
/// from the shared dice.
///
/// The reference's switch has no case for anything outside one to four and falls through to a
/// debug message: that path draws no dice and creates nothing, which the tests hold onto because
/// a port that rolled there would shift every later roll for a case that never happens in play.
pub fn create_hot_air(
    world: &mut World,
    square: (i32, i32),
    intensity: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let id = match intensity {
        1 => HOT_AIR_1,
        2 => HOT_AIR_2,
        3 => HOT_AIR_3,
        4 => HOT_AIR_4,
        _ => return,
    };
    for _ in 0..5 {
        let dx = rolls.rng(-1, 1);
        let dy = rolls.rng(-1, 1);
        let (x, y) = (square.0 + dx, square.1 + dy);
        let mut entry = FieldEntry::new(3, 100, true, false);
        entry.set_intensity(1);
        world.spawn((
            Position { x, y },
            Field {
                id,
                entry,
                priority: 0,
            },
        ));
    }
}

/// The four hot air field ids, placeholders until the field loader is wired in.
pub const HOT_AIR_1: FieldId = FieldId(58);
/// See [`HOT_AIR_1`].
pub const HOT_AIR_2: FieldId = FieldId(59);
/// See [`HOT_AIR_1`].
pub const HOT_AIR_3: FieldId = FieldId(60);
/// See [`HOT_AIR_1`].
pub const HOT_AIR_4: FieldId = FieldId(61);

/// Smoke, as the field registry numbers it.
pub const SMOKE: FieldId = FieldId(19);

/// Fire, as the field registry numbers it.
pub const FIRE: FieldId = FieldId(14);
/// An extinguisher's cloud, as the field registry numbers it.
pub const EXTINGUISHER: FieldId = FieldId(16);

/// An extinguisher cloud fights fire on its own square, one intensity for one intensity.
///
/// Reference: `field_processor_fd_extinguisher` (`src/map_field.cpp:559-569`). The reference takes
/// the smaller of the two strengths and removes that much from both, so a cloud that is weaker
/// than the fire is spent entirely and leaves the fire burning lower; a cloud that is stronger
/// puts the fire out and survives with what is left. Neither side draws dice.
pub fn process_extinguisher(world: &mut World, entity: Entity) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(cloud) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let fire = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .filter(|(_, at, field)| at.x == position.x && at.y == position.y && field.id == FIRE)
            .map(|(fire_entity, _, _)| fire_entity)
            .next()
    };
    let Some(fire_entity) = fire else {
        return;
    };
    let Some(burning) = world
        .get::<Field>(fire_entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let fought = cloud.min(burning);
    for (target, intensity) in [(fire_entity, burning), (entity, cloud)] {
        let left = intensity - fought;
        if left <= 0 {
            world.despawn(target);
        } else if let Some(mut field) = world.get_mut::<Field>(target) {
            field.entry.set_intensity(left);
        }
    }
}

/// An acid vent, as the field registry numbers it.
pub const ACID_VENT: FieldId = FieldId(33);

/// A vent that is at full strength thins once its age passes a minute; a spent one refills.
///
/// Reference: `field_processor_fd_acid_vent` (`src/map_field.cpp:877-901`). A vent stronger than 1
/// thins by one and resets its age once it is at least a minute old, drawing nothing. A vent at 1
/// goes back to 3.
///
/// The reference's second half then walks a radius of 5 looking for acid fields **whose intensity
/// is zero** and gives them strength. That branch is dead in the reference as well: `set_intensity`
/// floors a field at 1, so a zero-intensity field cannot exist, and no acid is ever revived by it.
/// The port therefore does not reproduce it - a divergence recorded in the ledger, not an omission
/// - and the observable half, the refill, is what the tests pin.
pub fn process_acid_vent(
    world: &mut World,
    entity: Entity,
    _rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    let age = field.entry.age();
    if intensity > 1 {
        if age >= 60
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(intensity - 1);
            field.entry.set_age(0);
        }
    } else if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_intensity(3);
    }
}

/// Electricity, as the field registry numbers it.
pub const ELECTRICITY: FieldId = FieldId(29);
/// A shock vent, as the field registry numbers it.
pub const SHOCK_VENT: FieldId = FieldId(32);

/// A shock vent thins one time in five, and when spent erupts into lightning bolts.
///
/// Reference: `field_processor_fd_shock_vent` (`src/map_field.cpp:838-876`). A vent stronger than 1
/// draws `one_in(5)` and thins if it fires. A vent at 1 goes back to 3, draws 3 to 6 bolts, and for
/// each one picks a direction by drawing twice from -1..1 until the pair is not both zero, a length
/// from 4 to 12, then walks that many steps: every step lays electricity of strength 2 or 3, and
/// after each step two `one_in(4)` draws may turn the bolt - the x draw turns x on or off, the y
/// draw turns y on or off, and a direction that is already zero is given a fresh 0-or-1 sign.
///
/// The dice order is the contract here: a replay that drew in a different order would light a
/// different corridor with the same seed.
pub fn process_shock_vent(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    if intensity > 1 {
        if rolls.one_in(5)
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(intensity - 1);
        }
        return;
    }
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_intensity(3);
    }
    let bolts = rolls.rng(3, 6);
    for _ in 0..bolts {
        let (mut dx, mut dy) = (0, 0);
        while dx == 0 && dy == 0 {
            dx = rolls.rng(-1, 1);
            dy = rolls.rng(-1, 1);
        }
        let length = rolls.rng(4, 12);
        let (mut x, mut y) = (position.x, position.y);
        for _ in 0..length {
            x += dx;
            y += dy;
            lay_electricity(world, (x, y), rolls.rng(2, 3));
            if rolls.one_in(4) {
                dx = if dx == 0 { rolls.rng(0, 1) * 2 - 1 } else { 0 };
            }
            if rolls.one_in(4) {
                dy = if dy == 0 { rolls.rng(0, 1) * 2 - 1 } else { 0 };
            }
        }
    }
}

/// Adds electricity to a square, keeping the stronger of what was there and what arrives.
fn lay_electricity(world: &mut World, square: (i32, i32), strength: i32) {
    let existing = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .find(|(_, at, field)| at.x == square.0 && at.y == square.1 && field.id == ELECTRICITY)
            .map(|(entity, _, field)| (entity, field.entry.intensity()))
    };
    if let Some((entity, already)) = existing {
        if strength > already
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(strength);
        }
        return;
    }
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(strength);
    world.spawn((
        Position {
            x: square.0,
            y: square.1,
        },
        Field {
            id: ELECTRICITY,
            entry,
            priority: 0,
        },
    ));
}

/// An item type, as the item registry numbers it.
///
/// Zero is reserved for "no item", as with fields, so a stack entry can be cleared without
/// carrying an `Option` in every tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemTypeId(pub u32);

/// An item lying on the ground.
///
/// Reference: `map_stack` / `map::i_at` (`src/map.cpp`), which keeps a per-tile stack in the order
/// things were added. The port keeps the same order by entity index, which is spawn order: a fire
/// that burns a pile must consume things in the same sequence the reference would, or the same seed
/// gives different results.
#[derive(Debug, Clone, Copy, Component)]
pub struct GroundItem {
    /// What it is.
    pub kind: ItemTypeId,
    /// How much of it: charges for a stack, 1 for a single object.
    pub charges: i32,
    /// How long it has been lying there, in turns.
    pub age: i64,
}

/// Puts an item on a square.
pub fn drop_item(world: &mut World, square: (i32, i32), kind: ItemTypeId, charges: i32) -> Entity {
    world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
            },
            GroundItem {
                kind,
                charges,
                age: 0,
            },
        ))
        .id()
}

/// Puts an item on a square with an age, for things that have already been lying there a while.
pub fn drop_item_aged(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    charges: i32,
    age: i64,
) -> Entity {
    let entity = drop_item(world, square, kind, charges);
    if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
        item.age = age;
    }
    entity
}

/// Everything lying on one square, in the order it was put there.
#[must_use]
pub fn items_at(world: &mut World, square: (i32, i32)) -> Vec<Entity> {
    let mut found = Vec::new();
    {
        let mut query = world.query::<(Entity, &Position, &GroundItem)>();
        for (entity, at, _) in query.iter(world) {
            if at.x == square.0 && at.y == square.1 {
                found.push(entity);
            }
        }
    }
    found.sort_by_key(|entity| entity.index());
    found
}

/// Takes an item off the ground, if it is still there.
pub fn take_item(world: &mut World, entity: Entity) {
    if world.get::<GroundItem>(entity).is_some() {
        world.despawn(entity);
    }
}

/// Fire with nothing to eat burns out faster.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1128-1132`). When a fire did not gain
/// time from consuming something, and its tile has nothing burnable on it, the fire's age advances
/// by ten seconds per point of intensity - so a strong fire over bare ground dies sooner than a
/// weak one, which is the reference's own comment: "Drain more power from big fires, so that they
/// stop raging over nothing".
///
/// This is the first slice of the fire processor, and it is the one that needs no wind, no
/// neighbours and no items: the caller says whether the tile has fuel, because what counts as fuel
/// is terrain and furniture data the caller holds.
pub fn process_fire_burnout(world: &mut World, entity: Entity, has_fuel: bool) {
    if has_fuel {
        return;
    }
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        let age = field.entry.age();
        field.entry.set_age(age + i64::from(intensity) * 10);
    }
}

/// What the fire needs to know about a neighbouring square.
///
/// Five independent facts, so it is deliberately a bag of bools rather than an enum: the fuel flags
/// are not exclusive (terrain can carry more than one), and they are checked in the reference's
/// order rather than by kind.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NeighbourFacts {
    /// Something there is already burning.
    pub fire: bool,
    /// The terrain carries `FLAMMABLE`.
    pub flammable: bool,
    /// The terrain carries `FLAMMABLE_ASH`.
    pub flammable_ash: bool,
    /// The terrain carries `FLAMMABLE_HARD`.
    pub flammable_hard: bool,
    /// There are items lying there.
    pub items: bool,
}

/// Whether the fire sets one neighbour alight, drawing in the reference's order.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1290-1345`). The dice come in this
/// order and only when reached: `power = intensity + one_in(5)`, then `rng(1, 100)` against a
/// spread chance of 25 per point of intensity above one, then the fuel checks in flag order -
/// `FLAMMABLE` and `FLAMMABLE_ASH` each on `one_in(2)`, `FLAMMABLE_HARD` on `one_in(5)` and only
/// when power is at least 3 - then `one_in(5)` for a pile of items. A square already on fire is
/// skipped before any dice are drawn.
pub fn should_ignite(
    intensity: i32,
    facts: &NeighbourFacts,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    if facts.fire {
        return false;
    }
    let spread_chance = 25 * (intensity - 1);
    let power = intensity + i32::from(rolls.one_in(5));
    if rolls.rng(1, 100) >= spread_chance {
        return false;
    }
    if power >= 2 && facts.flammable && rolls.one_in(2) {
        return true;
    }
    if power >= 2 && facts.flammable_ash && rolls.one_in(2) {
        return true;
    }
    if power >= 3 && facts.flammable_hard && rolls.one_in(5) {
        return true;
    }
    rolls.one_in(5) && facts.items
}

/// Starts a fire of strength 1, two minutes old, on a neighbouring square if the dice allow.
///
/// Reference: `add_field( dst, fd_fire, 1, 2_minutes, false )` followed by the burning square
/// spending a minute of its own life (`src/map_field.cpp:1330-1336`). The neighbours are visited
/// from a random starting point and wrap around, as the reference does, so the same seed picks the
/// same squares in the same order.
///
/// The caller supplies the fuel facts, because they come from terrain and item data the sim does
/// not own.
pub fn process_fire_spread<F>(
    world: &mut World,
    entity: Entity,
    facts: F,
    rolls: &mut cdda_core_types::rng::Rolls,
) where
    F: Fn((i32, i32)) -> NeighbourFacts,
{
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let start = rolls.rng(0, 7);
    for step in 0..8 {
        let index = usize::try_from((start + 1 + step).rem_euclid(8)).unwrap_or(0);
        let Some((dx, dy)) = NEIGHBOURS.get(index).copied() else {
            continue;
        };
        let square = (position.x + dx, position.y + dy);
        if should_ignite(intensity, &facts(square), rolls) {
            let mut entry = FieldEntry::new(3, 100, true, true);
            entry.set_intensity(1);
            entry.set_age(-120);
            world.spawn((
                Position {
                    x: square.0,
                    y: square.1,
                },
                Field {
                    id: FIRE,
                    entry,
                    priority: 0,
                },
            ));
            if let Some(mut field) = world.get_mut::<Field>(entity) {
                let age = field.entry.age();
                field.entry.set_age(age + 60);
            }
        }
    }
}

/// How big a fire can get, given how long it has burned and what is next to it.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1240-1262`). A fire under half an hour
/// of fuel age (-500 minutes) can reach 3 outright. Otherwise its ceiling comes from neighbours -
/// three adjacent fires allow 2, seven allow 3 - and a fire with more than fifty minutes of fuel
/// age gets 2 even when it is alone.
#[must_use]
pub fn growth_ceiling(age: i64, adjacent_fires: usize) -> i32 {
    if age < -30_000 {
        return 3;
    }
    let by_neighbours = 1 + i32::from(adjacent_fires >= 3) + i32::from(adjacent_fires >= 7);
    if by_neighbours < 2 && age < -3_000 {
        return 2;
    }
    by_neighbours
}

/// Grows a fire one step when it has fuel to spare, and weakens what it gained.
///
/// Reference: `src/map_field.cpp:1255-1262`. A fire only grows while its age is negative - fuel in
/// the bank - and growing costs it: ten minutes of age per point of intensity it ends up with, so a
/// fire that jumps to 3 pays half an hour for the privilege.
#[must_use]
pub fn grow_fire(intensity: i32, age: i64, adjacent_fires: usize) -> Option<(i32, i64)> {
    if age >= 0 || intensity >= growth_ceiling(age, adjacent_fires) {
        return None;
    }
    let grown = intensity + 1;
    Some((grown, age + 600 * i64::from(grown)))
}

/// Whether a fire hands five minutes of its life to a neighbouring fire.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1178-1181`). The neighbour is boosted
/// when it is no stronger than this fire, or when it is older - "older" here means a larger age,
/// which is a shorter life left. Either way the fire that gives up the time is the one that could
/// spare it.
#[must_use]
pub fn should_boost(
    self_intensity: i32,
    self_age: i64,
    other_intensity: i32,
    other_age: i64,
) -> bool {
    other_intensity <= self_intensity || other_age > self_age
}

/// A strong fire spends its excess age making its neighbours bigger: the reference's flashpoint.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1164-1215`). Only a fire above intensity
/// 1 takes part, and only when `one_in(3)` fires. It then visits the eight neighbours from a random
/// start, wrapping around, and stops as soon as its own age is no longer negative - each neighbour
/// it feeds costs it five minutes. A fed neighbour under intensity 2 grows to 2 and always gains
/// five minutes of life (its age drops by 300).
///
/// Returns how many neighbours were on fire, which the caller uses for the growth ceiling. The
/// reference counts that in the same pass; so does this.
pub fn process_flashpoint(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> usize {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return 0;
    };
    let Some(field) = world.get::<Field>(entity) else {
        return 0;
    };
    let intensity = field.entry.intensity();
    if intensity <= 1 || !rolls.one_in(3) {
        return 0;
    }
    let start = rolls.rng(0, 7);
    let mut adjacent = 0;
    for step in 0..8 {
        let age = world
            .get::<Field>(entity)
            .map_or(0, |field| field.entry.age());
        if age >= 0 {
            break;
        }
        let index = usize::try_from((start + 1 + step).rem_euclid(8)).unwrap_or(0);
        let Some((dx, dy)) = NEIGHBOURS.get(index).copied() else {
            continue;
        };
        let square = (position.x + dx, position.y + dy);
        let neighbour = {
            let mut query = world.query::<(Entity, &Position, &Field)>();
            query
                .iter(world)
                .find(|(_, at, field)| at.x == square.0 && at.y == square.1 && field.id == FIRE)
                .map(|(other, _, field)| (other, field.entry.intensity(), field.entry.age()))
        };
        let Some((other, other_intensity, other_age)) = neighbour else {
            continue;
        };
        adjacent += 1;
        if !should_boost(intensity, age, other_intensity, other_age) {
            continue;
        }
        if other_intensity < 2
            && let Some(mut field) = world.get_mut::<Field>(other)
        {
            field.entry.set_intensity(2);
        }
        if let Some(mut field) = world.get_mut::<Field>(other) {
            field.entry.set_age(other_age - 300);
        }
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.set_age(age + 300);
        }
    }
    adjacent
}

/// What a fire leaves behind: smoke, and heat.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1400-1425`). Smoke is made only when the
/// terrain does not suppress it, when `rng(0, 100 - windpower) <= smoke` passes - so wind makes
/// smoke rarer - and when `rng(3, 35)` comes in under the fire's intensity times ten. Hot air is
/// made when fewer than five fires are adjacent, on a `rng(0, 4 - adjacent)` that must be non-zero,
/// which is the reference's way of thinning it out as the neighbouring fires multiply.
///
/// The reference prefers to put smoke on the square above and falls back to its own square. The port
/// has no levels yet, so it always takes the fallback: smoke at the fire's square, at full intensity
/// of the fire. When levels land, this is the function that grows a branch.
pub fn process_fire_residue(
    world: &mut World,
    entity: Entity,
    smoke: i32,
    windpower: i32,
    suppressed: bool,
    adjacent_fires: usize,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    if !suppressed && rolls.rng(0, 100 - windpower) <= smoke && rolls.rng(3, 35) < intensity * 10 {
        add_field_at(world, (position.x, position.y), SMOKE, intensity);
    }
    let ceiling = 4 - i32::try_from(adjacent_fires).unwrap_or(4);
    if adjacent_fires < 5 && ceiling > 0 && rolls.rng(0, ceiling) != 0 {
        create_hot_air(world, (position.x, position.y), intensity, rolls);
    }
}

/// Adds or strengthens a field on a square, keeping the stronger of what is there and what arrives.
fn add_field_at(world: &mut World, square: (i32, i32), id: FieldId, strength: i32) {
    let existing = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .find(|(_, at, field)| at.x == square.0 && at.y == square.1 && field.id == id)
            .map(|(entity, _, field)| (entity, field.entry.intensity()))
    };
    if let Some((entity, already)) = existing {
        if strength > already
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(strength);
        }
        return;
    }
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(strength);
    world.spawn((
        Position {
            x: square.0,
            y: square.1,
        },
        Field {
            id,
            entry,
            priority: 0,
        },
    ));
}

/// What a square's terrain has to offer a fire.
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct TerrainFuel {
    /// `FLAMMABLE`: burns every turn.
    pub flammable: bool,
    /// `FLAMMABLE_HARD`: burns only when `one_in(3)` allows.
    pub flammable_hard: bool,
    /// `FLAMMABLE_ASH`: burns and leaves ash behind.
    pub flammable_ash: bool,
}

/// What one turn of burning terrain gives the fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainBurn {
    /// Turns of life added to the fire.
    pub time_added: i64,
    /// Smoke contribution.
    pub smoke: i32,
    /// Whether the terrain is wrecked this turn.
    pub bashes: bool,
    /// Ash left behind, as charges, when the terrain was of the ashy kind.
    pub ash_charges: Option<i32>,
}

/// Consuming the ground a fire is standing on.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1051-1090`). `FLAMMABLE` ground feeds the
/// fire `5 - intensity` turns and always burns. `FLAMMABLE_HARD` needs `one_in(3)` first and gives
/// `4 - intensity`. `FLAMMABLE_ASH` always burns, gives `5 - intensity`, and leaves ash when the
/// ground is wrecked. Every burning kind adds two smoke plus one per five points of wind power, and
/// a fire above intensity 1 wrecks the terrain on `one_in(200 - intensity * 50)`; the ashy kind then
/// leaves 10 to 1000 charges of ash.
///
/// The dice order is the reference's: the hard-ground check first, then the wrecking check, then the
/// ash. A square that is merely `FLAMMABLE_HARD` and fails its `one_in(3)` burns nothing at all.
#[must_use]
pub fn burn_terrain(
    fuel: TerrainFuel,
    intensity: i32,
    windpower: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Option<TerrainBurn> {
    let (time_added, ashy) = if fuel.flammable {
        (i64::from(5 - intensity), false)
    } else if fuel.flammable_hard && rolls.one_in(3) {
        (i64::from(4 - intensity), false)
    } else if fuel.flammable_ash {
        (i64::from(5 - intensity), true)
    } else {
        return None;
    };
    let mut smoke = 2 + windpower / 5;
    let mut bashes = false;
    let mut ash_charges = None;
    if intensity > 1 && rolls.one_in(200 - intensity * 50) {
        bashes = true;
        if ashy {
            ash_charges = Some(rolls.rng(10, 1000));
        }
    }
    if smoke < 0 {
        smoke = 0;
    }
    Some(TerrainBurn {
        time_added,
        smoke,
        bashes,
        ash_charges,
    })
}

/// Which neighbours a fire may reach through the wind, and which the wind shields.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1136-1152`). The reference caches three
/// wind-blocking neighbours for the wind direction and then keeps a neighbour when
///
/// ```text
/// (x != b1.x && y != b1.y) || (x != b2.x && y != b2.y) || (x != b3.x && y != b3.y)
///     || x_in_y( 1, max( 2, windpower ) )
/// ```
///
/// The "differs" test is `x != b.x && y != b.y`, so a neighbour that shares **either** coordinate
/// with a blocker counts as not differing from it. Only a neighbour that shares coordinates with all
/// three blockers reaches the `x_in_y` and can be shielded; every other neighbour is kept without
/// drawing. For the usual three distinct blockers that means exactly those blockers are the ones the
/// wind can stop, and only sometimes. The port reproduces the expression as written - dice included,
/// because a replay has to draw the same numbers.
#[must_use]
pub fn wind_reach(
    neighbour: (i32, i32),
    blockers: [(i32, i32); 3],
    windpower: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    let differs = |blocker: (i32, i32)| neighbour.0 != blocker.0 && neighbour.1 != blocker.1;
    differs(blockers[0])
        || differs(blockers[1])
        || differs(blockers[2])
        || rolls.x_in_y(1.0, f64::from(windpower.max(2)))
}

/// How many of the eight neighbours are on fire.
fn count_adjacent_fires(world: &mut World, entity: Entity) -> usize {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return 0;
    };
    let mut query = world.query::<(&Position, &Field)>();
    query
        .iter(world)
        .filter(|(at, field)| {
            field.id == FIRE
                && (at.x != position.x || at.y != position.y)
                && (at.x - position.x).abs() <= 1
                && (at.y - position.y).abs() <= 1
        })
        .count()
}

/// Everything one turn of burning needs from the world around the fire.
pub struct FireContext<'a> {
    /// What the fire is standing on.
    pub fuel: TerrainFuel,
    /// What a neighbouring square offers, asked once per neighbour.
    pub neighbour: &'a dyn Fn((i32, i32)) -> NeighbourFacts,
    /// Wind strength at the fire.
    pub windpower: i32,
    /// Items consumed this turn, which gate smoke.
    pub consumed: i32,
    /// Whether the terrain suppresses smoke.
    pub smoke_suppressed: bool,
}

/// One turn of a fire, in the reference's order.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:945-1425`), which runs its steps in this
/// sequence and draws dice in this sequence:
///
/// 1. eat the ground (terrain fuel decides the time and smoke gained);
/// 2. age: gained time is subtracted, otherwise a fire with fuel ages ten seconds per intensity;
/// 3. a strong fire's `one_in(3)` flashpoint, or the growth check for the others - never both;
/// 4. spreading to neighbours;
/// 5. smoke and hot air.
///
/// What is not here yet, and is named rather than implied: items burning, vehicles, zero-level
/// fall-through, webs, pits, and the effects a fire has on creatures.
pub fn process_fire(
    world: &mut World,
    entity: Entity,
    context: &FireContext<'_>,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let terrain_burn = burn_terrain(context.fuel, intensity, context.windpower, rolls);
    let mut smoke = context.consumed;
    if let Some(burn) = terrain_burn {
        smoke += burn.smoke;
    }
    // Age: gained time is spent first; a fire with nothing to eat starves.
    if let Some(burn) = terrain_burn
        && burn.time_added != 0
    {
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            let age = field.entry.age();
            field.entry.set_age(age - burn.time_added);
        }
    } else {
        let has_fuel = terrain_burn.is_some();
        process_fire_burnout(world, entity, has_fuel);
    }
    // A strong fire spends its extra life on neighbours, or else grows itself.
    let adjacent = if intensity > 1 {
        process_flashpoint(world, entity, rolls)
    } else {
        let age = world
            .get::<Field>(entity)
            .map_or(0, |field| field.entry.age());
        let adjacent = count_adjacent_fires(world, entity);
        if let Some((grown, age)) = grow_fire(intensity, age, adjacent)
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(grown);
            field.entry.set_age(age);
        }
        adjacent
    };
    process_fire_spread(world, entity, |square| (context.neighbour)(square), rolls);
    process_fire_residue(
        world,
        entity,
        smoke,
        context.windpower,
        context.smoke_suppressed,
        adjacent,
        rolls,
    );
}

/// A field that throws rocks around.
pub const PUSH_ITEMS: FieldId = FieldId(31);

/// A push-items field hurls aged rocks at its neighbours that also hold one.
///
/// Reference: `field_processor_fd_push_items` (`src/map_field.cpp:790-836`). Only rocks count, and
/// only rocks that have lain there at least one turn; a rock is thrown to a random one of the
/// neighbouring squares that has a push-items field of its own, losing its age on the way. A square
/// with no such neighbour keeps its rocks. The reference also hurts whatever is standing where a
/// rock lands - the port has no creatures yet, so that part is named in the ledger rather than
/// guessed at.
///
/// The caller supplies the item id for a rock, because item ids come from the item registry.
pub fn process_push_items(
    world: &mut World,
    square: (i32, i32),
    rock: ItemTypeId,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> usize {
    let rocks: Vec<Entity> = items_at(world, square)
        .into_iter()
        .filter(|entity| {
            world
                .get::<GroundItem>(*entity)
                .is_some_and(|item| item.kind == rock && item.age >= 1)
        })
        .collect();
    if rocks.is_empty() {
        return 0;
    }
    let destinations: Vec<(i32, i32)> = {
        let mut found = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let candidate = (square.0 + dx, square.1 + dy);
                let mut query = world.query::<(&Position, &Field)>();
                if query.iter(world).any(|(at, field)| {
                    field.id == PUSH_ITEMS && at.x == candidate.0 && at.y == candidate.1
                }) {
                    found.push(candidate);
                }
            }
        }
        found
    };
    if destinations.is_empty() {
        return 0;
    }
    let mut moved = 0;
    for entity in rocks {
        let Some(item) = world.get::<GroundItem>(entity).copied() else {
            continue;
        };
        let pick = rolls.rng(0, i32::try_from(destinations.len() - 1).unwrap_or(0));
        let Some(destination) = destinations
            .get(usize::try_from(pick).unwrap_or(0))
            .copied()
        else {
            continue;
        };
        take_item(world, entity);
        drop_item_aged(world, destination, item.kind, item.charges, 0);
        moved += 1;
    }
    moved
}
