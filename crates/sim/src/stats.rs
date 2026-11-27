//! The four numbers a character is made of.
//!
//! Reference: `Character::str_cur`, `dex_cur`, `int_cur`, `per_cur` (`character.h`), set by `reset_stats` to the maximum and moved by pain, hunger, drugs and mutations. One
//! creature's four numbers, so they live on the creature - the same choice `stamina` and `pain` already made.

use bevy_ecs::prelude::{Component, Entity, World};

/// What a creature's body and mind are worth right now.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Strength.
    pub strength: i32,
    /// Dexterity.
    pub dexterity: i32,
    /// Intelligence, which a trade's price and a book's difficulty both read.
    pub intelligence: i32,
    /// Perception.
    pub perception: i32,
}

/// The `Character` default: eight in everything, which is what `reset_stats` sets before any trait or drug moves it.
impl Default for Stats {
    fn default() -> Self {
        Self {
            strength: 8,
            dexterity: 8,
            intelligence: 8,
            perception: 8,
        }
    }
}

/// A creature's four numbers, or the default eight when it has never been given any.
///
/// **The default is the reference's**, not a zero: a creature with no `Stats` component is an ordinary person, and a rule that read zero would make every stranger a fool and a
/// weakling. The reference's own answer for a character that has not been randomized is the same eight.
#[must_use]
pub fn stats_of(world: &World, creature: Entity) -> Stats {
    world.get::<Stats>(creature).copied().unwrap_or_default()
}

/// Gives a creature its four numbers.
pub fn set_stats(world: &mut World, creature: Entity, stats: Stats) {
    world.entity_mut(creature).insert(stats);
}
