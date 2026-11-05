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
