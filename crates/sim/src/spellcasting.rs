//! Casting a spell at a creature, as far as the data allows it today.
//!
//! Reference: `spell::cast` (`magic.cpp`) picks the shape, `spell_effect::target_attack` finds what is in the area,
//! and `damage_target` hands each of them a `damage_instance` - which the creature then meets with the armour on the
//! part the blow landed on. The port keeps that shape: a spell's numbers come from content ([`cdda_content::spells`]),
//! the range and damage are worked out at the caster's level, and the damage goes through the same part-armour the
//! melee path uses, so a vest protects you from a taser for the same reason it protects you from a fist.

use bevy_ecs::prelude::*;
use cdda_content::spells::Spell;

use crate::creature::{Body, PartArmour, armour_on, hurt};
use crate::hit_location::struck_part;
use crate::through_armour;

/// What came of casting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cast {
    /// It landed: which part it hit and how much got through.
    Hit {
        /// The body part the blow landed on.
        part: String,
        /// What the target actually lost, after armour.
        dealt: i32,
    },
    /// The spell's effect is not one this rule runs. Only `attack` is ported so far.
    NotAnAttack,
    /// The target is further away than the spell reaches at this level.
    OutOfRange,
    /// The target has no body, or the caster and target are the same thing.
    NothingToHit,
}

/// Casts an `attack` spell at one creature.
///
/// Reference: `spell_effect::target_attack` -> `damage_target`, which deals the spell's damage type at
/// `spell::get_damage()` (the level-scaled value) to the part the blow lands on, and lets the creature's armour meet
/// it.
///
/// `fraction` is where in the hit-location roll the blow lands: 0.0 is the first part a body offers. It is a
/// parameter rather than a draw inside this function for the same reason every other roll in the port is - a rule
/// that reached for its own randomness could not be tested at all.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn cast_attack(
    world: &mut World,
    caster: (i32, i32, i32),
    spell: &Spell,
    level: i32,
    target: Entity,
    fraction: f64,
) -> Cast {
    if spell.effect_name != "attack" {
        return Cast::NotAnAttack;
    }
    let Some(place) = world.get::<crate::Position>(target) else {
        return Cast::NothingToHit;
    };
    // Reference: `spell::range()` is the level-scaled reach and `rl_dist` is the distance it is measured against -
    // the larger of the two axes, not the diagonal.
    let away = (place.x - caster.0).abs().max((place.y - caster.1).abs());
    let reach = spell.range.at(level).unwrap_or(f64::from(away));
    if f64::from(away) > reach {
        return Cast::OutOfRange;
    }
    let Some(body) = world.get::<Body>(target) else {
        return Cast::NothingToHit;
    };
    let Some(part) = struck_part(&body.parts, 0, false, fraction) else {
        return Cast::NothingToHit;
    };
    let part = part.name.clone();
    let Some(damage) = spell.damage.at(level) else {
        // An expression the loader would not read as a number: the port cannot say what the spell does, and a guess
        // here would be a silent divergence in every fight that involves it.
        return Cast::NothingToHit;
    };
    let armour = armour_on(world, target, &part);
    let dealt = through_armour(damage.round() as i32, armour);
    hurt(world, target, dealt as f32);
    Cast::Hit { part, dealt }
}

/// The armour a creature has on a part, for a caller that wants to know before casting.
///
/// A thin convenience: the number is `PartArmour`'s, and this only saves every caller from writing the lookup.
#[must_use]
pub fn armour_there(world: &World, target: Entity, part: &str) -> i32 {
    world
        .get::<PartArmour>(target)
        .and_then(|armour| armour.0.get(part).copied())
        .unwrap_or(0)
}
