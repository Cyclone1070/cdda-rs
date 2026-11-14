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

/// How much practice a creature has in a skill, which is what a gain is measured by.
#[must_use]
pub fn exercise_of(world: &World, creature: Entity, skill: &str) -> i32 {
    world
        .get::<Skills>(creature)
        .and_then(|skills| skills.0.get(skill).map(|level| level.exercise(false)))
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
/// What a craft's practice is worth, on the reference's own chain.
///
/// Reference: `Character::practice` (`character.cpp:2804-2880`) called from `craft_skill_gain` (`crafting.cpp:1032`). In order: the amount is capped at a thousand
/// and scaled by focus, then multiplied by a hundred; a level already **above** the cap earns nothing; and the training itself takes the catch-up and knowledge
/// modifiers the reference derives from intelligence and perception.
///
/// The two modifiers use the reference's own average of **eight** for both stats - its comments state the results those numbers give, "2 for an average person" and
/// "1.2 for an average person" (`character.cpp:2818-2822`), and eight in each is what produces them. The port has no intelligence or perception yet, so those
/// averages are the honest place to stand: the day the stats arrive, this is the one function that changes.
///
/// Answers how much practice was actually laid on, which is what a test can see.
pub fn practice(
    world: &mut bevy_ecs::prelude::World,
    creature: bevy_ecs::prelude::Entity,
    skill: &str,
    amount: i32,
    cap: i32,
) -> i32 {
    // The reference's average of eight for each stat (`character.cpp:2818-2822`), named so the sum is not an equal-operand division.
    const AVERAGE_INT: f32 = 8.0;
    const AVERAGE_PER: f32 = 8.0;
    if skill.is_empty() || amount <= 0 {
        return 0;
    }
    #[allow(clippy::cast_precision_loss)]
    let scaled = crate::skills::adjust_for_focus(world, creature, amount.min(1000) as f32) * 100.0;
    #[allow(clippy::cast_possible_truncation)]
    let scaled = scaled as i32;
    // `practice` refuses at the skill's maximum; the port's maximum is the same hundred the reference uses.
    if level_of(world, creature, skill) >= crate::skills::MAX_SKILL {
        return 0;
    }
    // Reference: a practical level above the cap earns nothing, which is the "blunt grinding cap" it names (`character.cpp:2839-2845`).
    if level_of(world, creature, skill) > cap {
        return 0;
    }
    let catchup_modifier = 1.0 + (2.0 * AVERAGE_INT + AVERAGE_PER) / 24.0;
    let knowledge_modifier = 1.0 + AVERAGE_INT / 40.0;
    let mut entry = world
        .get::<Skills>(creature)
        .and_then(|skills| skills.0.get(skill).copied())
        .unwrap_or_default();
    entry.train(scaled, catchup_modifier, knowledge_modifier, 1.0, true);
    if let Some(mut skills) = world.get_mut::<Skills>(creature) {
        skills.0.insert(skill.to_owned(), entry);
    } else {
        let mut fresh = Skills::default();
        fresh.0.insert(skill.to_owned(), entry);
        world.entity_mut(creature).insert(fresh);
    }
    scaled
}
