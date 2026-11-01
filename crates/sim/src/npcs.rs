//! Which creatures are people.
//!
//! Reference: `Character::is_npc` (`character.h`) and the `npc` class beside `monster`, which the game tells apart by type rather than by a flag - a shopkeeper
//! and a zombie are different kinds of thing. The port keeps **one** store, so the difference is a component: a creature carrying [`Npc`] is a person, and one
//! carrying [`crate::CreatureKind`] is a monster. A rule can ask which without knowing either class existed.
//!
//! The unique id is the reference's `npc::get_unique_id`, the name a rule uses to pick out one person from a crowd.

use bevy_ecs::prelude::*;

/// A creature that is a person rather than a monster.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Npc {
    /// The id the data writes in `unique_ids`, empty when the person was made without one.
    pub unique_id: String,
}

/// Makes a creature a person, with the id a rule can pick it out by.
pub fn make_npc(world: &mut World, creature: Entity, unique_id: &str) {
    world.entity_mut(creature).insert(Npc {
        unique_id: unique_id.to_owned(),
    });
}

/// Whether a creature is a person.
#[must_use]
pub fn is_npc(world: &World, creature: Entity) -> bool {
    world.get::<Npc>(creature).is_some()
}

/// One person's id, when the creature is one.
#[must_use]
pub fn unique_id_of(world: &World, creature: Entity) -> Option<&str> {
    world.get::<Npc>(creature).map(|npc| npc.unique_id.as_str())
}

/// Every person in the store, with the square they are standing on.
#[must_use]
pub fn npcs(world: &World) -> Vec<(Entity, (i32, i32, i32))> {
    let Some(mut query) = world.try_query::<(Entity, &Npc, &crate::Position)>() else {
        return Vec::new();
    };
    query
        .iter(world)
        .map(|(entity, _, at)| (entity, (at.x, at.y, at.z)))
        .collect()
}
