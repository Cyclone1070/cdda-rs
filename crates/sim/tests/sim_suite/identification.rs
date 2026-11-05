//! Identification of book types, against the reference's set-on-the-avatar rule.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::identification::{has_identified, identify_book, Identification, IdentifiedItems};

/// Identifying one copy identifies every copy of the type - and identifying again says nothing new.
///
/// Reference: `avatar::has_identified` / `avatar::identify` (`src/avatar.cpp:717-760`). The set holds **item type ids**, so a second copy of the same book is identified without being read; the
/// reference returns early when the type is already known rather than repeating its messages.
#[test]
fn a_book_is_identified_by_type_not_by_copy() {
    let mut world = World::new();
    let reader = world.spawn_empty().id();

    assert!(!has_identified(&world, reader, "adv_chemistry"), "**nothing is known before anything is read**, and a missing component means nothing is known rather than an error");

    assert_eq!(identify_book(&mut world, reader, "adv_chemistry", true), Identification::NewlyIdentified);
    assert!(has_identified(&world, reader, "adv_chemistry"), "the first copy teaches the type");
    assert!(
        has_identified(&world, reader, "adv_chemistry"),
        "so a second copy of the same type is known without being opened - **the set is of types, not of copies**"
    );

    assert_eq!(
        identify_book(&mut world, reader, "adv_chemistry", true),
        Identification::AlreadyKnown,
        "and the reference returns early the second time"
    );
}

/// Anything that is not a book cannot be identified, and nothing is recorded for it.
///
/// Reference: `item::is_identifiable` is `is_book()` (`src/item.cpp:10361-10364`), and `avatar::identify` refuses anything else with a `debugmsg`.
#[test]
fn a_thing_that_is_not_a_book_cannot_be_identified() {
    let mut world = World::new();
    let reader = world.spawn_empty().id();

    assert_eq!(identify_book(&mut world, reader, "rock", false), Identification::NotABook);
    assert!(!has_identified(&world, reader, "rock"), "**a refusal leaves no trace**, so the rock is not half-identified");
    assert!(
        world.get::<IdentifiedItems>(reader).is_none(),
        "and the character has not even gained the component"
    );
}
