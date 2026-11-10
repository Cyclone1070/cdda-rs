//! How many of each kind a character has killed.
//!
//! Reference: `kill_tracker` (`src/kill_tracker.h` and `game::get_kill_tracker`). The reference keeps a count per monster type and a separate `guilt_kill_count` for the ones that
//! count towards a character's conscience; `spell_effect::guilt` reads that second count to decide how bad a killing feels.
//!
//! The port derives the fact where it is decided - a body reaching zero with a character as the striker - and keeps the tally in one component, so "how many of these have I
//! killed" is a question about the store rather than a list of corpses the world no longer holds.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

/// A count of kills per monster kind, for the creatures the player has struck down.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Kills(pub BTreeMap<crate::MonsterKindId, i32>);

/// Marks the entity carrying [`Kills`].
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TheKills;

/// Records one kill of a kind.
pub fn record(world: &mut World, kind: crate::MonsterKindId) {
    let entity = crate::singletons::singleton::<Kills, TheKills>(world);
    if let Some(mut kills) = world.get_mut::<Kills>(entity) {
        *kills.0.entry(kind).or_insert(0) += 1;
    }
}

/// How many of a kind have been killed, which is zero for one that never has.
#[must_use]
pub fn count_of(world: &World, kind: crate::MonsterKindId) -> i32 {
    let Some(entity) = crate::singletons::holder::<TheKills>(world) else {
        return 0;
    };
    world
        .get::<Kills>(entity)
        .and_then(|kills| kills.0.get(&kind).copied())
        .unwrap_or(0)
}
