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
/// Everything a gas needs to know about the wind and the sky.
pub struct GasContext {
    /// The chance in a hundred that this gas spreads on a windless turn.
    pub percent_spread: i32,
    /// Wind strength where the gas is.
    pub windpower: i32,
    /// Wind direction, in degrees, for the blockers.
    pub wind_angle: i32,
    /// Whether the square is outdoors, which makes the gas dissipate faster.
    pub outdoors: bool,
    /// How much faster, in turns.
    pub outdoor_age_speedup: i64,
}

/// Chooses where a gas spreads to and moves it there, aging it first if it is outdoors.
pub fn spread_gas(
    world: &mut World,
    source: Entity,
    context: &GasContext,
    can_enter: impl Fn(i32, i32) -> bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let percent_spread = context.percent_spread;
    let (windpower, wind_angle) = (context.windpower, context.wind_angle);
    let outdoors = context.outdoors;
    let outdoor_age_speedup = context.outdoor_age_speedup;
    let Some(field) = world.get::<Field>(source) else {
        return;
    };
    let intensity = field.entry.intensity();
    let id = field.id;
    // Dissipate faster outdoors, before any of the spread dice - and before the intensity bail,
    // which is what the reference does: a thin gas outdoors still ages away faster.
    if outdoors && outdoor_age_speedup != 0 {
        let age = world
            .get::<Field>(source)
            .map_or(0, |field| field.entry.age());
        if let Some(mut field) = world.get_mut::<Field>(source) {
            field.entry.set_age(age + outdoor_age_speedup);
        }
    }
    if intensity <= 1 {
        return;
    }
    let Some(position) = world.get::<Position>(source).copied() else {
        return;
    };
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
    /// How much of a solid item has burned away so far.
    pub burnt: i32,
    /// How damaged it is, in the reference's own units.
    pub damage: i32,
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
                burnt: 0,
                damage: 0,
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
            // Something inside a container is not lying on the square, whatever square its container is
            // standing on. Without this, a craft that holds its own parts looks like it left them there.
            if at.x == square.0 && at.y == square.1 && world.get::<Inside>(entity).is_none() {
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

/// The gas processor: fields whose data says they spread do so every turn.
///
/// Reference: `field_processor_spread_gas` (`src/map_field.cpp:579-585`), which runs for any field
/// with a non-zero spread chance (`field_type::gas_can_spread`). Everything else about the turn -
/// the chance, the damping by wind, where the gas goes - is [`spread_gas`].
///
/// The caller supplies the spread chance, the wind and the outdoor speedup, because those come from
/// the field's prototype, which the sim does not own.
pub fn process_spread_gas(
    world: &mut World,
    source: Entity,
    context: &GasContext,
    can_enter: impl Fn(i32, i32) -> bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    if context.percent_spread <= 0 {
        return;
    }
    spread_gas(world, source, context, can_enter, rolls);
}

/// A field under water ages away faster.
///
/// Reference: `field_processor_underwater_dissipation` (`src/map_field.cpp:516-523`). If the terrain
/// carries the swimmable flag the field's age advances by the field type's
/// `underwater_age_speedup`; nothing else happens and no dice are drawn.
pub fn process_underwater_dissipation(
    world: &mut World,
    entity: Entity,
    swimmable: bool,
    speedup: i64,
) {
    if !swimmable || speedup == 0 {
        return;
    }
    let Some(age) = world.get::<Field>(entity).map(|field| field.entry.age()) else {
        return;
    };
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_age(age + speedup);
    }
}

/// A field that grows one intensity on its own, once in a while.
///
/// Reference: `field_processor_upgrade_intensity` (`src/map_field.cpp:503-514`). The level's
/// `upgrade_chance` and `upgrade_duration` must both be positive, the calendar must land on the
/// period (`calendar::once_every`), and only then is `one_in(chance)` drawn.
///
/// The data says exactly one field in the corpus can do this at all (`fd_fatigue`), which is worth
/// knowing before wondering why a processor never seems to fire.
pub fn process_upgrade_intensity(
    world: &mut World,
    entity: Entity,
    now: i64,
    chance: i32,
    duration: i64,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    if chance <= 0 || duration <= 0 || now.rem_euclid(duration) != 0 {
        return;
    }
    if !rolls.one_in(chance) {
        return;
    }
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_intensity(intensity + 1);
    }
}

/// A monster type, as the monster registry numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonsterKindId(pub u32);

/// A creature standing on a square.
///
/// Reference: `monster` and `Creature` (`src/monster.h`). The port keeps the smallest thing a field
/// processor can touch: what it is, how much health it has left, and where it stands. Speed, moves,
/// armour and behaviour come as the systems that need them arrive.
#[derive(Debug, Clone, Copy, Component)]
pub struct Monster {
    /// What kind of creature it is.
    pub kind: MonsterKindId,
    /// Hit points left.
    pub hp: i32,
    /// Moves left this turn; the reference's action points.
    pub moves: f64,
}

/// Puts a creature on a square.
pub fn spawn_monster(
    world: &mut World,
    square: (i32, i32),
    kind: MonsterKindId,
    hp: i32,
) -> Entity {
    world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
            },
            Monster {
                kind,
                hp,
                moves: 100.0,
            },
        ))
        .id()
}

/// Every creature standing on one square, in spawn order.
#[must_use]
pub fn monsters_at(world: &mut World, square: (i32, i32)) -> Vec<Entity> {
    let mut found = Vec::new();
    {
        let mut query = world.query::<(Entity, &Position, &Monster)>();
        for (entity, at, _) in query.iter(world) {
            if at.x == square.0 && at.y == square.1 {
                found.push(entity);
            }
        }
    }
    found.sort_by_key(|entity| entity.index());
    found
}

/// Applies a gas effect to one creature.
///
/// Returns whether the creature died. Reference: the per-creature part of
/// `map_field::process_fields_in_submap` (`src/map_field.cpp:1900-1990`), which deals damage and
/// takes moves in the same pass.
pub fn breathe_gas(world: &mut World, entity: Entity, effect: &GasEffect) -> bool {
    if let Some(mut monster) = world.get_mut::<Monster>(entity) {
        monster.moves -= effect.moves_penalty;
    }
    damage_monster(world, entity, effect.damage)
}

/// Hurts a creature, and takes it off the map when it has nothing left.
///
/// Returns whether the creature died. Reference: `Creature::deal_damage` and the death check that
/// follows it (`src/monster.cpp`); the port keeps the part a field needs - damage goes in, and a
/// creature at zero or below leaves the world.
pub fn damage_monster(world: &mut World, entity: Entity, amount: i32) -> bool {
    let Some(hp) = world.get::<Monster>(entity).map(|monster| monster.hp) else {
        return false;
    };
    let left = hp - amount.max(0);
    if left <= 0 {
        world.despawn(entity);
        return true;
    }
    if let Some(mut monster) = world.get_mut::<Monster>(entity) {
        monster.hp = left;
    }
    false
}

/// What breathing a gas does to a creature this turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasEffect {
    /// Hit points lost, before armour.
    pub damage: i32,
    /// Moves taken away, in raw move units.
    pub moves_penalty: f64,
}

/// Toxic gas: a little damage, a little slowness, per point of intensity.
///
/// Reference: `field_processor_fd_toxic_gas` (`src/map_field.cpp:1966-1971`). A creature that
/// breathes takes damage equal to the intensity and loses that many moves. Nothing is drawn.
#[must_use]
pub fn toxic_gas_effect(intensity: i32, no_breathe: bool) -> GasEffect {
    if no_breathe {
        return GasEffect {
            damage: 0,
            moves_penalty: 0.0,
        };
    }
    GasEffect {
        damage: intensity,
        moves_penalty: f64::from(intensity),
    }
}

/// Nuke gas: worse with intensity, and much worse for plants.
///
/// Reference: `field_processor_fd_nuke_gas` (`src/map_field.cpp:1972-1990`). Each intensity draws its
/// own move penalty and damage; a plant takes a second move penalty on top, drawn from the intensity,
/// and then has its damage multiplied by the intensity.
///
/// The draw order matters: the move penalty first, the damage second, and the plant's penalty last.
#[must_use]
pub fn nuke_gas_effect(
    intensity: i32,
    no_breathe: bool,
    veggy: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> GasEffect {
    if no_breathe {
        return GasEffect {
            damage: 0,
            moves_penalty: 0.0,
        };
    }
    let (mut moves_penalty, mut damage) = match intensity {
        3 => (rolls.rng_float(0.6, 1.2), rolls.rng(30, 50)),
        2 => (rolls.rng_float(0.2, 0.5), rolls.rng(10, 25)),
        _ => (rolls.rng_float(0.0, 0.15), rolls.rng(0, 12)),
    };
    if veggy {
        moves_penalty += rolls.rng_float(f64::from(intensity) * 0.05, f64::from(intensity) * 0.12);
        damage *= intensity;
    }
    GasEffect {
        damage,
        moves_penalty,
    }
}

/// How many spawn attempts a field asks for this turn, and whether it is allowed any at all.
///
/// Reference: `field_processor_monster_spawn` (`src/map_field.cpp:763-787`). A level needs a positive
/// count and a positive chance, and then a single `one_in(chance)` decides; the count is how many
/// monsters the group will be asked for, not how many separate rolls happen.
///
/// The port stops here, at the gate, and says so: turning a monster group into actual creatures needs
/// the group loader and a deferred spawn list, and neither exists yet. What is ported is the part the
/// data decides - 15 for the Tindalos rift, 600 for the fatigue tear.
#[must_use]
pub fn monster_spawn_attempts(
    spawn_chance: i32,
    spawn_count: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> i32 {
    if spawn_chance <= 0 || spawn_count <= 0 {
        return 0;
    }
    if rolls.one_in(spawn_chance) {
        spawn_count
    } else {
        0
    }
}

/// One material in an item: its portions, and its behaviour at each intensity.
#[derive(Debug, Clone, Copy)]
pub struct MaterialPortion {
    /// How many portions of it the item has.
    pub portions: i32,
    /// Its behaviour at intensity 1, 2 and 3, in that order.
    pub levels: [BurnDataView; 3],
}

impl MaterialPortion {
    /// The material's behaviour at an intensity, clamped to the three the data describes.
    #[must_use]
    pub fn at(&self, intensity: i32) -> &BurnDataView {
        // The data describes exactly three intensities, so the clamp above always lands inside it.
        self.levels
            .get(usize::try_from(intensity.clamp(1, 3) - 1).unwrap_or(0))
            .unwrap_or(&self.levels[0])
    }
}

/// What a material contributes at one intensity, as the item sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnDataView {
    /// Nothing burns.
    pub immune: bool,
    /// Volume the material can burn per turn, in millilitres. Zero means unconstrained.
    pub volume_per_turn: f64,
    /// Fuel contributed.
    pub fuel: f64,
    /// Smoke produced.
    pub smoke: f64,
    /// Fraction destroyed per turn.
    pub burn: f64,
}

/// An item as the fire sees it.
#[derive(Debug, Clone)]
pub struct Burnable {
    /// Its materials and their portions.
    pub materials: Vec<MaterialPortion>,
    /// Its base volume, in millilitres.
    pub volume_ml: f64,
    /// Whether it is a liquid, which smothers weak fires.
    pub liquid: bool,
    /// The sum of material portions the item type declares, one when it declares none.
    pub material_total: i32,
    /// Whether it is counted by charges (a stack rather than one object).
    pub by_charges: bool,
    /// How many charges it has.
    pub charges: i32,
    /// How many charges make one stack.
    pub stack_size: i32,
}

/// What one turn of burning does to an item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnResult {
    /// Fuel added to the fire.
    pub fuel: f64,
    /// Smoke produced.
    pub smoke: f64,
    /// Fraction of the item destroyed, which the caller turns into charges.
    pub burn: f64,
}

/// How much of an item a fire consumes in one turn.
///
/// Reference: `item::simulate_burn` (`src/item.cpp:12543-12592`). A contained fire burns at intensity
/// 3 whatever its real intensity. A material that is immune stops the whole item burning. Each
/// material contributes fuel, smoke and burn, scaled by the volume it can burn per turn against the
/// item's volume unless the fire is contained or the material has no volume limit. A liquid that
/// would give the fire less than 200 draws `rng(400 * litres, 1200 * litres)` to *subtract*, because
/// a wet thing smothers a small fire. Several materials are averaged by the item's declared portions,
/// and an item with no materials at all burns at a flat rate of one. A stack multiplies all three by
/// its number of stacks.
#[must_use]
pub fn simulate_burn(
    item: &Burnable,
    fire_intensity: i32,
    contained: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> BurnResult {
    let intensity = if contained { 3 } else { fire_intensity };
    let mut fuel = 0.0;
    let mut smoke = 0.0;
    let mut burn = 0.0;
    for material in &item.materials {
        let burn_data = material.at(intensity);
        if burn_data.immune {
            return BurnResult {
                fuel: 0.0,
                smoke: 0.0,
                burn: 0.0,
            };
        }
        let scale = f64::from(material.portions);
        if contained || burn_data.volume_per_turn <= 0.0 {
            fuel += burn_data.fuel * scale;
            smoke += burn_data.smoke * scale;
            burn += burn_data.burn * scale;
        } else {
            let litres = item.volume_ml / 1000.0;
            let rate = if litres > 0.0 {
                (burn_data.volume_per_turn / 1000.0) / litres
            } else {
                0.0
            };
            fuel += burn_data.fuel * rate * scale;
            smoke += burn_data.smoke * rate * scale;
            burn += burn_data.burn * rate * scale;
        }
    }
    let materials = i32::try_from(item.materials.len()).unwrap_or(0);
    if item.liquid && fuel < 200.0 {
        let litres = item.volume_ml / 1000.0;
        fuel -= rolls.rng_float(400.0 * litres, 1200.0 * litres);
    } else if materials > 1 {
        let total = f64::from(item.material_total.max(1));
        fuel /= total;
        smoke /= total;
        burn /= total;
    } else if materials == 0 {
        burn = 1.0;
    }
    if item.by_charges {
        let stacks = (item.charges / item.stack_size.max(1)).max(1);
        let scale = f64::from(stacks);
        fuel *= scale;
        smoke *= scale;
        burn *= scale;
    }
    BurnResult { fuel, smoke, burn }
}

/// Rounds a value down, and up with the probability of its fraction.
///
/// Reference: `roll_remainder` (`src/rng.cpp`). A whole value draws nothing; a fractional one draws
/// once. The order of these draws is part of the contract for anything that burns.
#[must_use]
pub fn roll_remainder(value: f64, rolls: &mut cdda_core_types::rng::Rolls) -> i32 {
    let whole = value.trunc();
    let fraction = value - whole;
    let bumped = if fraction > 0.0 {
        rolls.rng_float(0.0, 1.0) < fraction
    } else {
        false
    };
    #[allow(clippy::cast_possible_truncation)]
    let result = whole as i32;
    if bumped { result + 1 } else { result }
}

/// What a burning item remembers between turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnState {
    /// Whether it is a stack counted in charges.
    pub by_charges: bool,
    /// Charges left, for a stack.
    pub charges: i32,
    /// Charges in one stack.
    pub stack_size: i32,
    /// Its base volume, in millilitres.
    pub volume_ml: f64,
    /// How much of a solid item has burned away so far.
    pub burnt: i32,
}

/// Applies a turn of burning to one item, and says whether it is gone.
///
/// Reference: the bookkeeping half of `item::burn` (`src/item.cpp:12609-12643`). A stack loses
/// `roll_remainder(burn * 250ml * stack_size / (3 * volume))` charges, and a stack with no volume
/// loses them all at once. A solid item accumulates `roll_remainder(burn)` and is gone once it has
/// burned `volume / 250ml * 3`.
pub fn burn_item(
    state: &mut BurnState,
    burn_added: f64,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    if state.by_charges {
        if state.volume_ml <= 0.0 {
            state.charges = 0;
        } else {
            let lost = roll_remainder(
                burn_added * 250.0 * f64::from(state.stack_size) / (3.0 * state.volume_ml),
                rolls,
            );
            state.charges -= lost;
        }
        return state.charges <= 0;
    }
    state.burnt += roll_remainder(burn_added, rolls);
    let threshold = (state.volume_ml / 250.0 * 3.0).trunc();
    #[allow(clippy::cast_possible_truncation)]
    let limit = threshold as i32;
    state.burnt >= limit
}

/// How to build the fire's view of one kind of item.
#[derive(Debug, Clone)]
pub struct ItemBurnPlan {
    /// Its materials and their portions.
    pub materials: Vec<MaterialPortion>,
    /// Its base volume, in millilitres.
    pub volume_ml: f64,
    /// Whether it is a liquid.
    pub liquid: bool,
    /// The item type's declared material portions, one when it declares none.
    pub material_total: i32,
    /// Whether it is counted in charges.
    pub by_charges: bool,
    /// How many charges make one stack.
    pub stack_size: i32,
}

/// What one square's worth of burning produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemBurnTotals {
    /// Smoke produced, before rounding.
    pub smoke: f64,
    /// Fuel produced, before rounding.
    pub fuel: f64,
    /// How many items the fire finished off.
    pub consumed: i32,
    /// How many items went off instead of burning.
    pub detonated: i32,
}

/// What the burning loop needs to ask about an item, bundled so the signature stays readable.
pub struct ItemFireRules<F, I, E> {
    /// How it burns, or `None` when it does not.
    pub plan_of: F,
    /// Whether it cannot be taken out of its container.
    pub irremovable: I,
    /// Whether a fire sets it off rather than burning it.
    pub explodes: E,
}

/// Burns what is lying on one square.
///
/// Reference: the item loop inside `field_processor_fd_fire` (`src/map_field.cpp:980-1031`). A fire
/// consumes at most `intensity * 2` items a turn and stops as soon as it has. Each item's contribution
/// comes from [`simulate_burn`] and its bookkeeping from [`burn_item`]; something the fire finishes
/// leaves the map. Smoke and fuel are summed for the caller, which rounds them the way the reference
/// does.
///
/// Not ported, and declared: items that explode in fire (`will_explode_in_fire`), the contents of a
/// destroyed container, and burn products spawned by lost weight. All three need item contents, which
/// the port does not carry yet.
pub fn burn_items_in_fire(
    world: &mut World,
    square: (i32, i32),
    intensity: i32,
    contained: bool,
    rules: &ItemFireRules<
        impl Fn(ItemTypeId) -> Option<ItemBurnPlan>,
        impl Fn(Entity) -> bool,
        impl Fn(Entity) -> bool,
    >,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> ItemBurnTotals {
    let max_consume = intensity * 2;
    let mut totals = ItemBurnTotals {
        smoke: 0.0,
        fuel: 0.0,
        consumed: 0,
        detonated: 0,
    };
    // Items that go off in a fire are removed before anything burns (`src/map_field.cpp:982-993`):
    // the reference detonates them and swallows whatever they throw out. The port removes them and
    // spills what they held; the explosion itself needs blast damage, which is declared not ported.
    for entity in items_at(world, square) {
        if !(rules.explodes)(entity) {
            continue;
        }
        for held in contents(world, entity) {
            take_out(world, held);
            if (rules.irremovable)(held) {
                world.despawn(held);
            }
        }
        world.despawn(entity);
        totals.detonated += 1;
    }
    let here = items_at(world, square);
    for entity in here {
        if totals.consumed >= max_consume {
            break;
        }
        let (kind, charges, burnt) = match world.get::<GroundItem>(entity) {
            Some(item) => (item.kind, item.charges, item.burnt),
            None => continue,
        };
        let Some(plan) = (rules.plan_of)(kind) else {
            continue;
        };
        let burnable = Burnable {
            materials: plan.materials,
            volume_ml: plan.volume_ml,
            liquid: plan.liquid,
            material_total: plan.material_total,
            by_charges: plan.by_charges,
            charges,
            stack_size: plan.stack_size,
        };
        let burn = simulate_burn(&burnable, intensity, contained, rolls);
        totals.smoke += burn.smoke;
        totals.fuel += burn.fuel;
        let mut state = BurnState {
            by_charges: plan.by_charges,
            charges,
            stack_size: plan.stack_size,
            volume_ml: plan.volume_ml,
            burnt,
        };
        let destroyed = burn_item(&mut state, burn.burn, rolls);
        if destroyed {
            // A container that burns away spills what was inside it onto the square, which is the
            // reference's `new_content` list (`src/map_field.cpp:1015-1028`). An irremovable item
            // stays where it was, so it does not spill.
            for held in contents(world, entity) {
                take_out(world, held);
                // The reference collects only contents without `NO_UNLOAD` into `new_content`;
                // anything irremovable is erased along with the container rather than spilling.
                if (rules.irremovable)(held) {
                    world.despawn(held);
                }
            }
            world.despawn(entity);
            totals.consumed += 1;
        } else if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
            item.charges = state.charges;
            item.burnt = state.burnt;
        }
    }
    totals
}

/// What a turn needs to know about one kind of field.
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldTurnData {
    /// The chance in a hundred that this gas spreads to a neighbour.
    pub percent_spread: i32,
    /// How much faster it ages away outdoors, in turns.
    pub outdoor_age_speedup: i64,
    /// How much faster it ages away under water, in turns.
    pub underwater_age_speedup: i64,
    /// The self-upgrade chance and period for the intensity it is at, zero when it cannot grow.
    pub upgrade_chance: i32,
    /// How often the upgrade chance is rolled, in turns.
    pub upgrade_duration: i64,
    /// Whether the square it stands on is outdoors.
    pub outdoors: bool,
    /// Whether the square it stands on is swimmable.
    pub swimmable: bool,
}

/// The part of the world a turn touches: the reference's reality bubble.
///
/// The reference keeps 11 by 11 submaps loaded around the player (`MAPSIZE` in
/// `src/map_scale.h`), each 12 by 12, which is 132 squares on a side and a radius of 66 from the
/// player's square. Anything further away is not simulated at all, which is why a fire two
/// overmap tiles away does not burn on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bubble {
    /// The square the bubble is centred on, usually the player's.
    pub center: (i32, i32),
    /// How far from the centre the bubble reaches, in squares.
    pub radius: i32,
}

impl Bubble {
    /// The bubble the reference uses, centred on a square.
    #[must_use]
    pub fn around(center: (i32, i32)) -> Self {
        Self { center, radius: 66 }
    }

    /// Whether a square is inside it. The edge counts.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        (x - self.center.0).abs() <= self.radius && (y - self.center.1).abs() <= self.radius
    }
}

/// What a turn needs to know about the world rather than about a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnContext {
    /// The turn number, in turns since the start.
    pub now: i64,
    /// Wind strength where the fields are.
    pub windpower: i32,
    /// Wind direction, in degrees.
    pub wind_angle: i32,
    /// The part of the world that is simulated.
    pub bubble: Bubble,
}

/// What the map says about one square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellFacts {
    /// Whether a gas may spread into it.
    pub can_enter: bool,
    /// Whether it is under water, which makes fields age faster.
    pub swimmable: bool,
    /// Whether it is outdoors, which makes gases dissipate faster.
    pub outdoors: bool,
}

impl CellFacts {
    /// A square of open ground with nothing special about it.
    #[must_use]
    pub fn open() -> Self {
        Self {
            can_enter: true,
            swimmable: false,
            outdoors: false,
        }
    }
}

/// What one turn of field processing did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldTurnSummary {
    /// Fields that were looked at.
    pub processed: u32,
    /// Fields that aged away and were removed.
    pub removed: u32,
    /// Gases that got the chance to spread.
    pub spread_attempts: u32,
}

/// Runs one turn over every field in the world.
///
/// This is the assembly `docs/05-roadmap.md` calls for in P2: a field's own aging first, then the
/// processors the port has, in the order the reference runs them inside
/// `map::process_fields_in_submap` (`src/map_field.cpp:160-460`).
///
/// The order matters and is deliberate: decay, then outdoor dissipation (which is part of spreading),
/// then underwater dissipation, then the self-upgrade, then the spread attempt itself. A field that
/// aged away is gone before any of the later steps run, which is what the reference's `!is_alive()`
/// check does.
pub fn process_field_turn(
    world: &mut World,
    context: &TurnContext,
    data_of: impl Fn(FieldId) -> FieldTurnData,
    cell_of: impl Fn(i32, i32) -> CellFacts,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> FieldTurnSummary {
    let now = context.now;
    let mut summary = FieldTurnSummary::default();
    let mut order: Vec<(i32, i32, Entity)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &Position)>();
        for (entity, position) in query.iter(world) {
            order.push((position.x, position.y, entity));
        }
    }
    order.sort_by_key(|&(x, y, entity)| (x, y, entity.to_bits()));
    for (x, y, entity) in order {
        // Outside the bubble nothing happens at all: not even aging. A field out there keeps the
        // age it had, which is what the reference means by leaving it unloaded.
        if !context.bubble.contains(x, y) {
            continue;
        }
        let Some(field) = world.get::<Field>(entity) else {
            continue;
        };
        let id = field.id;
        // The map has the last word on water and sky: the prototype says how much faster a field
        // ages in each, the terrain says whether it is there at all.
        let cell = cell_of(x, y);
        let mut data = data_of(id);
        data.swimmable = cell.swimmable;
        data.outdoors = cell.outdoors;
        summary.processed += 1;
        // 1. The field's own clock.
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.do_decay(now, rolls);
        }
        // 2. Dissipation, both kinds, before anything else.
        process_underwater_dissipation(world, entity, data.swimmable, data.underwater_age_speedup);
        if !field_is_alive(world, entity) {
            world.despawn(entity);
            summary.removed += 1;
            continue;
        }
        // 3. A field that grows on its own.
        process_upgrade_intensity(
            world,
            entity,
            now,
            data.upgrade_chance,
            data.upgrade_duration,
            rolls,
        );
        // 4. The spread attempt, which also ages an outdoor gas.
        if data.percent_spread > 0 {
            summary.spread_attempts += 1;
            let gas = GasContext {
                percent_spread: data.percent_spread,
                windpower: context.windpower,
                wind_angle: context.wind_angle,
                outdoors: data.outdoors,
                outdoor_age_speedup: data.outdoor_age_speedup,
            };
            let enter = |x: i32, y: i32| cell_of(x, y).can_enter;
            spread_gas(world, entity, &gas, enter, rolls);
        }
        if !field_is_alive(world, entity) {
            world.despawn(entity);
            summary.removed += 1;
        }
    }
    summary
}

/// Whether a field entity still exists and still holds a living field.
fn field_is_alive(world: &World, entity: Entity) -> bool {
    world
        .get::<Field>(entity)
        .is_some_and(|field| field.entry.is_alive())
}

/// What decides whether two items on the ground become one stack.
///
/// Reference: `item::stacks_with` (`src/item.cpp:1693-1760`). The full rule also compares relics,
/// variants, contents, degradation, temperature, filth and activity; the port compares what its
/// ground items actually carry and the ledger names the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackFacts {
    /// What kind of item it is.
    pub kind: ItemTypeId,
    /// How many charges it holds.
    pub charges: i32,
    /// How much of it has burned away.
    pub burnt: i32,
    /// Whether the type is counted in charges rather than as separate objects.
    pub by_charges: bool,
    /// How damaged it is, as the reference's level.
    pub damage_level: i32,
}

/// Whether two of these would merge into one stack.
///
/// Two things of the same kind stack. Charges are ignored for a type counted in charges - that is
/// what makes a pile of rocks a pile - and must match exactly otherwise, so two partly-used
/// batteries never merge. Burnt amounts must agree.
#[must_use]
pub fn stacks_with(left: &StackFacts, right: &StackFacts) -> bool {
    left.kind == right.kind
        && left.burnt == right.burnt
        && left.damage_level == right.damage_level
        && (left.by_charges || left.charges == right.charges)
}

/// Drops items on a square, merging into an existing stack when the rule allows it.
///
/// Returns the entity that now holds them: the existing stack when the new charges joined it, or a
/// new one. Reference: `map::add_item` and the merge it performs through `item::stacks_with`.
pub fn drop_and_merge(world: &mut World, square: (i32, i32), facts: StackFacts) -> Entity {
    let here = items_at(world, square);
    for entity in here {
        let Some(existing) = world.get::<GroundItem>(entity) else {
            continue;
        };
        let existing_facts = StackFacts {
            kind: existing.kind,
            charges: existing.charges,
            burnt: existing.burnt,
            damage_level: existing.damage,
            by_charges: facts.by_charges,
        };
        if stacks_with(&existing_facts, &facts) {
            let charges = existing.charges + facts.charges;
            if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
                item.charges = charges;
            }
            return entity;
        }
    }
    drop_item(world, square, facts.kind, facts.charges)
}

/// How many charges of an item fit in a given volume.
///
/// Reference: `item::charges_per_volume` (`src/item.cpp:1391-1420`). For a type counted in charges,
/// one charge takes `volume / stack_size`, so the answer is `wanted * stack_size / volume`. For
/// anything else the answer is how many whole items fit. A type with no volume at all reports
/// `INT_MAX`, which is the reference's way of saying "no limit" without a sentinel of its own.
#[must_use]
pub fn charges_per_volume(
    wanted_ml: i64,
    item_volume_ml: i64,
    stack_size: i32,
    by_charges: bool,
) -> i32 {
    // The reference only reports "no limit" when the item has no volume. A wanted volume of zero
    // simply holds nothing, which is what its arithmetic says and what this returns.
    if item_volume_ml <= 0 {
        return i32::MAX;
    }
    if wanted_ml <= 0 {
        return 0;
    }
    let fits = if by_charges {
        wanted_ml.saturating_mul(i64::from(stack_size.max(1))) / item_volume_ml
    } else {
        wanted_ml / item_volume_ml
    };
    i32::try_from(fits).unwrap_or(i32::MAX)
}

/// How damaged an item is, as one of six levels.
///
/// Reference: `itype::damage_level` (`src/itype.cpp:117-126`) and `item::damage_level` above it. An
/// undamaged item is level 0. A type counted in charges has no partial damage at all - any damage
/// puts it at level 5, which is how a leaking bottle reads as ruined rather than scratched. Anything
/// else is scaled against the type's own maximum: a quarter of the way through its damage is level 2,
/// and the scale saturates at 5.
#[must_use]
pub fn damage_level(damage: i32, damage_max: i32, by_charges: bool) -> i32 {
    if damage == 0 {
        return 0;
    }
    if by_charges {
        return 5;
    }
    if damage_max <= 0 {
        return 5;
    }
    (1 + 4 * damage / damage_max).clamp(0, 5)
}

/// A thing that holds other things: a box, a bottle, a backpack.
///
/// Reference: `item::is_container` and the pocket capacity check in `item::can_contain`
/// (`src/item.cpp`). The reference's pockets each have their own volume, weight and length limits
/// and an allow-list of what they accept; the port keeps one capacity per container and grows
/// pockets when something needs them.
#[derive(Debug, Clone, Copy, Component)]
pub struct Container {
    /// How much volume it can hold, in millilitres.
    pub capacity_ml: i64,
}

/// Marks an item as being inside a container.
#[derive(Debug, Clone, Copy, Component)]
pub struct Inside(pub Entity);

/// Why something could not be put in a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertError {
    /// The container has no room for it.
    TooBig {
        /// Room it had left, in millilitres.
        remaining_ml: i64,
        /// What it was asked to take, in millilitres.
        wanted_ml: i64,
    },
    /// The item has already been put somewhere else.
    AlreadyInside,
}

/// Puts an item into a container, if there is room.
///
/// The volume of what is already inside is asked of `volume_of`, because an item's volume belongs to
/// its prototype and the sim does not own prototypes.
///
/// # Errors
///
/// Returns [`InsertError::TooBig`] when the contents would exceed the capacity, and
/// [`InsertError::AlreadyInside`] when the item is already in a container.
pub fn insert_into(
    world: &mut World,
    container: Entity,
    item: Entity,
    volume_of: impl Fn(Entity) -> i64,
) -> Result<(), InsertError> {
    if world.get::<Inside>(item).is_some() {
        return Err(InsertError::AlreadyInside);
    }
    let Some(capacity) = world.get::<Container>(container).map(|c| c.capacity_ml) else {
        return Err(InsertError::TooBig {
            remaining_ml: 0,
            wanted_ml: volume_of(item),
        });
    };
    let mut used = 0;
    for (entity, inside) in world.query::<(Entity, &Inside)>().iter(world) {
        if inside.0 == container {
            used += volume_of(entity);
        }
    }
    let wanted = volume_of(item);
    let remaining = capacity - used;
    if wanted > remaining {
        return Err(InsertError::TooBig {
            remaining_ml: remaining,
            wanted_ml: wanted,
        });
    }
    world.entity_mut(item).insert(Inside(container));
    Ok(())
}

/// Everything inside a container, in insertion order.
#[must_use]
pub fn contents(world: &mut World, container: Entity) -> Vec<Entity> {
    let mut found = Vec::new();
    for (entity, inside) in world.query::<(Entity, &Inside)>().iter(world) {
        if inside.0 == container {
            found.push(entity);
        }
    }
    found.sort_by_key(|entity| entity.index());
    found
}

/// Takes an item out of whatever holds it.
pub fn take_out(world: &mut World, item: Entity) {
    world.entity_mut(item).remove::<Inside>();
}

/// What an item weighs, contents included.
///
/// Reference: `item::weight` (`src/item.cpp:7229-7300`). The base weight comes from the prototype, an
/// item counted in charges weighs that much per charge, and **what is inside a container counts** -
/// that is why the reference takes `include_contents` and why a full box is heavier than an empty one.
/// The recursion is bounded by the tree of contents, which cannot be cyclic because an item can only be
/// in one place at once.
#[must_use]
pub fn item_weight(
    world: &mut World,
    entity: Entity,
    base_grams: impl Fn(ItemTypeId) -> i64,
) -> i64 {
    weight_with(world, entity, &base_grams)
}

/// The recursion itself, taking the lookup as a trait object so it does not monomorphise per depth.
fn weight_with(world: &mut World, entity: Entity, base_grams: &dyn Fn(ItemTypeId) -> i64) -> i64 {
    let Some(item) = world.get::<GroundItem>(entity).copied() else {
        return 0;
    };
    let charges = i64::from(item.charges.max(1));
    let mut total = base_grams(item.kind).saturating_mul(charges);
    for held in contents(world, entity) {
        total = total.saturating_add(weight_with(world, held, base_grams));
    }
    total
}

/// What burning a portion of an item leaves behind.
///
/// Reference: `map::create_burnproducts` (`src/map_field.cpp:116-143`). For each material the item is
/// made of, each of that material's burn products is spawned at
/// `floor(efficiency * burned / product_weight * portion / total_portions)`, skipping a product that is
/// the item itself - the reference notes that would recurse forever. A count of zero or less leaves
/// nothing behind.
#[must_use]
pub fn burn_products(
    portions: &[(String, f64)],
    burned_grams: i64,
    material_total: i32,
    product_weight_grams: impl Fn(&str) -> i64,
    fuel_kind: &str,
) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    for (product, efficiency) in portions {
        if product == fuel_kind {
            continue;
        }
        let weight = product_weight_grams(product);
        if weight <= 0 {
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        let share = f64::from(material_total.max(1));
        #[allow(clippy::cast_precision_loss)]
        let burned = burned_grams as f64;
        #[allow(clippy::cast_precision_loss)]
        let per_item = weight as f64;
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let count = (efficiency * (burned / per_item) * (1.0 / share)).floor() as i32;
        if count > 0 {
            out.push((product.clone(), count));
        }
    }
    out
}

/// What came of asking a square for some items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consumed {
    /// How much was taken: charges for a stack, objects otherwise.
    pub taken: i32,
    /// Whether the request was met in full.
    pub satisfied: bool,
}

/// Takes up to `quantity` of one kind of item from a square.
///
/// Reference: `map::use_amount_square` with `use_amount_stack` and `item::use_charges`
/// (`src/map.cpp:5863-5897`, `src/item.cpp:13135-13180`). Items are taken in the order they lie there -
/// which is spawn order, and therefore part of the deterministic contract - and a stack that runs out is
/// removed. The request can come up short: the caller learns both how much it got and whether that was
/// everything it asked for, because crafting needs to put back what it took when it fails.
pub fn consume_items(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    quantity: i32,
) -> Consumed {
    let mut remaining = quantity.max(0);
    let mut taken = 0;
    for entity in items_at(world, square) {
        if remaining == 0 {
            break;
        }
        let Some(item) = world.get::<GroundItem>(entity).copied() else {
            continue;
        };
        if item.kind != kind {
            continue;
        }
        let available = item.charges.max(0);
        let take = available.min(remaining);
        if take == 0 {
            continue;
        }
        remaining -= take;
        taken += take;
        if take == available {
            world.despawn(entity);
        } else if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
            item.charges = available - take;
        }
    }
    Consumed {
        taken,
        satisfied: remaining == 0,
    }
}

/// Why a craft did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftError {
    /// A requirement group could not be met; nothing was taken.
    Missing {
        /// Which group failed, counting from zero.
        group: usize,
    },
}

/// What a craft produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crafted {
    /// The item that appeared on the square.
    pub result: Entity,
    /// How many of it.
    pub count: i32,
}

/// How many of one kind lie on a square.
#[must_use]
pub fn count_on_square(world: &mut World, square: (i32, i32), kind: ItemTypeId) -> i32 {
    items_at(world, square)
        .into_iter()
        .filter_map(|entity| world.get::<GroundItem>(entity).copied())
        .filter(|item| item.kind == kind)
        .map(|item| item.charges.max(0))
        .sum()
}

/// Makes something from what is lying on a square, all or nothing.
///
/// Reference: `Character::craft` and the consumption it performs (`src/craft.cpp`, the component
/// handling around `consume_components`). Three things the port keeps: **availability is checked before
/// anything is taken** - the reference's `has_comps` only asks - alternatives are tried in the order the
/// recipe lists them, and components are consumed before the result appears, so a failure leaves the
/// square exactly as it was.
///
/// # Errors
///
/// Returns [`CraftError::Missing`] naming the first group that could not be met. Nothing has been consumed
/// when that happens.
pub fn craft_on_square(
    world: &mut World,
    square: (i32, i32),
    components: &[Vec<(ItemTypeId, i32)>],
    batch: i32,
    result_kind: ItemTypeId,
) -> Result<Crafted, CraftError> {
    let batch = batch.max(1);
    let mut chosen: Vec<(ItemTypeId, i32)> = Vec::new();
    for (group, alternatives) in components.iter().enumerate() {
        let mut taken = None;
        for (kind, count) in alternatives {
            let wanted = count.saturating_mul(batch);
            if count_on_square(world, square, *kind) >= wanted {
                taken = Some((*kind, wanted));
                break;
            }
        }
        match taken {
            Some(choice) => chosen.push(choice),
            None => return Err(CraftError::Missing { group }),
        }
    }
    // Everything is known to be there, so this cannot come up short; if it ever does, the parts are put
    // back rather than the craft half happening.
    let mut spent: Vec<(ItemTypeId, i32)> = Vec::new();
    for (kind, wanted) in &chosen {
        let consumed = consume_items(world, square, *kind, *wanted);
        spent.push((*kind, consumed.taken));
        if !consumed.satisfied {
            for (kind, count) in spent.iter().rev() {
                drop_item(world, square, *kind, *count);
            }
            return Err(CraftError::Missing {
                group: cost_group(components, *kind),
            });
        }
    }
    let entity = drop_item(world, square, result_kind, batch);
    Ok(Crafted {
        result: entity,
        count: batch,
    })
}

/// Which requirement group names a kind, for the error path above.
fn cost_group(components: &[Vec<(ItemTypeId, i32)>], kind: ItemTypeId) -> usize {
    components
        .iter()
        .position(|alternatives| alternatives.iter().any(|(candidate, _)| *candidate == kind))
        .unwrap_or(0)
}

/// How many moves a batch takes to craft, once the crafter's speed is known.
///
/// Reference: `recipe::batch_time` (`src/recipe.cpp:108-149`). Everything about this is fiddly and every
/// part of it is load-bearing:
///
/// * a speed multiplier of zero means "cannot craft here", and the reference reports the time as if the
///   crafter were at full speed rather than dividing by zero;
/// * a recipe that does not benefit from batching takes the whole time per unit;
/// * one that does follows a scaled logistic curve, which is why a batch of twenty is far less than twenty
///   times one - the per-unit saving deepens with the batch and flattens out at `batch_rscale`;
/// * assistants take a fixed cut, one giving three quarters and two or more giving three fifths;
/// * and no arrangement ever makes a batch faster than a single unit.
#[must_use]
pub fn batch_time_moves(
    time_moves: i64,
    batch: i32,
    batch_rscale: f64,
    batch_rsize: f64,
    multiplier: f64,
    assistants: usize,
) -> i64 {
    let multiplier = if multiplier == 0.0 { 1.0 } else { multiplier };
    #[allow(clippy::cast_precision_loss)]
    let local_time = time_moves as f64 / multiplier;
    if batch_rscale == 0.0 && assistants == 0 {
        #[allow(clippy::cast_possible_truncation)]
        return (local_time as i64) * i64::from(batch.max(1));
    }
    let mut total = 0.0;
    if batch_rscale == 0.0 {
        total = local_time * f64::from(batch.max(1));
    } else {
        let scale = batch_rsize / 6.0;
        for unit in 0..batch.max(1) {
            #[allow(clippy::cast_precision_loss)]
            let step = f64::from(unit) / scale;
            let logf = (2.0 / (1.0 + (-step).exp())) - 1.0;
            total += local_time * (1.0 - (batch_rscale * logf));
        }
    }
    match assistants {
        1 => total *= 0.75,
        count if count >= 2 => total *= 0.60,
        _ => {}
    }
    if total < local_time {
        total = local_time;
    }
    #[allow(clippy::cast_possible_truncation)]
    let total = total as i64;
    total
}

/// A craft in progress: how much work it needs and how much has been done.
///
/// Reference: `craft_activity_actor::do_turn` (`src/activity_actor.cpp:4163-4240`). The reference stores
/// progress as a percentage of the *base* batch time in a fixed-point field and scales each turn's moves by
/// `base_total / current_total`, so a slow crafter accrues less progress per turn rather than needing a
/// different denominator. The port keeps the same relationship in plain moves, which is easier to read and
/// identical in behaviour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraftingWork {
    /// Moves the craft needs with no speed modifier, as `recipe::batch_time` reports for full speed.
    pub base_total_moves: f64,
    /// Moves it needs at the current speed and with the current assistants.
    pub current_total_moves: f64,
    /// Moves of base work done so far.
    pub progress: f64,
}

/// How a craft turn went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkOutcome {
    /// Still going.
    Working,
    /// It is done this turn.
    Finished,
    /// The crafter cannot work here at all, so the craft is abandoned.
    Cancelled,
}

impl CraftingWork {
    /// A craft of `base_total_moves`, starting at the same speed.
    #[must_use]
    pub fn new(base_total_moves: f64) -> Self {
        Self {
            base_total_moves: base_total_moves.max(1.0),
            current_total_moves: base_total_moves.max(1.0),
            progress: 0.0,
        }
    }

    /// How far along it is, from zero to one.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        (self.progress / self.base_total_moves).clamp(0.0, 1.0)
    }
}

/// Spends one turn of work on a craft.
///
/// A speed of zero or less cancels it, which is the reference's own answer when nothing about the place
/// allows crafting. Progress never passes the total, so a slow turn at the end does not overshoot.
pub fn craft_turn(work: &mut CraftingWork, moves: i64, speed: f64) -> WorkOutcome {
    if speed <= 0.0 {
        return WorkOutcome::Cancelled;
    }
    #[allow(clippy::cast_precision_loss)]
    let spent = moves.max(0) as f64;
    let delta = spent * work.base_total_moves / work.current_total_moves.max(1.0);
    work.progress = (work.progress + delta).min(work.base_total_moves);
    if work.progress >= work.base_total_moves {
        WorkOutcome::Finished
    } else {
        WorkOutcome::Working
    }
}

/// The kind an unfinished craft carries until it becomes its result.
pub const UNFINISHED_CRAFT: ItemTypeId = ItemTypeId(u32::MAX);

/// Marks the item that holds a craft in progress, together with what it will become.
///
/// Reference: `craft_activity_actor` (`src/activity_actor.cpp:4163-4335`). The reference's in-progress craft
/// is an item: it **holds the components from the moment they are taken**, which is why cancelling a craft
/// loses nothing - the unfinished item stays where it is with its contents inside, and only an empty
/// componentless recipe item is removed outright.
#[derive(Debug, Clone, Copy, Component)]
pub struct CraftInProgress {
    /// What it will become.
    pub result_kind: ItemTypeId,
    /// How many at once.
    pub batch: i32,
    /// Where the work stands.
    pub work: CraftingWork,
}

/// How a craft ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftEnd {
    /// Still working.
    Working,
    /// Finished: the result is on the square and this is it.
    Made(Entity),
    /// Cancelled: the parts are still inside the unfinished craft.
    Cancelled,
}

/// Starts a craft, taking the parts into an unfinished item on the square.
///
/// # Errors
///
/// Returns [`CraftError::Missing`] when a requirement group cannot be met; nothing is taken then.
pub fn start_craft(
    world: &mut World,
    square: (i32, i32),
    components: &[Vec<(ItemTypeId, i32)>],
    batch: i32,
    result_kind: ItemTypeId,
    base_total_moves: f64,
) -> Result<Entity, CraftError> {
    // Take the parts first, exactly as `craft_on_square` does, and keep them where they came from.
    let mut parts: Vec<(ItemTypeId, i32)> = Vec::new();
    for (group, alternatives) in components.iter().enumerate() {
        let mut chosen = None;
        for (kind, count) in alternatives {
            let wanted = count.saturating_mul(batch.max(1));
            if count_on_square(world, square, *kind) >= wanted {
                chosen = Some((*kind, wanted));
                break;
            }
        }
        if let Some(part) = chosen {
            parts.push(part);
        } else {
            for (kind, count) in parts.iter().rev() {
                drop_item(world, square, *kind, *count);
            }
            return Err(CraftError::Missing { group });
        }
    }
    for (kind, wanted) in &parts {
        consume_items(world, square, *kind, *wanted);
    }
    // An unfinished craft is an item that holds its parts, so nothing can be lost by cancelling it. It
    // carries a sentinel kind until it is finished, so nothing can mistake it for the result.
    let craft = drop_item(world, square, UNFINISHED_CRAFT, 1);
    world
        .entity_mut(craft)
        .insert(Container {
            capacity_ml: i64::MAX,
        })
        .insert(CraftInProgress {
            result_kind,
            batch: batch.max(1),
            work: CraftingWork::new(base_total_moves),
        });
    for (kind, count) in parts {
        let part = drop_item(world, square, kind, count);
        // Capacity is unbounded, so this cannot fail.
        let _ = insert_into(world, craft, part, |_| 0);
    }
    Ok(craft)
}

/// Spends one turn on a craft.
pub fn craft_turn_on(world: &mut World, craft: Entity, moves: i64, speed: f64) -> CraftEnd {
    let Some(mut in_progress) = world.get::<CraftInProgress>(craft).copied() else {
        return CraftEnd::Cancelled;
    };
    let outcome = craft_turn(&mut in_progress.work, moves, speed);
    if let Some(mut component) = world.get_mut::<CraftInProgress>(craft) {
        component.work = in_progress.work;
    }
    match outcome {
        WorkOutcome::Working => CraftEnd::Working,
        WorkOutcome::Cancelled => CraftEnd::Cancelled,
        WorkOutcome::Finished => {
            let square = world
                .get::<Position>(craft)
                .map_or((0, 0), |position| (position.x, position.y));
            // The parts are consumed and the unfinished item becomes the result.
            for held in contents(world, craft) {
                world.despawn(held);
            }
            world.despawn(craft);
            CraftEnd::Made(drop_item(
                world,
                square,
                in_progress.result_kind,
                in_progress.batch,
            ))
        }
    }
}

/// Cancels a craft. The unfinished item stays where it is with its parts inside it.
pub fn cancel_craft(world: &mut World, craft: Entity) {
    world.entity_mut(craft).remove::<CraftInProgress>();
}

/// Why something could not be eaten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EatError {
    /// Nothing of that kind is on the square.
    NotThere,
    /// It is not food: the reference's comestibles are the only things with a nutrition value.
    NotFood,
    /// There is some, but not a full serving.
    NotEnoughCharges {
        /// What was there.
        available: i32,
        /// What one serving costs.
        needed: i32,
    },
}

/// What is left of a meal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meal {
    /// Nutrition gained, in the reference's own units.
    pub nutr: i32,
    /// Thirst quenched.
    pub quench: i32,
    /// Charges spent.
    pub charges: i32,
}

/// Eats one serving of something on a square.
///
/// Reference: the comestible path of `Character::consume` and `itype::charges_to_use`
/// (`src/itype.cpp:151-159`): **one serving spends `charges_to_use` charges** - which is one for food, so
/// the rule only shows when it is not - and the nutrition gained is the type's per serving. The port keeps
/// the spending and the accounting and leaves hunger, thirst and the eating time to the systems that own
/// them.
///
/// `nutrition_of` answers with `(nutr, quench, charges_to_use)` for a kind.
///
/// # Errors
///
/// Returns [`EatError`] when there is nothing there, it is not food, or there are not enough charges for a
/// full serving. Nothing is consumed when it fails.
pub fn eat(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    nutrition_of: impl Fn(ItemTypeId) -> (i32, i32, i32),
) -> Result<Meal, EatError> {
    let (nutr, quench, charges_to_use) = nutrition_of(kind);
    if nutr == 0 && quench == 0 {
        return Err(EatError::NotFood);
    }
    let charges_to_use = charges_to_use.max(1);
    let available = count_on_square(world, square, kind);
    if available == 0 {
        return Err(EatError::NotThere);
    }
    if available < charges_to_use {
        return Err(EatError::NotEnoughCharges {
            available,
            needed: charges_to_use,
        });
    }
    consume_items(world, square, kind, charges_to_use);
    Ok(Meal {
        nutr,
        quench,
        charges: charges_to_use,
    })
}

/// What a character has eaten and drunk, in the reference's units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct MealTally {
    /// Nutrition taken in.
    pub nutr: i32,
    /// Thirst quenched.
    pub quench: i32,
}

impl MealTally {
    /// Adds a meal to the tally.
    pub fn eat(&mut self, meal: Meal) {
        self.nutr = self.nutr.saturating_add(meal.nutr);
        self.quench = self.quench.saturating_add(meal.quench);
    }
}

/// One square of what a player would see.
///
/// The shape mirrors `--dump-frame` in the oracle (`oracle/harness/harness.cpp`, `dump_frame`), so the two
/// can be compared square by square: the glyph the interface would draw, the terrain under it, how many
/// items lie there, and the light.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSquare {
    /// Where it is.
    pub x: i32,
    /// Where it is.
    pub y: i32,
    /// What the interface draws: an item over furniture over terrain, which is the reference's order.
    pub glyph: char,
    /// The terrain's id.
    pub terrain: u32,
    /// How many items lie there.
    pub items: i32,
    /// Ambient light at the square.
    pub light: f32,
}

/// What a square looks like before anything is drawn on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SquareAppearance {
    /// The terrain's own glyph.
    pub glyph: char,
    /// The furniture's glyph, when something is standing there.
    pub furniture: Option<char>,
}

/// Draws the view around a centre.
///
/// This is the port's half of the frame gate: nothing here is a game rule, and everything here is a choice
/// about what a player is shown. The order is the reference's - an item hides furniture, furniture hides the
/// floor - because a diff against the oracle is worthless if the two disagree about precedence.
#[must_use]
pub fn render_frame(
    world: &mut World,
    level: &cdda_core_types::level::Level,
    centre: (i32, i32),
    radius: i32,
    appearance_of: impl Fn(u32) -> SquareAppearance,
    light_at: impl Fn(i32, i32) -> f32,
) -> Vec<FrameSquare> {
    let mut squares = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (x, y) = (centre.0 + dx, centre.1 + dy);
            let terrain = level.terrain_at(x, y).unwrap_or(0);
            let appearance = appearance_of(terrain);
            let here = items_at(world, (x, y));
            let mut glyph = appearance.furniture.unwrap_or(appearance.glyph);
            if !here.is_empty() {
                // An item on the floor covers whatever is under it. Its glyph belongs to its prototype,
                // which the caller owns, so the renderer marks the square and the caller draws it.
                glyph = '*';
            }
            squares.push(FrameSquare {
                x,
                y,
                glyph,
                terrain,
                items: i32::try_from(here.len()).unwrap_or(i32::MAX),
                // Light comes from the lighting system, which owns it; the renderer only asks.
                light: light_at(x, y),
            });
        }
    }
    squares
}

/// What stepping onto a square costs, in moves.
///
/// Reference: `monmove.cpp` uses `50 * here.move_cost( p )` for the cost of leaving a square. Default
/// terrain has a move cost of 2, so an ordinary step costs 100 moves - one turn at normal speed.
#[must_use]
pub fn step_cost(terrain_move_cost: i32) -> i32 {
    50 * terrain_move_cost
}

/// A creature's moves at the start of its turn: what it had, plus its speed.
///
/// Reference: `Creature::get_speed` is what a turn grants (`monster::process_turn`, `mod_moves( get_speed() )`).
/// Saturating because a pathological speed should slow a creature down, not wrap it into negative moves.
#[must_use]
pub fn regen_moves(moves: i32, speed: i32) -> i32 {
    moves.saturating_add(speed)
}

/// Whether a creature has moves left to act.
///
/// Reference: an action is taken while moves remain (`monster::move`), so zero moves means waiting.
#[must_use]
pub fn can_act(moves: i32) -> bool {
    moves > 0
}

/// Whether a creature is next to a square, which is where it can reach to attack.
///
/// Reference: `Creature::is_adjacent` (`creature.cpp`), which asks `rl_dist( pos, target ) != 1` - so the four
/// sides and the four diagonals count, and a creature standing on the square does not.
#[must_use]
pub fn is_adjacent(from: (i32, i32), to: (i32, i32)) -> bool {
    let dx = (from.0 - to.0).abs();
    let dy = (from.1 - to.1).abs();
    dx.max(dy) == 1
}

/// One step toward a square, which is where an idle creature walks.
///
/// Reference: `monster::move` falls back to stepping at the player when there is nothing better to do; the step
/// is one square in each axis that is not already aligned.
#[must_use]
pub fn step_toward(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    let step = |delta: i32| match delta.cmp(&0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };
    (from.0 + step(to.0 - from.0), from.1 + step(to.1 - from.1))
}

/// What a creature does with its turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Hit what is next to it.
    Attack,
    /// Walk one square toward the target.
    Step((i32, i32)),
    /// Nothing to do.
    Wait,
}

/// Chooses an action: attack what is next to it, otherwise close the distance, otherwise wait.
///
/// Reference: `monster::move` (`monmove.cpp`), whose first branch is a melee attack on an adjacent target and
/// whose fallback is a step toward the player. This is the ladder without the refinements - no pathfinding, no
/// fleeing, no special attacks - because those are separate rules with separate tests.
#[must_use]
pub fn choose_action(from: (i32, i32), target: (i32, i32)) -> Action {
    if from == target {
        return Action::Wait;
    }
    if is_adjacent(from, target) {
        return Action::Attack;
    }
    Action::Step(step_toward(from, target))
}

/// What one creature did with its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnOutcome {
    /// Every action taken, in order.
    pub actions: Vec<Action>,
    /// Where the creature ended up.
    pub position: (i32, i32),
    /// Moves left over, which a faster creature can keep for later.
    pub moves: i32,
    /// How many times it hit the target.
    pub hits: i32,
}

/// Gives a creature its turn: it acts while it can pay for acting, and stops when it cannot.
///
/// Reference: `monster::move` (`monmove.cpp`) charges **the creature's own speed** for an action
/// (`mod_moves( -get_speed() )`), not a flat 100 - so a slow creature pays less per action and acts more often
/// within the same number of moves, and a fast one pays more. That is easy to get backwards, so it is written
/// down here and pinned by a test.
#[must_use]
pub fn take_turn(
    position: (i32, i32),
    target: (i32, i32),
    mut moves: i32,
    speed: i32,
) -> TurnOutcome {
    let mut actions = Vec::new();
    let mut at = position;
    let mut hits = 0;
    // A creature that cannot pay for an action does not get a free one, and speed <= 0 would loop forever.
    while speed > 0 && moves >= speed {
        let action = choose_action(at, target);
        if action == Action::Wait {
            break;
        }
        match action {
            Action::Attack => hits += 1,
            Action::Step(to) => at = to,
            Action::Wait => break,
        }
        actions.push(action);
        moves -= speed;
    }
    TurnOutcome {
        actions,
        position: at,
        moves,
        hits,
    }
}

/// A turn taken in a world that can refuse a step.
///
/// Reference: `monster::move_to` (`monmove.cpp`) returns **false and charges nothing** when the destination is
/// blocked by terrain or by another creature - the moves are only taken after the checks
/// (`mod_moves( -ceil( cost ) )` sits below them). So a monster that walks into a wall wastes nothing; it simply
/// does not move, which is why a blocked monster still has its moves next turn.
#[must_use]
pub fn take_turn_blocked(
    position: (i32, i32),
    target: (i32, i32),
    mut moves: i32,
    speed: i32,
    passable: impl Fn((i32, i32)) -> bool,
) -> TurnOutcome {
    let mut actions = Vec::new();
    let mut at = position;
    let mut hits = 0;
    while speed > 0 && moves >= speed {
        match choose_action(at, target) {
            Action::Wait => break,
            Action::Attack => {
                hits += 1;
                actions.push(Action::Attack);
                moves -= speed;
            }
            Action::Step(to) => {
                if !passable(to) {
                    // Blocked: the step never happens and costs nothing, so the turn ends here.
                    break;
                }
                at = to;
                actions.push(Action::Step(to));
                moves -= speed;
            }
        }
    }
    TurnOutcome {
        actions,
        position: at,
        moves,
        hits,
    }
}

/// Whether anything alive stands on a square.
///
/// This is the occupancy question a moving creature asks, and it is answered from the World rather than from a
/// list kept alongside it: a second copy of where things are is a copy that can disagree.
#[must_use]
pub fn occupied(world: &mut World, square: (i32, i32)) -> bool {
    let mut query = world.query::<(&Position, &Monster)>();
    query
        .iter(world)
        .any(|(position, monster)| (position.x, position.y) == square && monster.hp > 0)
}

/// Rounds a creature's moves to the whole number the turn loop works in.
fn whole_moves(moves: f64) -> i32 {
    let rounded = moves.round();
    if rounded >= f64::from(i32::MAX) {
        i32::MAX
    } else if rounded <= f64::from(i32::MIN) {
        i32::MIN
    } else {
        // Inside the range above, so the cast cannot lose anything the caller would notice.
        #[allow(clippy::cast_possible_truncation)]
        let whole = rounded as i32;
        whole
    }
}

/// One creature's state when its turn starts, gathered before anything moves.
struct CreatureTurn {
    /// Which thing it is.
    entity: Entity,
    /// Where it stands.
    start: (i32, i32),
    /// Moves it has this turn.
    moves: i32,
    /// What kind it is, which decides its speed.
    kind: MonsterKindId,
}

/// Gives every creature in the world a turn, in the order the world lists them.
///
/// Reference: the reference asks each creature to act in turn and a creature may not step onto a square another
/// creature occupies (`monster::move_to`, which refuses when `creature_at( destination )` is not null). Order
/// matters and is therefore the World's own order rather than a hash order, so a replay is a replay.
///
/// `speed_of` supplies a creature's speed because speed belongs to its kind, which the content owns.
pub fn monster_turns(
    world: &mut World,
    target: (i32, i32),
    speed_of: impl Fn(MonsterKindId) -> i32,
    passable: impl Fn(u32) -> bool,
    level: &cdda_core_types::level::Level,
) {
    let mut occupied_squares: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(&Position, &Monster)>();
        query
            .iter(world)
            .filter(|(_, monster)| monster.hp > 0)
            .map(|(position, _)| (position.x, position.y))
            .collect()
    };
    let creatures: Vec<CreatureTurn> = {
        let mut query = world.query::<(Entity, &Position, &Monster)>();
        query
            .iter(world)
            .filter(|(_, _, monster)| monster.hp > 0)
            .map(|(entity, position, monster)| CreatureTurn {
                entity,
                start: (position.x, position.y),
                // Moves are whole in practice. The conversion is explicit about what happens at the ends
                // rather than letting a cast decide.
                moves: whole_moves(monster.moves),
                kind: monster.kind,
            })
            .collect()
    };
    for CreatureTurn {
        entity,
        start,
        moves,
        kind,
    } in creatures
    {
        let speed = speed_of(kind);
        // The creature's own square is passable to itself, and another creature's is not.
        let can_enter = |square: (i32, i32)| {
            if square == start {
                return true;
            }
            let terrain_ok = level.terrain_at(square.0, square.1).is_some_and(&passable);
            terrain_ok && !occupied_squares.contains(&square)
        };
        let turn = take_turn_blocked(start, target, moves, speed, can_enter);
        if let Some(mut monster) = world.get_mut::<Monster>(entity) {
            monster.moves = f64::from(turn.moves);
        }
        if turn.position != start {
            occupied_squares.remove(&start);
            occupied_squares.insert(turn.position);
            if let Some(mut position) = world.get_mut::<Position>(entity) {
                position.x = turn.position.0;
                position.y = turn.position.1;
            }
        }
    }
}

/// What a creature's blow does before armour: its dice, rolled.
///
/// Reference: `monster::melee_attack` (`monster.cpp`), which adds bash damage of
/// `dice( type->melee_dice, type->melee_sides )` when the kind has any dice at all. The roll comes from the
/// stream, so its order matters: a port that rolls at a different moment produces a different fight from the
/// same seed.
pub fn melee_blow(
    rolls: &mut cdda_core_types::rng::Rolls,
    dice: i32,
    sides: i32,
    bonus: i32,
) -> i32 {
    if dice <= 0 || sides <= 0 {
        return bonus.max(0);
    }
    let mut total = bonus;
    for _ in 0..dice {
        total += rolls.rng(1, sides);
    }
    total.max(0)
}

/// What gets through: the blow, less armour, never below nothing.
///
/// Reference: armour reduces each damage type before it lands (`Creature::deal_damage`/`absorb_hit`); a blow
/// smaller than the armour does nothing rather than healing the target.
#[must_use]
pub fn through_armour(blow: i32, armour: i32) -> i32 {
    (blow - armour).max(0)
}

/// Applies a blow to a creature, and says whether that killed it.
///
/// Reference: hit points reach zero and the creature dies (`monster::die`). The port keeps the death as a
/// property of the state rather than removing the entity here, because removal is a system's decision and a
/// caller may want the corpse's square.
pub fn strike(world: &mut World, target: Entity, blow: i32, armour: i32) -> bool {
    let dealt = through_armour(blow, armour);
    let Some(mut monster) = world.get_mut::<Monster>(target) else {
        return false;
    };
    monster.hp = monster.hp.saturating_sub(dealt);
    monster.hp <= 0
}

/// The rules a creature's turn needs, gathered so the turn functions do not grow a parameter per fact.
pub struct TurnRules<'a> {
    /// How fast a kind is, which decides what an action costs it.
    pub speed_of: &'a dyn Fn(MonsterKindId) -> i32,
    /// A kind's blow: its dice, its sides and its bonus.
    pub blow_of: &'a dyn Fn(MonsterKindId) -> (i32, i32, i32),
    /// A kind's armour, which reduces what lands on it.
    pub armour_of: &'a dyn Fn(MonsterKindId) -> i32,
    /// Whether the terrain lets a creature through.
    pub passable: &'a dyn Fn(u32) -> bool,
}

/// Gives every creature a turn, and lets its attacks land on whoever is standing where it strikes.
///
/// This is where two rules meet: the turn loop decides that a creature attacks, and the damage rule decides what
/// that costs the target. Keeping the dice in the caller's hands matters - the reference rolls from one stream, so
/// a port that gave each system its own dice would produce a different fight from the same seed.
///
/// Returns how many blows landed.
pub fn monster_turns_striking(
    world: &mut World,
    target: (i32, i32),
    rules: &TurnRules<'_>,
    rolls: &mut cdda_core_types::rng::Rolls,
    level: &cdda_core_types::level::Level,
) -> i32 {
    let mut occupied_squares: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(&Position, &Monster)>();
        query
            .iter(world)
            .filter(|(_, monster)| monster.hp > 0)
            .map(|(position, _)| (position.x, position.y))
            .collect()
    };
    let creatures: Vec<CreatureTurn> = {
        let mut query = world.query::<(Entity, &Position, &Monster)>();
        query
            .iter(world)
            .filter(|(_, _, monster)| monster.hp > 0)
            .map(|(entity, position, monster)| CreatureTurn {
                entity,
                start: (position.x, position.y),
                moves: whole_moves(monster.moves),
                kind: monster.kind,
            })
            .collect()
    };
    let mut landed = 0;
    for CreatureTurn {
        entity,
        start,
        moves,
        kind,
    } in creatures
    {
        let speed = (rules.speed_of)(kind);
        let can_enter = |square: (i32, i32)| {
            if square == start {
                return true;
            }
            let terrain_ok = level
                .terrain_at(square.0, square.1)
                .is_some_and(rules.passable);
            terrain_ok && !occupied_squares.contains(&square)
        };
        let turn = take_turn_blocked(start, target, moves, speed, can_enter);
        // Every hit the turn produced lands on whoever stands on the target square.
        for _ in 0..turn.hits {
            let victim = {
                let mut monsters = world.query::<(Entity, &Position, &Monster)>();
                let found = monsters
                    .iter(world)
                    .find(|(_, position, monster)| {
                        (position.x, position.y) == target && monster.hp > 0
                    })
                    .map(|(victim, _, _)| victim);
                found.map_or_else(
                    || {
                        let mut players = world.query::<(Entity, &Position, &Player)>();
                        players
                            .iter(world)
                            .find(|(_, position, player)| {
                                (position.x, position.y) == target && player.hp > 0
                            })
                            .map(|(victim, _, _)| (victim, true))
                    },
                    |entity| Some((entity, false)),
                )
            };
            let Some((victim, is_player)) = victim else {
                break;
            };
            let (dice, sides, bonus) = (rules.blow_of)(kind);
            let blow = melee_blow(rolls, dice, sides, bonus);
            if is_player {
                strike_player(world, victim, blow);
            } else {
                let armour = world
                    .get::<Monster>(victim)
                    .map_or(0, |monster| (rules.armour_of)(monster.kind));
                strike(world, victim, blow, armour);
            }
            landed += 1;
        }
        if let Some(mut monster) = world.get_mut::<Monster>(entity) {
            monster.moves = f64::from(turn.moves);
        }
        if turn.position != start {
            occupied_squares.remove(&start);
            occupied_squares.insert(turn.position);
            if let Some(mut position) = world.get_mut::<Position>(entity) {
                position.x = turn.position.0;
                position.y = turn.position.1;
            }
        }
    }
    landed
}

/// The player, as a thing in the world rather than a pair of coordinates.
///
/// Reference: `avatar` (`src/avatar.h`) is a `Character`, which is a `Creature`: it has hit points and armour and
/// can be struck like anything else. The port gives it the two facts the striking rules need now, and adds the
/// rest as rules arrive that ask for them.
#[derive(Debug, Clone, Copy, Component)]
pub struct Player {
    /// Hit points left.
    pub hp: i32,
    /// Hit points at full health.
    pub hp_max: i32,
    /// What armour takes off every blow.
    pub armour: i32,
    /// Moves left this turn; the reference's action points, like a monster's.
    pub moves: i32,
}

/// Puts the player in the world.
pub fn spawn_player(world: &mut World, square: (i32, i32), hp_max: i32, armour: i32) -> Entity {
    world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
            },
            Player {
                hp: hp_max,
                hp_max,
                armour,
                moves: 0,
            },
        ))
        .id()
}

/// Whether the player is dead, which is the moment the reference ends the game.
#[must_use]
pub fn player_dead(world: &mut World) -> bool {
    let mut query = world.query::<&Player>();
    query.iter(world).any(|player| player.hp <= 0)
}

/// Applies a blow to the player.
pub fn strike_player(world: &mut World, target: Entity, blow: i32) -> bool {
    let Some(mut player) = world.get_mut::<Player>(target) else {
        return false;
    };
    let dealt = through_armour(blow, player.armour);
    player.hp = player.hp.saturating_sub(dealt);
    player.hp <= 0
}

/// Gives the player its turn, by the same ladder a monster uses: strike what is next to it, otherwise step
/// toward the target.
///
/// The player's blow is a parameter rather than a lookup, because the player has no kind to look one up from -
/// what it is holding decides that, and equipment is a later rule.
///
/// Returns how many blows landed.
pub fn player_turn(
    world: &mut World,
    target: (i32, i32),
    blow: (i32, i32, i32),
    speed: i32,
    rules: &TurnRules<'_>,
    rolls: &mut cdda_core_types::rng::Rolls,
    level: &cdda_core_types::level::Level,
) -> i32 {
    let Some((entity, start, moves)) = ({
        let mut query = world.query::<(Entity, &Position, &Player)>();
        query.iter(world).find_map(|(entity, position, player)| {
            (player.hp > 0).then_some((entity, (position.x, position.y), player.moves))
        })
    }) else {
        return 0;
    };
    // Occupancy is read once, because the turn loop asks its question many times and may not borrow the World.
    let mut taken: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(&Position, &Monster)>();
        query
            .iter(world)
            .filter(|(_, monster)| monster.hp > 0)
            .map(|(position, _)| (position.x, position.y))
            .collect()
    };
    let mut players: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(&Position, &Player)>();
        query
            .iter(world)
            .filter(|(_, player)| player.hp > 0)
            .map(|(position, _)| (position.x, position.y))
            .collect()
    };
    taken.append(&mut players);
    let can_enter = |square: (i32, i32)| {
        if square == start {
            return true;
        }
        level
            .terrain_at(square.0, square.1)
            .is_some_and(rules.passable)
            && !taken.contains(&square)
    };
    let turn = take_turn_blocked(start, target, moves, speed, can_enter);
    let mut landed = 0;
    for _ in 0..turn.hits {
        let victim = {
            let mut query = world.query::<(Entity, &Position, &Monster)>();
            query
                .iter(world)
                .find(|(_, position, monster)| (position.x, position.y) == target && monster.hp > 0)
                .map(|(victim, _, _)| victim)
        };
        let Some(victim) = victim else {
            break;
        };
        let armour = world
            .get::<Monster>(victim)
            .map_or(0, |monster| (rules.armour_of)(monster.kind));
        let (dice, sides, bonus) = blow;
        let damage = melee_blow(rolls, dice, sides, bonus);
        strike(world, victim, damage, armour);
        landed += 1;
    }
    if let Some(mut player) = world.get_mut::<Player>(entity) {
        player.moves = turn.moves;
    }
    if turn.position != start
        && let Some(mut position) = world.get_mut::<Position>(entity)
    {
        position.x = turn.position.0;
        position.y = turn.position.1;
    }
    landed
}
