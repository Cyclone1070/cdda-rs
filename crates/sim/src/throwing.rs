//! Throwing something at a square: the spread rules, the landing rule and the move, wired together.
//!
//! Reference: `Character::throw_item` (`src/ranged.cpp:1467-1600`). The reference's version also computes damage, shattering and noise; the port carries the part that
//! decides **where the thing goes**, because that is the part every later rule (a hit, a break, a fire) reads. An item is an entity here, so "throwing" is taking it out of
//! the hand and putting it on the square the rules name.

use bevy_ecs::prelude::*;

use crate::dispersion::{Dispersion, Throw, throwing_dispersion};

/// What came of a throw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Toss {
    /// It left the hand and stopped here.
    Landed {
        /// The square it stopped on.
        at: (i32, i32, i32),
        /// How badly the throw missed, as a fraction of the target.
        missed_by: f64,
    },
    /// The thrower is not in the world, so there is nowhere to throw from.
    NoBody,
    /// The thing is not in the thrower's hands.
    NotCarried,
    /// The target was the thrower's own square, which the reference refuses outright.
    AtYourself,
}

/// Throws what a creature holds at a square.
///
/// Reference: `Character::throw_item` (`src/ranged.cpp:1467-1600`) composes `throwing_dispersion` with `projectile_attack`, which rolls the spread through
/// [`crate::ballistics`]. The port does the same three steps: the spread from what is thrown and who throws it, a roll of that spread, and the aim rule that turns the roll
/// into a square. **The reference's damage, shattering and noise are not here** - they belong to what happens *on landing*, which is a rule of the square's own.
pub fn throw_at(world: &mut World, thrower: Entity, item: Entity, target: (i32, i32, i32)) -> Toss {
    let Some(source) = world
        .get::<crate::Position>(thrower)
        .map(|place| (place.x, place.y, place.z))
    else {
        return Toss::NoBody;
    };
    if target == source {
        return Toss::AtYourself;
    }
    let held = world.get::<crate::creature::Carried>(item).map(|who| who.0) == Some(thrower)
        || world.get::<crate::creature::WieldedBy>(item).map(|who| who.0) == Some(thrower);
    if !held {
        return Toss::NotCarried;
    }
    let Some(kind) = world.get::<crate::GroundItem>(item).map(|held| held.kind) else {
        return Toss::NotCarried;
    };
    let charges = world.get::<crate::GroundItem>(item).map_or(1, |held| held.charges);
    let names = crate::item_registry::snapshot(world);
    // Reference: `throw_dispersion_per_dodge` reads the **arm** strength, which is the strength a creature can put behind a throw.
    let arm_strength = crate::creature::effective_strength(world, thrower);
    // Reference: `get_skill_level( skill_throw )`, from the creature's own skill map.
    let skill = f64::from(crate::skills_of::level_of(world, thrower, "throw"));
    // The port has no eyes to encumber and no perception score, so those two terms of the reference's sum are zero - said here rather than left to look accidental.
    let spread = throwing_dispersion(&Throw {
        weight_g: names.weight_of(kind),
        volume_ml: i64::from(names.volume_of(kind)),
        charges,
        arm_strength,
        skill,
        perception: 0,
        eye_encumbrance: 0,
        blinded: false,
        target: None,
    });
    // `rl_dist` in the reference is the **larger** of the two offsets, which is what a diagonal step counts as one square.
    let range = (target.0 - source.0).abs().max((target.1 - source.1).abs());
    let shot = Dispersion {
        linear: vec![f64::from(spread)],
        ..Dispersion::default()
    };
    // The dice are taken for the length of a draw and put back, which is how every other rule in the port draws. A world with no generator has no spread to draw from, so
    // the throw is a perfect aim - the same convention [`crate::craft_roll`] uses, where a missing generator draws zero rather than refusing the action.
    let rolled = match crate::singletons::take_rolls(world) {
        Some(mut dice) => {
            let drawn = shot.roll(&mut dice);
            crate::singletons::put_rolls(world, dice);
            drawn
        }
        None => 0.0,
    };
    let aim = crate::ballistics::aim_of(rolled, f64::from(range), 1.0);
    let flight = crate::ballistics::Shot {
        source,
        target,
        range,
        aim,
        // A throw at open ground stops where it was aimed rather than flying on, which is the reference's `no_overshoot` for a passable square with no creature.
        no_overshoot: true,
    };
    let at = match crate::singletons::take_rolls(world) {
        Some(mut dice) => {
            let stopped = crate::ballistics::landing(&flight, &mut dice);
            crate::singletons::put_rolls(world, dice);
            stopped
        }
        None => target,
    };
    // The move itself: out of the hand, onto the square. The store finds an item by its position, so this is one component's worth of change.
    world
        .entity_mut(item)
        .remove::<crate::creature::Carried>()
        .remove::<crate::creature::WieldedBy>()
        .insert(crate::Position {
            x: at.0,
            y: at.1,
            z: at.2,
        });
    Toss::Landed {
        at,
        missed_by: aim.missed_by,
    }
}
