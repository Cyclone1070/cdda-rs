//! What a monster is when it is not real.
//!
//! Reference: `mtype::HALLUCINATION` (`monstergenerator.cpp` and `monster.cpp`), the flag a hallucinated monster carries, and
//! `game::remove_nonlocal_monsters` / the hallucination culling that follows a good look at one. The port keeps the flag as a **component** so "is this real"
//! is a query rather than a special case in whatever reads the monster next.

use bevy_ecs::prelude::*;

/// A monster that looks real and is not.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hallucination;

/// Whether a creature is a hallucination.
#[must_use]
pub fn is_hallucination(world: &World, creature: Entity) -> bool {
    world.get::<Hallucination>(creature).is_some()
}

/// Every hallucination in the store, which a test or a dump reads.
#[must_use]
pub fn hallucinations(world: &World) -> Vec<Entity> {
    let Some(mut query) = world.try_query::<(Entity, &Hallucination)>() else {
        return Vec::new();
    };
    query.iter(world).map(|(entity, _)| entity).collect()
}
