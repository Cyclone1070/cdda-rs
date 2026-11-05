//! What a character has learned the contents of.
//!
//! Reference: `avatar::has_identified` and `avatar::identify` (`src/avatar.cpp:717-760`). The reference keeps a `std::set<itype_id>` on the avatar - **the identity of the type, not of the copy** - so
//! identifying one copy of a book identifies every copy, which is the rule a player relies on when they find a second printing.
//!
//! In the port that set is a component on the character entity: one piece of data, no second store, and no global. An item that is not a book can never be identified (`item::is_identifiable` is
//! `is_book()`, `src/item.cpp:10361-10364`), so the port refuses rather than recording it.

use bevy_ecs::prelude::*;
use std::collections::BTreeSet;

/// The item **type** ids a character has identified.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct IdentifiedItems {
    /// Type ids, in the character's own order of learning them.
    pub ids: BTreeSet<String>,
}

/// What came of an attempt to identify a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identification {
    /// It was new to the character and is now known.
    NewlyIdentified,
    /// They already knew this type - the reference returns early and says nothing.
    AlreadyKnown,
    /// It is not a book, so there is nothing to identify. The reference logs a `debugmsg` and does nothing.
    NotABook,
}

/// Whether a character knows what is inside this item type.
///
/// A character with no [`IdentifiedItems`] component has identified nothing, which is the reference's empty set rather than an error.
#[must_use]
pub fn has_identified(world: &World, character: Entity, item_type: &str) -> bool {
    world
        .get::<IdentifiedItems>(character)
        .is_some_and(|known| known.ids.contains(item_type))
}

/// Records that a character has learned what is in an item type.
///
/// Reference: `avatar::identify` (`src/avatar.cpp:722-760`): it returns early when the type is already known, refuses anything that is not a book, and otherwise inserts the **type** id - so every copy
/// of that type is identified from then on.
pub fn identify_book(world: &mut World, character: Entity, item_type: &str, is_book: bool) -> Identification {
    if !is_book {
        return Identification::NotABook;
    }
    if has_identified(world, character, item_type) {
        return Identification::AlreadyKnown;
    }
    if world.get::<IdentifiedItems>(character).is_none() {
        world.entity_mut(character).insert(IdentifiedItems::default());
    }
    if let Some(mut known) = world.get_mut::<IdentifiedItems>(character) {
        known.ids.insert(item_type.to_owned());
    }
    Identification::NewlyIdentified
}

/// The facts about a book that its skim summary prints.
#[derive(Debug, Clone, Copy)]
pub struct SkimFacts<'a> {
    /// The book's name, as the player sees it.
    pub book_name: &'a str,
    /// The **trainable** skill it raises, or `None` when it raises none - the reference's `can_train()` gate.
    pub skill_name: Option<&'a str>,
    /// The level it can take a reader to.
    pub level: i32,
    /// The level a reader needs, which is printed **only when non-zero**.
    pub req: i32,
    /// The intelligence it asks for, which is printed **only when non-zero**.
    pub intel: i32,
    /// How much it moves morale, which is printed **only when non-zero**.
    pub fun: i32,
    /// How long one chapter takes, in turns (seconds).
    pub time_seconds: i64,
}

/// What the game says when a character first skims a book.
///
/// Reference: `avatar::identify` (`avatar.cpp:740-770`), whose messages come in this order and with these exact words:
///
/// ```text
/// "You skim %s to find out what's in it."
/// "Can bring your %s knowledge to %d."                       // only when the skill can be trained
/// "Requires %1$s knowledge level %2$d to understand."        // only when req != 0
/// "Requires intelligence of %d to easily read."              // only when intel != 0
/// "Reading this book affects your morale by %d."             // only when book_fun_for != 0
/// "A chapter of this book takes %s to read."                 // the clipped duration
/// ```
///
/// **Three of those lines are conditional on a number being non-zero**, which is not the same as the field being absent: a book that requires intelligence 0 says nothing about intelligence, and the
/// player is not told "Requires intelligence of 0". The gate asserts the two-line book and the six-line book, and the conditional in between.
///
/// The skill's own name comes from the caller, because the reference translates it (`skill.obj().name()`); the port has no translation machinery yet and prints the id's name as given.
#[must_use]
pub fn skim_messages(facts: &SkimFacts<'_>) -> Vec<String> {
    let mut lines = vec![format!("You skim {} to find out what's in it.", facts.book_name)];
    if let Some(skill) = facts.skill_name {
        lines.push(format!("Can bring your {skill} knowledge to {}.", facts.level));
        if facts.req != 0 {
            lines.push(format!("Requires {skill} knowledge level {} to understand.", facts.req));
        }
    }
    if facts.intel != 0 {
        lines.push(format!("Requires intelligence of {} to easily read.", facts.intel));
    }
    if facts.fun != 0 {
        lines.push(format!("Reading this book affects your morale by {}.", facts.fun));
    }
    lines.push(format!(
        "A chapter of this book takes {} to read.",
        clipped_duration(facts.time_seconds)
    ));
    lines
}

/// A duration as the reference clips it for a message: **one unit, the largest that fits**, spelled out.
///
/// Reference: `clipped_time` / `to_string_clipped` (`calendar.cpp:445-556`) - seconds under a minute, minutes under an hour, hours under a day, days under a week, then weeks. The reference also has
/// season and year rungs that depend on the calendar's season length; the port stops at weeks and names the rest.
#[must_use]
pub fn clipped_duration(seconds: i64) -> String {
    /// One rung of the reference's ladder: its length in seconds, singular and plural names.
    const RUNGS: [(i64, &str, &str); 4] = [
        (60, "second", "seconds"),
        (3600, "minute", "minutes"),
        (86_400, "hour", "hours"),
        (604_800, "day", "days"),
    ];
    for (index, (length, singular, plural)) in RUNGS.iter().enumerate() {
        if seconds < *length {
            let count = match index {
                0 => seconds,
                1 => seconds / 60,
                2 => seconds / 3600,
                _ => seconds / 86_400,
            };
            return format!("{count} {}", if count == 1 { singular } else { plural });
        }
    }
    let weeks = seconds / 604_800;
    format!("{weeks} {}", if weeks == 1 { "week" } else { "weeks" })
}

/// Identifies a book and says what it can do for the reader - the whole of `avatar::identify`'s effect.
///
/// Reference: `avatar::identify` (`avatar.cpp:722-770`). The messages are pushed **only when the identification is new**, because the reference returns early on a type it already knows; a second copy
/// of the same book is silently already known, which is exactly the behaviour a player sees as "I have read this one".
pub fn identify_book_and_say(
    world: &mut World,
    character: Entity,
    item_type: &str,
    is_book: bool,
    facts: &SkimFacts<'_>,
) -> Identification {
    let outcome = identify_book(world, character, item_type, is_book);
    if outcome == Identification::NewlyIdentified {
        for line in skim_messages(facts) {
            crate::messages::push(world, &line);
        }
    }
    outcome
}
