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
const NEIGHBOURS: [(i32, i32); 8] = [
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
