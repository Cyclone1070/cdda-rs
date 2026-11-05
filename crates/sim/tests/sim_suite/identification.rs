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

/// Skimming a book tells the reader what it can do for them, in the reference's own order.
///
/// Reference: `avatar::identify` (`avatar.cpp:740-770`), which prints in this sequence: the skim line, the skill it can raise, the level it needs (**only when non-zero**), the intelligence it wants
/// (**only when non-zero**), the morale it moves (**only when non-zero**), and how long a chapter takes. A book that states none of the extras is a two-line book.
#[test]
fn skimming_a_book_says_what_it_can_do_for_you() {
    use cdda_sim::identification::{SkimFacts, skim_messages};

    let plain = skim_messages(&SkimFacts {
        book_name: "Advanced Physical Chemistry",
        skill_name: None,
        level: 0,
        req: 0,
        intel: 0,
        fun: 0,
        time_seconds: 35 * 60,
    });
    assert_eq!(
        plain,
        vec![
            "You skim Advanced Physical Chemistry to find out what's in it.".to_string(),
            "A chapter of this book takes 35 minutes to read.".to_string(),
        ],
        "**a bare book says two things**: that it was skimmed, and what a chapter costs"
    );

    let rich = skim_messages(&SkimFacts {
        book_name: "A Book",
        skill_name: Some("chemistry"),
        level: 7,
        req: 5,
        intel: 9,
        fun: -2,
        time_seconds: 90,
    });
    assert_eq!(
        rich,
        vec![
            "You skim A Book to find out what's in it.".to_string(),
            "Can bring your chemistry knowledge to 7.".to_string(),
            "Requires chemistry knowledge level 5 to understand.".to_string(),
            "Requires intelligence of 9 to easily read.".to_string(),
            "Reading this book affects your morale by -2.".to_string(),
            "A chapter of this book takes 1 minute to read.".to_string(),
        ],
        "and a rich one says all six, in the reference's order - and **90 seconds is already a minute**, because the ladder asks for less than a minute before it counts seconds"
    );

    // **The zeros are the rule**: a requirement of zero is not printed at all, and neither is a morale of zero.
    let zero_requirement = skim_messages(&SkimFacts {
        book_name: "A Book",
        skill_name: Some("chemistry"),
        level: 3,
        req: 0,
        intel: 0,
        fun: 0,
        time_seconds: 60,
    });
    assert_eq!(zero_requirement.len(), 3, "the skim line, the skill line and the chapter line - and nothing about a requirement of zero");
    assert!(zero_requirement[2].contains("1 minute"), "**a single minute is singular**: {}", zero_requirement[2]);
}

/// Skimming a book says what it can do for you **in the game's log**, once, at the moment it is learned.
///
/// Reference: `avatar::identify` (`avatar.cpp:722-770`) - it returns early when the type is already known, so the messages appear **only the first time** a type is identified; every later copy is
/// silent.
#[test]
fn skimming_puts_its_lines_in_the_log_once() {
    use cdda_sim::identification::{SkimFacts, identify_book_and_say};
    use cdda_sim::messages;

    let mut world = World::new();
    let reader = world.spawn_empty().id();
    let facts = SkimFacts {
        book_name: "A Book",
        skill_name: Some("chemistry"),
        level: 7,
        req: 0,
        intel: 0,
        fun: 0,
        time_seconds: 60,
    };

    assert_eq!(identify_book_and_say(&mut world, reader, "a_book", true, &facts), Identification::NewlyIdentified);
    let lines = messages::lines(&world);
    assert_eq!(lines.len(), 3, "**the skim line, the skill line and the chapter line** - and none about a requirement of zero: {lines:?}");
    assert_eq!(lines[0], "You skim A Book to find out what's in it.");

    assert_eq!(
        identify_book_and_say(&mut world, reader, "a_book", true, &facts),
        Identification::AlreadyKnown,
        "a second copy is already known"
    );
    assert_eq!(messages::lines(&world).len(), 3, "**and it says nothing the second time** - the early return is the silence");
}
