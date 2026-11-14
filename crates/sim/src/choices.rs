//! The answers a menu is given.
//!
//! The reference draws menus and asks the player (`uilist`, `query_int`). A headless port cannot draw, and it must not quietly pick the first option -
//! so the answer is a **queue** on one entity: a scripted playtest pushes what it would have typed, and the UI pushes what the player clicked. Both
//! reach the game the same way, which is why this is a store and not a parameter threaded through the effect runner.

use std::collections::VecDeque;

use bevy_ecs::prelude::*;

/// The answers the game will be given, oldest first.
///
/// A number is an index into the menu. [`CANCELLED`] is walking away, which the reference spells as the cancel key.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices(pub VecDeque<i32>);

/// The marker that says a single entity holds the queued answers.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheChoices;

/// The answer that means "nothing": the player walked away from the menu.
pub const CANCELLED: i32 = -1;

/// Queues the answers a menu will be given, oldest first, replacing whatever was queued.
pub fn set_choices(world: &mut World, answers: impl IntoIterator<Item = i32>) {
    let holder = crate::singletons::singleton::<Choices, TheChoices>(world);
    world
        .entity_mut(holder)
        .insert(Choices(answers.into_iter().collect()));
}

/// Takes the next answer, or nothing when nobody queued one.
///
/// Nothing is **not** a cancel: a menu that was never answered is a question the game could not ask, and the caller refuses rather than guessing. That is
/// the difference between a scripted playtest with a missing step and a player who pressed escape.
#[must_use]
pub fn take(world: &mut World) -> Option<i32> {
    let holder = crate::singletons::holder::<TheChoices>(world)?;
    world.get_mut::<Choices>(holder)?.0.pop_front()
}

/// How many answers are still queued, which a test or a dump reads.
#[must_use]
pub fn queued(world: &World) -> usize {
    crate::singletons::holder::<TheChoices>(world)
        .and_then(|holder| world.get::<Choices>(holder))
        .map_or(0, |choices| choices.0.len())
}
