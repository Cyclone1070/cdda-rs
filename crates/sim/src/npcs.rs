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

/// Marks a person as one of the player's allies.
///
/// Reference: `npc::is_player_ally` (`npc.cpp`), the flag `f_npc_allies`/`f_npc_allies_global` count (`condition.cpp:1159-1179`). It is a state a person is in, so it is a marker
/// component rather than a field beside the store - the follower rule sets it when a person agrees to follow.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ally;

/// Makes a person one of the player's allies.
pub fn make_ally(world: &mut World, creature: Entity) {
    world.entity_mut(creature).insert(Ally);
}

/// How many of the player's allies the store holds.
///
/// Reference: `f_npc_allies_global` (`condition.cpp:1167-1179`) counts the NPCs that are allies, are not hallucinations and are not dead. The port has no hallucinations, and a
/// dead person is one whose [`crate::creature::Health`] has run out - which is the same set the death rule removes.
#[must_use]
pub fn allies(world: &World) -> usize {
    let Some(mut query) = world.try_query::<(Entity, &Ally)>() else {
        return 0;
    };
    query
        .iter(world)
        .filter(|(entity, _)| {
            // Reference: `!ptr->is_dead()` - a person whose hit points have run out is no longer counted among the living. A creature the port invented has no health
            // record at all, and nothing has hurt it, so it is alive.
            world
                .get::<crate::creature::Health>(*entity)
                .is_none_or(|health| health.hp > 0)
        })
        .count()
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
