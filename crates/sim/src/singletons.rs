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

/// Queues one square whose support went away.
pub fn queue_support_loss(world: &mut World, square: (i32, i32, i32)) {
    support_lost(world).queue(square);
}
