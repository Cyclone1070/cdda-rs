//! What a monster is when it is not real.
//!
//! Reference: `mtype::HALLUCINATION` (`monstergenerator.cpp` and `monster.cpp`), the flag a hallucinated monster carries, and
//! `game::remove_nonlocal_monsters` / the hallucination culling that follows a good look at one. The port keeps the flag as a **component** so "is this real"
//! is a query rather than a special case in whatever reads the monster next.

use bevy_ecs::prelude::*;

/// A monster that looks real and is not.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hallucination;

/// A monster that was summoned for a while and will go when its time is up.
///
/// Reference: `monster::set_summon_time( time_duration )` (`monster.cpp`), which stores the turn a summoned monster expires, and the summon-expiry check that
/// removes it. The port keeps the expiry as a component, so "what is about to go" is a query rather than a scan of every monster.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summoned {
    /// The turn it stops existing at.
    pub until: i64,
}

/// Notes that a monster was summoned until a given turn.
pub fn summon_until(world: &mut World, creature: Entity, until: i64) {
    world.entity_mut(creature).insert(Summoned { until });
}

/// Removes every summon whose time has come, answering how many went.
///
/// Reference: the summon expiry in `monster::die`/`game::monmove`, which compares the summon time against the clock and erases the monster. The port answers
/// with a count so the caller can say what happened, and so a test can see the number rather than the absence.
pub fn expire_summonings(world: &mut World, now: i64) -> usize {
    let Some(mut query) = world.try_query::<(Entity, &Summoned)>() else {
        return 0;
    };
    let gone: Vec<Entity> = query
        .iter(world)
        .filter(|(_, summoned)| summoned.until <= now)
        .map(|(entity, _)| entity)
        .collect();
    for entity in &gone {
        world.entity_mut(*entity).despawn();
    }
    gone.len()
}

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
