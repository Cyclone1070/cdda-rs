//! The achievements a character has completed.
//!
//! Reference: `achievement_tracker` (`achievement.h`), with `report_achievement` marking one completed, and `f_give_achievment` (`npctalk.cpp:5566-5585`) reaching
//! it from a dialogue. The port keeps the completed ids on the character - one component, no table beside it - and answers "is this one done" from the same store.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Entity, World};
use std::collections::BTreeSet;

/// The achievements a character has completed, by id.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Achievements(pub BTreeSet<String>);

/// Marks one achievement completed on a character, answering whether it was new.
///
/// Reference: `f_give_achievment` only reports an achievement that is **pending**, so completing a finished one changes nothing and answers `false` - which is what
/// lets a caller tell "it happened" from "it already had".
pub fn complete(world: &mut World, character: Entity, achievement: &str) -> bool {
    if let Some(mut held) = world.get_mut::<Achievements>(character) {
        return held.0.insert(achievement.to_owned());
    }
    world
        .entity_mut(character)
        .insert(Achievements(BTreeSet::from([achievement.to_owned()])));
    true
}

/// Whether a character has completed an achievement.
#[must_use]
pub fn has_completed(world: &World, character: Entity, achievement: &str) -> bool {
    world
        .get::<Achievements>(character)
        .is_some_and(|held| held.0.contains(achievement))
}

/// Every achievement a character has completed, in order.
#[must_use]
pub fn completed(world: &World, character: Entity) -> Vec<String> {
    world
        .get::<Achievements>(character)
        .map(|held| held.0.iter().cloned().collect())
        .unwrap_or_default()
}
