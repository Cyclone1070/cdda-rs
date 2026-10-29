//! Singletons: things that exist once, stored the same way as everything else.
//!
//! The architecture is one store of entities and components. Anything that is not a thing in the
//! world - a queue of squares, a light cache, a content table - is still stored there, as a
//! component on one entity that a marker component identifies. That is what these helpers are for:
//! they find the entity, or make it the first time, so no caller has to know where it lives.
//!
//! Two rules keep the pattern honest:
//!
//! * **Exactly one.** Finding or spawning returns the same entity every time, so a second queue, a
//!   second index or a second light map cannot exist by accident. Asking twice is how that is
//!   tested.
//! * **No side channel.** The value is a component and nothing else. No matching global exists, and
//!   the architecture gates count any global that appears beside it.

use bevy_ecs::prelude::*;

use crate::SupportLost;
use crate::falling::FieldRules;
use crate::memory::Memory;
use crate::field_table::FieldTable;
use crate::weather::Weather;
use crate::{Light, PositionIndex, Prototypes};
use crate::scent_table::ScentTable;
use crate::weather::Climate;
use crate::radiation::Radiation;

/// Marks the one entity that carries the falling queue.
#[derive(Debug, Default, Component)]
pub struct TheFallingQueue;

/// The entity carrying `T`, spawned with `T::default()` and the marker if it is not there yet.
///
/// `T` and `M` are the pair that makes a singleton: the data, and the marker that names it.
pub fn singleton<T: Component + Default, M: Component + Default>(world: &mut World) -> Entity {
    let found = {
        let mut query = world.query_filtered::<Entity, With<M>>();
        query.iter(world).next()
    };
    match found {
        Some(entity) => entity,
        None => world.spawn((T::default(), M::default())).id(),
    }
}

/// The entity carrying the falling queue, spawned on first use.
pub fn falling_queue_entity(world: &mut World) -> Entity {
    singleton::<SupportLost, TheFallingQueue>(world)
}

/// The falling queue, spawned on first use.
///
/// # Panics
///
/// Panics if the entity [`falling_queue_entity`] returned does not carry the queue, which cannot
/// happen: the helper is the only thing that creates it, and it creates it with both.
pub fn support_lost(world: &mut World) -> Mut<'_, SupportLost> {
    let entity = falling_queue_entity(world);
    match world.get_mut::<SupportLost>(entity) {
        Some(queue) => queue,
        None => unreachable!("the falling queue entity was just spawned with its queue"),
    }
}

/// Marks the one entity that carries the radiation store.
#[derive(Debug, Default, Component)]
pub struct TheRadiation;

/// The radiation store, spawned on first use.
///
/// # Panics
///
/// Panics if the entity [`singleton`] returned does not carry the store, which cannot happen: the
/// helper is the only thing that creates it, and it creates it with both.
pub fn radiation(world: &mut World) -> Mut<'_, Radiation> {
    let entity = singleton::<Radiation, TheRadiation>(world);
    match world.get_mut::<Radiation>(entity) {
        Some(store) => store,
        None => unreachable!("the radiation entity was just spawned with its store"),
    }
}

/// Adds a dose to a square, which is `map::adjust_radiation`.
pub fn adjust_radiation(world: &mut World, square: (i32, i32, i32), delta: i32) {
    radiation(world).adjust(square, delta);
}

/// Marks the one entity that carries the character's memory.
#[derive(Debug, Default, Component)]
pub struct TheMemory;

/// The entity carrying the character's memory, spawned on first use.
pub fn memory_entity(world: &mut World) -> Entity {
    singleton::<Memory, TheMemory>(world)
}

/// The character's memory, spawned on first use.
///
/// # Panics
///
/// Panics if the entity [`memory_entity`] returned does not carry the memory, which cannot happen:
/// the helper is the only thing that creates it, and it creates it with both.
pub fn memory(world: &mut World) -> Mut<'_, Memory> {
    let entity = memory_entity(world);
    match world.get_mut::<Memory>(entity) {
        Some(memory) => memory,
        None => unreachable!("the memory entity was just spawned with its memory"),
    }
}

/// Marks the one entity that carries the rules for what falls and what floats.
#[derive(Debug, Default, Component)]
pub struct TheFieldRules;

/// The falling rules, spawned empty on first use.
///
/// # Panics
///
/// Panics if the entity [`singleton`] returned does not carry the rules, which cannot happen: the
/// helper is the only thing that creates it, and it creates it with both.
pub fn field_rules(world: &mut World) -> Mut<'_, FieldRules> {
    let entity = singleton::<FieldRules, TheFieldRules>(world);
    match world.get_mut::<FieldRules>(entity) {
        Some(rules) => rules,
        None => unreachable!("the rules entity was just spawned with its rules"),
    }
}

/// Replaces the falling rules, which is what a loaded content set does once.
pub fn set_field_rules(world: &mut World, rules: FieldRules) {
    *field_rules(world) = rules;
}

/// Marks the one entity that carries what the loaded codes mean for light.
#[derive(Debug, Default, Component)]
pub struct ThePrototypes;

/// The prototypes, if the world loaded them.
#[must_use]
pub fn prototypes(world: &World) -> Option<&Prototypes> {
    world.iter_entities().find_map(|entity| entity.get::<Prototypes>())
}

/// Replaces the prototypes, which is what a loaded content set does once.
pub fn set_prototypes(world: &mut World, prototypes: Prototypes) {
    let entity = singleton::<Prototypes, ThePrototypes>(world);
    if let Some(mut slot) = world.get_mut::<Prototypes>(entity) {
        *slot = prototypes;
    }
}

/// Marks the one entity that carries the derived square index.
#[derive(Debug, Default, Component)]
pub struct TheIndex;

/// The square index, if it has been built.
#[must_use]
pub fn index(world: &World) -> Option<&PositionIndex> {
    world.iter_entities().find_map(|entity| entity.get::<PositionIndex>())
}

/// The square index, made empty if nothing has built it yet.
///
/// # Panics
///
/// Panics if the entity found does not carry the index, which cannot happen: it is found by having
/// it, and spawned with it.
pub fn index_mut(world: &mut World) -> Mut<'_, PositionIndex> {
    let entity = singleton::<PositionIndex, TheIndex>(world);
    match world.get_mut::<PositionIndex>(entity) {
        Some(index) => index,
        None => unreachable!("the index entity was just spawned with its index"),
    }
}

/// How many sight casts the world has made.
///
/// A receipt rather than a rule: the frame draws with one cast, and a test can say "one" only if
/// something counts them. The counter is a component, because a counter is a thing.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct SightCasts {
    /// How many casts have been built.
    pub casts: u32,
}

/// Marks the one entity that carries the count of sight casts.
#[derive(Debug, Default, Component)]
pub struct TheSightCasts;

/// Counts one sight cast.
pub fn count_sight_cast(world: &mut World) {
    let mut casts = sight_cast_counter(world);
    casts.casts = casts.casts.saturating_add(1);
}

/// How many sight casts the world has made.
#[must_use]
pub fn sight_casts(world: &World) -> u32 {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<SightCasts>().map(|casts| casts.casts))
        .unwrap_or(0)
}

/// The counter itself, made on first use.
///
/// # Panics
///
/// Panics if the entity found does not carry the counter, which cannot happen: it is found by
/// having it, and spawned with it.
fn sight_cast_counter(world: &mut World) -> Mut<'_, SightCasts> {
    let entity = singleton::<SightCasts, TheSightCasts>(world);
    match world.get_mut::<SightCasts>(entity) {
        Some(casts) => casts,
        None => unreachable!("the sight-cast entity was just spawned with its counter"),
    }
}

/// Marks the one entity that carries the light answer.
#[derive(Debug, Default, Component)]
pub struct TheLight;

/// The light answer, or an empty one when nothing has computed it.
///
/// Returned by value because the answer is a small map and a caller that reads it usually keeps it
/// across a loop that also writes the world.
#[must_use]
pub fn light(world: &World) -> Light {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<Light>().cloned())
        .unwrap_or_default()
}

/// The light answer, made empty if nothing has written it yet.
///
/// # Panics
///
/// Panics if the entity found does not carry the answer, which cannot happen: it is found by
/// having it, and spawned with it.
pub fn light_mut(world: &mut World) -> Mut<'_, Light> {
    let entity = singleton::<Light, TheLight>(world);
    match world.get_mut::<Light>(entity) {
        Some(light) => light,
        None => unreachable!("the light entity was just spawned with its answer"),
    }
}

/// Marks the one entity that carries what the sky is doing.
#[derive(Debug, Default, Component)]
pub struct TheWeather;

/// The sky, if the world has one.
#[must_use]
pub fn weather(world: &World) -> Option<Weather> {
    world.iter_entities().find_map(|entity| entity.get::<Weather>()).copied()
}

/// Replaces the sky, which is what a weather change does.
pub fn set_weather(world: &mut World, weather: Weather) {
    let entity = singleton::<Weather, TheWeather>(world);
    if let Some(mut slot) = world.get_mut::<Weather>(entity) {
        *slot = weather;
    }
}

/// Marks the one entity that carries the weather's own clock.
#[derive(Debug, Default, Component)]
pub struct TheClimate;

/// The climate, if the world has one.
#[must_use]
pub fn climate_read(world: &World) -> Option<&Climate> {
    world.iter_entities().find_map(|entity| entity.get::<Climate>())
}

/// The climate, if the world has one, ready to be changed.
pub fn climate(world: &mut World) -> Option<Mut<'_, Climate>> {
    let entity = world
        .query_filtered::<Entity, With<TheClimate>>()
        .iter(world)
        .next()?;
    world.get_mut::<Climate>(entity)
}

/// Replaces the climate, which is what a world with loaded weather rules does once.
pub fn set_climate(world: &mut World, climate: Climate) {
    let entity = singleton::<Climate, TheClimate>(world);
    if let Some(mut slot) = world.get_mut::<Climate>(entity) {
        *slot = climate;
    }
}

/// Marks the one entity that carries what each field type does to light.
#[derive(Debug, Default, Component)]
pub struct TheFieldTable;

/// The field light table, if the world loaded one.
#[must_use]
pub fn field_table(world: &World) -> Option<&FieldTable> {
    world.iter_entities().find_map(|entity| entity.get::<FieldTable>())
}

/// Replaces the field light table, which is what a loaded content set does once.
pub fn set_field_table(world: &mut World, table: FieldTable) {
    let entity = singleton::<FieldTable, TheFieldTable>(world);
    if let Some(mut slot) = world.get_mut::<FieldTable>(entity) {
        *slot = table;
    }
}

/// Marks the one entity that carries the terrain's scent flags.
#[derive(Debug, Default, Component)]
pub struct TheScentTable;

/// The scent flags, if the world loaded them.
#[must_use]
pub fn scent_table(world: &World) -> Option<&ScentTable> {
    world.iter_entities().find_map(|entity| entity.get::<ScentTable>())
}

/// Replaces the scent flags, which is what a loaded content set does once.
pub fn set_scent_table(world: &mut World, table: ScentTable) {
    let entity = singleton::<ScentTable, TheScentTable>(world);
    if let Some(mut slot) = world.get_mut::<ScentTable>(entity) {
        *slot = table;
    }
}

/// Marks the one entity that carries the level's scent grid.
#[derive(Debug, Default, Component)]
pub struct TheScent;

/// Queues one square whose support went away.
pub fn queue_support_loss(world: &mut World, square: (i32, i32, i32)) {
    support_lost(world).queue(square);
}
