//! Identification of book types, against the reference's set-on-the-avatar rule.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

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
    let chapter_line = zero_requirement.get(2).cloned().unwrap_or_default();
    assert!(chapter_line.contains("1 minute"), "**a single minute is singular**: {chapter_line}");
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
    assert_eq!(lines.first().map(String::as_str), Some("You skim A Book to find out what's in it."));

    assert_eq!(
        identify_book_and_say(&mut world, reader, "a_book", true, &facts),
        Identification::AlreadyKnown,
        "a second copy is already known"
    );
    assert_eq!(messages::lines(&world).len(), 3, "**and it says nothing the second time** - the early return is the silence");
}

/// A message carries a severity, an empty message is not stored, and debug chatter is dropped.
///
/// Reference: `messages.cpp:174-205`. `add_msg_string( msg )` forwards to **`m_neutral`** - not `m_info` - so an untyped message is neutral. Before anything is stored, `add_msg_string` returns on
/// **`msg.empty()`** and on **`m_debug` while `debug_mode` is off**. The port's log stored an empty string as a line, which is the shape of bug this gate exists for: an empty line in the log is a line
/// the player sees and the turn digest counts.
///
/// The severities are `game_message_type` (`enums.h:361-379`), in its own order.
#[test]
fn a_message_has_a_severity_and_empty_or_debug_lines_are_not_stored() {
    use cdda_sim::messages::{self, MessageKind};

    let mut world = World::new();
    messages::push(&mut world, "");
    assert!(messages::entries(&world).is_empty(), "**an empty message is not stored at all**");

    messages::push(&mut world, "plain");
    assert_eq!(
        messages::entries(&world).first().map(|entry| entry.kind),
        Some(MessageKind::Neutral),
        "**an untyped message is neutral**, which is the reference's default and not one of the louder kinds"
    );

    messages::push_kind(&mut world, "debug chatter", MessageKind::Debug);
    assert_eq!(messages::lines(&world), vec!["plain".to_string()], "**debug chatter is dropped while debug mode is off**");

    messages::push_kind(&mut world, "you are hurt", MessageKind::Bad);
    let entries = messages::entries(&world);
    assert_eq!(entries.len(), 2, "the text and the severity live together, in order");
    assert_eq!(entries.get(1).map(|entry| entry.text.as_str()), Some("you are hurt"));
    assert_eq!(
        entries.get(1).map(|entry| entry.kind),
        Some(MessageKind::Bad),
        "and the severity is the one the caller chose"
    );
}

/// A high character gets smileys in their messages, on a curve that rises slowly and never reaches three.
///
/// Reference: `modify_msg_with_exclamations` (`messages.cpp:255-268`), whose comment states the curve outright: *"Slowly rises. 0 at 0, 1 at 22.5, 2 at 45 mins. Asymptote at 3."* -
///
/// ```text
/// const float max_per_msg = 3 - 3 / ( 1 + 0.001 * in_minutes * in_minutes );
/// ```
///
/// **The comment is the oracle**, which is why the gate asserts the three datapoints it names rather than a value picked from the formula's shape: 22.5 minutes is **just over** one smiley and 45
/// **just over** two, and the cap approaches three without ever reaching it.
#[test]
fn a_high_character_gets_smileys_on_a_slowly_rising_curve() {
    use cdda_sim::messages::weed_smiley_cap;

    assert!((weed_smiley_cap(0.0) - 0.0).abs() < 1e-6, "**zero minutes, no smileys**");
    assert!((weed_smiley_cap(22.5) - 1.0).abs() < 0.02, "the reference's own first datapoint: about one at 22.5 minutes");
    assert!((weed_smiley_cap(45.0) - 2.0).abs() < 0.02, "and about two at 45");
    assert!(weed_smiley_cap(450.0) < 3.0, "**the asymptote is three and is never reached**");
    assert!(weed_smiley_cap(450.0) > 2.9, "though at 450 minutes it is already 2.97");
    assert!(weed_smiley_cap(45.0) > weed_smiley_cap(22.5), "and the curve only rises");
}

/// A smiley only goes where a word is long enough and not on the reference's forbidden list.
///
/// Reference: `modify_msg_with_exclamations` (`messages.cpp:271-318`). The end of the message is always a candidate; a space is a candidate unless it follows **the first word**, **a forbidden word**
/// (`" that"`, `" with"`, `" this"`, `" over"`, `" your"`, `" onto"`), **a word of one to four characters**, or **another space**.
///
/// **The forbidden list is compared with its leading space**, which is why `" that"` is five characters: the reference builds `msg.substr( prev_space, j - prev_space )` and that slice starts at the
/// space. A port that compared bare words would let a smiley after "that" through and be wrong in exactly the case the list exists for.
#[test]
fn a_smiley_only_goes_where_a_word_is_long_enough() {
    use cdda_sim::messages::smiley_slots;

    let fine = "You feel fine today.";
    assert_eq!(
        smiley_slots(fine),
        vec![20, 8, 13],
        "**the end is collected first**, then the spaces: `that` word is forbidden, but `feel` and `fine` are not - and the first word's space is skipped"
    );

    let short = "You that cat sat.";
    assert_eq!(
        smiley_slots(short),
        vec![17],
        "`that` is forbidden and `cat`/`sat` are too short, so only the end is left"
    );

    assert_eq!(smiley_slots("Hi"), vec![2], "a message with no spaces still has its end");
}

/// Inserting a smiley at a position shifts every later candidate by its own three characters.
///
/// Reference: `modify_msg_with_exclamations` (`messages.cpp:318-334`): each insertion writes `" :)"` (**three bytes, including the space**) at its position and then adds 3 to **every remaining
/// candidate beyond it** - the reference's own comment says *"we increment to account for the insertion of the smiley string"*.
///
/// **The positions handed in are the ones collected before any insertion**, so the bookkeeping is the rule: a port that inserted at the raw offsets in order would drift by three bytes per smiley and
/// quietly mangle the message.
#[test]
fn inserting_a_smiley_shifts_every_later_candidate() {
    use cdda_sim::messages::apply_smileys;

    let message = "You feel fine today.";
    let one = apply_smileys(message, &[20, 8]);
    assert_eq!(
        one, "You feel :) fine today. :)",
        "**two smileys, in the order drawn**, each three bytes: the later candidate shifted by the one inserted before it"
    );
    assert_eq!(apply_smileys(message, &[]), message, "and no smileys leaves the message alone");
    assert_eq!(apply_smileys(message, &[8]), "You feel :) fine today.", "a single early smiley");
}
