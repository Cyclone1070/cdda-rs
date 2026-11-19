//! **Whether a creature notices a trap in time to step over it.**
//!
//! Reference: `Character::avoid_trap` (`character.cpp:12182-12203`) and the two questions it asks - whether the creature can see the trap at all (`trap::can_see`,
//! `trap.cpp:319-331`) and, if it can, how many dice the comparison is made with.
//!
//! The shape is a contest of two rolls:
//!
//! ```text
//! myroll   = dice( 3, round( dex_cur + dodge * 1.5 ) )   [+ dice( 2, 6 ) with Light Step, - dice( 2, 6 ) with Clumsy]
//! traproll = dice( can_see ? 3 : 6, avoidance )
//! avoided  = myroll >= traproll
//! ```
//!
//! **`can_see` is a memory question, not a sight question.** `trap::can_see` answers `visibility < 0 || p.knows_trap( pos )` - a trap is "visible" to a character once they have
//! noticed it there - and that is why the port needs [`KnownTraps`]: a creature that has once dodged a trap sees it next time, and the next roll is made with three dice instead of
//! six. A trap that is `always_invisible` is never seen and cannot be remembered.
//!
//! **Named gaps.** The reference's Light Step and Clumsy traits add and subtract `dice( 2, 6 )`; the port has no traits, so neither branch exists and the roll is the plain one.
//! `dodge` is the creature's [`crate::creature::Dodge`] points, which the reference reads as `get_skill_level( skill_dodge )`. And a creature the port made without a
//! [`crate::creature::Dexterity`] is given **no roll at all** - it does not dodge, so the trap fires. The reference starts every character at eight dexterity, so a creature here
//! with no stat is a port artefact rather than a person.

use bevy_ecs::prelude::*;
use std::collections::BTreeSet;

use cdda_content::traps::Trap;
use cdda_core_types::rng::Rolls;

/// The traps a creature has noticed, by square.
///
/// Reference: `Character::known_traps` (`character.h`), a map from square to trap id, filled by `add_known_trap` and emptied by `forget_trap`. The port keeps the squares
/// themselves: what the rule asks is "has this creature noticed the trap **here**", and the square is the answer to that question.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownTraps(pub BTreeSet<(i32, i32, i32)>);

/// Whether the creature has noticed a trap on this square.
#[must_use]
pub fn knows(world: &World, who: Entity, square: (i32, i32, i32)) -> bool {
    world
        .get::<KnownTraps>(who)
        .is_some_and(|known| known.0.contains(&square))
}

/// Remembers a trap, so the creature sees it from then on.
pub fn remember(world: &mut World, who: Entity, square: (i32, i32, i32)) {
    if let Some(mut known) = world.get_mut::<KnownTraps>(who) {
        known.0.insert(square);
        return;
    }
    let _ = world
        .entity_mut(who)
        .insert(KnownTraps(BTreeSet::from([square])));
}

/// **Whether the creature can see the trap on a square.**
///
/// Reference: `trap::can_see` (`trap.cpp:319-331`): a trap that is `always_invisible` is never seen, and otherwise the creature sees it when `visibility < 0` or when it already
/// knows about it. The `visibility < 0` case is the reference's way of saying "this trap is always shown".
#[must_use]
pub fn can_see(world: &World, who: Entity, trap: &Trap, square: (i32, i32, i32)) -> bool {
    if trap.always_invisible {
        return false;
    }
    trap.visibility < 0 || knows(world, who, square)
}

/// **Whether a creature steps over a trap instead of onto it.**
///
/// Reference: `Character::avoid_trap` (`character.cpp:12182-12203`). Answers `false` for a creature with no dexterity: the port cannot roll its side of the contest, and a trap
/// that fires is the honest answer where "the creature dodges because it has no body" would not be.
#[must_use]
pub fn avoid_trap(world: &mut World, who: Entity, trap: &Trap, square: (i32, i32, i32)) -> bool {
    let Some(dexterity) = world
        .get::<crate::creature::Dexterity>(who)
        .map(|dex| dex.base)
    else {
        return false;
    };
    let dodge = world
        .get::<crate::creature::Dodge>(who)
        .map_or(0, |dodge| dodge.points);
    let seen = can_see(world, who, trap, square);
    let mut rolls = crate::singletons::take_rolls(world).unwrap_or_else(|| Rolls::new(0));
    // Reference: `dice( 3, round( dex_cur + get_skill_level( skill_dodge ) * 1.5 ) )`. The sum is rounded to an integer because the die count is an integer.
    let sides = round_half_up(f64::from(dexterity) + f64::from(dodge) * 1.5);
    let myroll = crate::melee::dice_total(&mut rolls, 3, sides);
    // Reference: three dice when the trap is seen and six when it is not - the *unseen* trap is the harder one to beat, because its roll has more dice behind it.
    let faces = if seen { 3 } else { 6 };
    let traproll = crate::melee::dice_total(&mut rolls, faces, trap.avoidance);
    crate::singletons::put_rolls(world, rolls);
    myroll >= traproll
}

/// The reference's `round`, which rounds halves away from zero (`std::round`), not towards even.
#[must_use]
pub fn round_half_up(value: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a die count is an integer in the reference too"
    )]
    let narrowed = value.round() as i32;
    narrowed
}
