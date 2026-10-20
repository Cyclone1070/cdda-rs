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
