//! Mutations: what a creature has become, and what it may become next.
//!
//! Reference: `Character::has_trait`, `Character::can_mutate` and `Character::mutate_towards` (`mutation.cpp:520-700`). The creature holds **ids** in one component;
//! the four sets that decide whether a mutation may be gained live in the data table (`cdda_content::mutations`), not on the creature.

use bevy_ecs::prelude::*;
use cdda_content::mutations::Mutations as MutationData;

/// The mutations and traits a creature has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct Mutations(pub Vec<String>);

/// Why a mutation cannot be gained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gain {
    /// It can be gained.
    Yes,
    /// One of its prerequisites is missing.
    MissingPrerequisite,
    /// The creature already holds something the mutation conflicts with.
    Conflicting,
    /// The data does not define it.
    Unknown,
}

/// Whether a creature has a mutation.
#[must_use]
pub fn has(world: &World, creature: Entity, id: &str) -> bool {
    world
        .get::<Mutations>(creature)
        .is_some_and(|held| held.0.iter().any(|known| known == id))
}

/// Whether a creature may gain a mutation, and why not when it may not.
///
/// Reference: `Character::can_mutate` (`mutation.cpp:540-575`): every entry in `prereqs` is an alternative - **one** is enough - and the same holds for `prereqs2`;
/// a mutation in `cancels` that the creature already has blocks the gain outright.
#[must_use]
pub fn can_gain(world: &World, creature: Entity, data: &MutationData, id: &str) -> Gain {
    let Some(rule) = data.get(id) else {
        return Gain::Unknown;
    };
    if has(world, creature, id) {
        return Gain::Yes;
    }
    // Reference: `Character::mutate_towards` (`mutation.cpp:1656-1712`): a conflict is **removed**, not a barrier - unless the trait holding the conflict cannot be
    // taken away at all, in which case the gain is refused (`canceltrait` is not purifiable, and the function returns false).
    let stuck = rule.cancels.iter().any(|blocker| {
        has(world, creature, blocker) && data.get(blocker).is_some_and(|held| !held.purifiable)
    });
    if stuck {
        return Gain::Conflicting;
    }
    if !satisfied(world, creature, &rule.prereqs) || !satisfied(world, creature, &rule.prereqs2) {
        return Gain::MissingPrerequisite;
    }
    Gain::Yes
}

/// One of the list must be held, and an empty list asks for nothing.
fn satisfied(world: &World, creature: Entity, needed: &[String]) -> bool {
    needed.is_empty() || needed.iter().any(|one| has(world, creature, one))
}

/// Gives a creature a mutation, taking away what the new one conflicts with.
///
/// Reference: `Character::mutate_towards` (`mutation.cpp:600-700`): the conflicting traits are removed **first**, so the creature never holds two mutations that
/// cancel each other, and the gain is refused when the prerequisites are not met.
pub fn gain(world: &mut World, creature: Entity, data: &MutationData, id: &str) -> Gain {
    let verdict = can_gain(world, creature, data, id);
    if verdict != Gain::Yes {
        return verdict;
    }
    let Some(rule) = data.get(id) else {
        return Gain::Unknown;
    };
    let conflicts: Vec<String> = rule.cancels.clone();
    let mut held = world.get::<Mutations>(creature).cloned().unwrap_or_default();
    held.0.retain(|known| !conflicts.contains(known));
    if !held.0.iter().any(|known| known == id) {
        held.0.push(id.to_owned());
    }
    world.entity_mut(creature).insert(held);
    Gain::Yes
}

/// Every mutation a creature has.
#[must_use]
pub fn mutations_of(world: &World, creature: Entity) -> Vec<String> {
    world
        .get::<Mutations>(creature)
        .map_or_else(Vec::new, |held| held.0.clone())
}
