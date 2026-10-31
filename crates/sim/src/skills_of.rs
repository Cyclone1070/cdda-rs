//! Which skills a creature has, and how well it knows them.
//!
//! Reference: `Character::get_skill_level( const skill_id & )` (`character.cpp`), which reads one entry out of the character's skill map. The port keeps the
//! map on the creature, so "how good is this creature at that" is one component and one lookup.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::skills::SkillLevel;

/// A creature's standing in each skill it has practised.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Skills(pub BTreeMap<String, SkillLevel>);

/// How good a creature is at a skill. A skill it has never practised is level **0**, not "unknown" - the reference's map returns a default entry.
#[must_use]
pub fn level_of(world: &World, creature: Entity, skill: &str) -> i32 {
    world
        .get::<Skills>(creature)
        .and_then(|skills| skills.0.get(skill).map(SkillLevel::level))
        .unwrap_or(0)
}

/// Sets a creature's level in a skill, keeping any practice it already has.
pub fn set_level(world: &mut World, creature: Entity, skill: &str, level: i32) {
    let mut entry = world
        .get::<Skills>(creature)
        .and_then(|skills| skills.0.get(skill).copied())
        .unwrap_or_default();
    entry.level_to(level);
    if let Some(mut skills) = world.get_mut::<Skills>(creature) {
        skills.0.insert(skill.to_owned(), entry);
        return;
    }
    let mut fresh = Skills::default();
    fresh.0.insert(skill.to_owned(), entry);
    world.entity_mut(creature).insert(fresh);
}

/// The skills a creature has practised, which a test or a dump reads.
#[must_use]
pub fn skills_of(world: &World, creature: Entity) -> Vec<String> {
    world
        .get::<Skills>(creature)
        .map(|skills| skills.0.keys().cloned().collect())
        .unwrap_or_default()
}
