//! The flags a creature carries.
//!
//! Reference: `Creature::has_flag` (`creature.cpp`), which looks a creature's flag up in the flags its mutations and worn items give it. A flag is a
//! **name with no number**: `GRAB`, `FLIES`, `SEESLEEP`. The reference keeps them in a set built from the creature's mutations; the port keeps them on
//! the creature itself, so a flag is a component rather than something re-derived every time somebody asks.

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;

/// Every flag a creature carries.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct CreatureFlags(pub BTreeSet<String>);

/// Whether a creature carries a flag.
///
/// A creature with no flags at all answers `false` rather than "cannot say": that is what the reference does for a creature whose mutations give it
/// none, and the data leans on it - 46 `u_has_flag` guards in the data ask about `GRAB_FILTER` and expect a plain survivor to answer no.
#[must_use]
pub fn has_flag(world: &World, creature: Entity, flag: &str) -> bool {
    world
        .get::<CreatureFlags>(creature)
        .is_some_and(|flags| flags.0.contains(flag))
}

/// Gives a creature a flag, adding it to whatever it already carries.
pub fn add_flag(world: &mut World, creature: Entity, flag: &str) {
    if let Some(mut flags) = world.get_mut::<CreatureFlags>(creature) {
        flags.0.insert(flag.to_owned());
        return;
    }
    let mut flags = CreatureFlags::default();
    flags.0.insert(flag.to_owned());
    world.entity_mut(creature).insert(flags);
}

/// Takes a flag away from a creature, answering whether it had it.
pub fn remove_flag(world: &mut World, creature: Entity, flag: &str) -> bool {
    let Some(mut flags) = world.get_mut::<CreatureFlags>(creature) else {
        return false;
    };
    flags.0.remove(flag)
}

/// Every flag a creature carries, which a test or a dump reads.
#[must_use]
pub fn flags_of(world: &World, creature: Entity) -> Vec<String> {
    world
        .get::<CreatureFlags>(creature)
        .map(|flags| flags.0.iter().cloned().collect())
        .unwrap_or_default()
}
