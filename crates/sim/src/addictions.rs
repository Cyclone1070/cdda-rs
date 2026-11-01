//! What a character is addicted to, how strongly, and how long since it was last sated.
//!
//! Reference: `class addiction` (`src/addiction.h:58-80`): a type, an `intensity` from zero to `MAX_ADDICTION_LEVEL`, and a `sated` duration that starts at one
//! hour. The level is what the dialogue asks about (`addiction_intensity`), and the sated time is what it asks about next (`addiction_turns`).
//!
//! Why this is a store rather than a reader: the reference keeps addictions inside the character, the data asks for the numbers constantly
//! (`rand(6) < u_addiction_intensity('mutagen')`), and a port that answered from nowhere would be answering zero for every character in the game.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use std::collections::BTreeMap;

/// The strongest an addiction can become: `MAX_ADDICTION_LEVEL` (`src/addiction.h:80`).
pub const MAX_ADDICTION_LEVEL: i32 = 20;

/// The weakest level that still shows effects: `MIN_ADDICTION_LEVEL` (`src/addiction.h:79`).
pub const MIN_ADDICTION_LEVEL: i32 = 3;

/// How long an addiction starts sated for: the reference's `sated = 1_hours` default (`src/addiction.h:64`).
pub const SATED_START_TURNS: i64 = 600;

/// One addiction: how strong it is, and how long since it was last satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Addiction {
    /// The level, from zero to [`MAX_ADDICTION_LEVEL`].
    pub intensity: i32,
    /// Turns left until it is unsatisfied; the reference stores a duration, and the port keeps turns because that is what the data asks for.
    pub sated_turns: i64,
}

/// The addictions a character carries.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Addictions(pub BTreeMap<String, Addiction>);

/// The level of one addiction, **zero** when the character does not have it.
///
/// Reference: `Character::addiction_level` (`src/suffer.cpp:2118-2125`): a search of the list, and zero when no entry matches - the same "none of it" answer the
/// vitamins give.
#[must_use]
pub fn intensity_of(world: &World, creature: Entity, addiction: &str) -> i32 {
    world
        .get::<Addictions>(creature)
        .and_then(|held| held.0.get(addiction))
        .map_or(0, |found| found.intensity)
}

/// How long one addiction has left before it is unsatisfied, **zero** when the character does not have it.
///
/// Reference: `talker_character_const::get_addiction_turns` (`src/talker_character.cpp:1011-1019`): the matching entry's `sated`, or zero after the loop.
#[must_use]
pub fn turns_of(world: &World, creature: Entity, addiction: &str) -> i64 {
    world
        .get::<Addictions>(creature)
        .and_then(|held| held.0.get(addiction))
        .map_or(0, |found| found.sated_turns)
}

/// Gives a character an addiction at a level, or changes the level of one it already has.
///
/// A level outside `[0, MAX_ADDICTION_LEVEL]` is clamped rather than refused: the reference's own `addiction_level` setter does the same, because the data
/// computes levels and a formula can overshoot.
pub fn set_intensity(world: &mut World, creature: Entity, addiction: &str, intensity: i32) {
    let intensity = intensity.clamp(0, MAX_ADDICTION_LEVEL);
    let entry = Addiction {
        intensity,
        sated_turns: SATED_START_TURNS,
    };
    if let Some(mut held) = world.get_mut::<Addictions>(creature) {
        held.0
            .entry(addiction.to_owned())
            .and_modify(|found| found.intensity = intensity)
            .or_insert(entry);
        return;
    }
    let mut held = Addictions::default();
    held.0.insert(addiction.to_owned(), entry);
    world.entity_mut(creature).insert(held);
}

/// Sets how long one addiction has left before it is unsatisfied, adding it if the character does not have it.
pub fn set_turns(world: &mut World, creature: Entity, addiction: &str, turns: i64) {
    if let Some(mut held) = world.get_mut::<Addictions>(creature)
        && let Some(found) = held.0.get_mut(addiction)
    {
        found.sated_turns = turns;
        return;
    }
    set_intensity(world, creature, addiction, 1);
    if let Some(mut held) = world.get_mut::<Addictions>(creature)
        && let Some(found) = held.0.get_mut(addiction)
    {
        found.sated_turns = turns;
    }
}
