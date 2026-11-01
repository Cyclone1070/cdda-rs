//! The spells a creature knows, and how far it has taken each one.
//!
//! Reference: `Character::magic` (`character.h`), a map from `spell_id` to `spell`, each carrying a `level` and an `exp`. The dialogue asks about them constantly -
//! `u_spell_level('x')` is the gate on "have you learned enough to cast this" - and the effects write them back, so a store has to exist before either reader means
//! anything.
//!
//! It is one component beside the others: a creature either knows spells or has none, and there is no second table the level could come from.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::World;
use std::collections::BTreeMap;

/// One spell a creature knows: the level it has reached and the experience banked toward the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spelling {
    /// The level, from zero up.
    pub level: i32,
    /// Experience banked toward the next level.
    pub experience: i32,
}

/// The spells a creature knows, by id.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownSpells(pub BTreeMap<String, Spelling>);

/// A spell's level, or zero for one the creature does not know.
///
/// Reference: `Character::get_spell_level` (`magic.cpp`), which answers zero rather than failing - "not learned" is a level, not an error, and the data compares
/// against it without a guard (`u_spell_level('x') >= 3`).
#[must_use]
pub fn level_of(world: &World, creature: bevy_ecs::entity::Entity, spell: &str) -> i32 {
    world
        .get::<KnownSpells>(creature)
        .and_then(|known| known.0.get(spell).map(|learned| learned.level))
        .unwrap_or(0)
}

/// The experience banked toward a spell's next level, or zero.
#[must_use]
pub fn experience_of(world: &World, creature: bevy_ecs::entity::Entity, spell: &str) -> i32 {
    world
        .get::<KnownSpells>(creature)
        .and_then(|known| known.0.get(spell).map(|learned| learned.experience))
        .unwrap_or(0)
}

/// The highest level the creature has reached in any spell, which is what an unnamed `spell_level()` answers.
///
/// Reference: `Character::get_highest_spell_level` (`magic.cpp`) - the data uses it to scale a spell nobody named, so a creature that knows nothing answers zero.
#[must_use]
pub fn highest_level(world: &World, creature: bevy_ecs::entity::Entity) -> i32 {
    world
        .get::<KnownSpells>(creature)
        .and_then(|known| known.0.values().map(|learned| learned.level).max())
        .unwrap_or(0)
}

/// Sets a spell's level, answering whether the creature can hold spells at all.
pub fn set_level(world: &mut World, creature: bevy_ecs::entity::Entity, spell: &str, level: i32) -> bool {
    change(world, creature, spell, |learned| learned.level = level)
}

/// Sets the experience banked toward a spell's next level.
pub fn set_experience(
    world: &mut World,
    creature: bevy_ecs::entity::Entity,
    spell: &str,
    experience: i32,
) -> bool {
    change(world, creature, spell, |learned| {
        learned.experience = experience;
    })
}

/// Changes one spell's record, creating it when the creature does not have it.
fn change(
    world: &mut World,
    creature: bevy_ecs::entity::Entity,
    spell: &str,
    edit: impl FnOnce(&mut Spelling),
) -> bool {
    let Some(mut known) = world.get_mut::<KnownSpells>(creature) else {
        let mut fresh = KnownSpells::default();
        let mut learned = Spelling::default();
        edit(&mut learned);
        fresh.0.insert(spell.to_owned(), learned);
        world.entity_mut(creature).insert(fresh);
        return true;
    };
    let learned = known.0.entry(spell.to_owned()).or_default();
    edit(learned);
    true
}
