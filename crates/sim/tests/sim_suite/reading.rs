//! Reading speed, against the reference's own comment.

#![allow(clippy::panic, clippy::expect_used)]

/// Intelligence turns into a reading time, on the curve the reference writes out in its own comment.
///
/// Reference: `Character::read_speed()` (`character.cpp:3669-3684`), whose comment is the table: *4 int = 72 seconds, 8 int = 60 seconds, 12 int = 51 seconds, 16 int = 45 seconds, 20 int = 40
/// seconds*. The answer is that duration as a **percentage of a minute**, so 60 seconds is 100 and 72 is 120 - **a lower number reads faster**, which is the opposite of what "speed" suggests.
#[test]
fn intelligence_turns_into_a_reading_time() {
    use cdda_sim::reading::read_speed_percent;

    assert_eq!(read_speed_percent(4), 120, "72 seconds is 120% of a minute");
    assert_eq!(read_speed_percent(8), 100, "**60 seconds is 100** - average intelligence reads a minute a minute");
    assert_eq!(read_speed_percent(12), 85, "51.4 truncates to 51 seconds");
    assert_eq!(read_speed_percent(16), 75, "45 seconds");
    assert_eq!(read_speed_percent(20), 66, "40 seconds");
    // The clamp: a very high intelligence still takes a second.
    // 400 int is 3.46 seconds, which is not the clamp; **1000** is, at 1.4 seconds.
    assert_eq!(read_speed_percent(1000), 1, "the floor is one second, which is 1% of a minute once divided");
}

/// Reading a book takes a time the book, the reader's speed, their eyes and their intelligence all scale.
///
/// Reference: `Character::time_to_read` (`character.cpp:12411-12436`):
///
/// ```text
/// time_duration retval = type->time * reading_speed / 100;            // speed
/// retval *= std::min( fine_detail_vision_mod(), reader.fine_detail_vision_mod() );
/// if( type->intel > effective_int ) retval += type->time * ( from_seconds( type->intel - effective_int ) / 1_minutes );
/// if( !has_identified( book.typeId() ) ) retval /= 10;                 // skimming
/// ```
///
/// **The intelligence penalty is counted in whole minutes of shortfall**, because the difference is divided by `1_minutes` before it multiplies the book's time - so **a shortfall of 30 seconds adds
/// nothing at all**, and 60 adds a whole copy of the book's base time. That integer division is the rule this gate exists for.
#[test]
fn a_book_takes_the_time_the_book_the_reader_and_their_eyes_say() {
    use cdda_sim::reading::{time_to_read, skimmed};

    assert_eq!(time_to_read(60, 100, 1, 0), 60, "a minute for a minute");
    assert_eq!(time_to_read(60, 120, 1, 0), 72, "a slow reader takes longer");
    assert_eq!(time_to_read(60, 100, 2, 0), 120, "**bad eyes double it**");
    assert_eq!(time_to_read(60, 100, 1, 30), 60, "**a shortfall under a minute adds nothing**");
    assert_eq!(time_to_read(60, 100, 1, 60), 120, "one minute short is one more book's worth");
    assert_eq!(time_to_read(60, 100, 1, 90), 120, "and 90 seconds is still one minute");
    assert_eq!(skimmed(65), 6, "skimming divides by ten and truncates");
}

/// Being thirsty slows a character down along a set of thresholds, not a straight line.
///
/// Reference: `Character::thirst_speed_penalty` (`character.cpp:12439-12451`): a `multi_lerp` over four points - **40 thirst is no penalty, 300 is −25, 600 is −50, and 1200 is −75**, where the
/// character dies. `multi_lerp` (`cata_utility.cpp:267-286`) walks past every threshold at or below the value, answers the **first** point's value below the range and the **last** point's above it, and
/// interpolates between the two that bracket it. **The `static_cast<int>` truncates toward zero**, so halfway to 300 is −12, not −13.
#[test]
fn thirst_slows_a_character_along_its_thresholds() {
    use cdda_sim::needs::thirst_speed_penalty;

    assert_eq!(thirst_speed_penalty(0), 0, "below the first threshold, the first value - and `<= x` means 40 itself also answers");
    assert_eq!(thirst_speed_penalty(40), 0, "40 is exactly the first threshold: still no penalty");
    assert_eq!(thirst_speed_penalty(170), -12, "**halfway to 300 is -12.5, and the cast truncates toward zero**");
    assert_eq!(thirst_speed_penalty(300), -25, "at the second threshold");
    assert_eq!(thirst_speed_penalty(450), -37, "-37.5 truncates to -37, not -38");
    assert_eq!(thirst_speed_penalty(600), -50, "at the third");
    assert_eq!(thirst_speed_penalty(1200), -75, "at the fourth, where a character dies");
    assert_eq!(thirst_speed_penalty(5000), -75, "and beyond it, the last value");
}

/// Being starved slows a character by their fat category, and only when they actually need food.
///
/// Reference: `Character::kcal_speed_penalty` (`character.cpp:4531-4549`): nothing when `!needs_food()`, nothing when `get_kcal_percent() > 0.95`, otherwise `std::round( multi_lerp( thresholds,
/// get_bmi_fat() ) )` over **fat category** values, not BMI - `0 -> -90`, `emaciated (1) -> -50`, `underweight (2) -> -25`, `normal (3) -> 0` (`game_constants.h:170-179`).
///
/// **`std::round` takes halves away from zero**, so −12.5 becomes **−13** - the opposite of the truncation `thirst_speed_penalty` uses, and the reason both gates exist.
#[test]
fn being_starved_slows_a_character_by_their_fat_category() {
    use cdda_sim::needs::kcal_speed_penalty;

    assert_eq!(kcal_speed_penalty(false, 0.5, 1.0), 0, "**not needing food means no penalty**, however thin");
    assert_eq!(kcal_speed_penalty(true, 0.99, 1.0), 0, "a character over 95% fed does not pay for weight");
    assert_eq!(kcal_speed_penalty(true, 0.5, 3.0), 0, "normal weight costs nothing");
    assert_eq!(kcal_speed_penalty(true, 0.5, 2.0), -25, "underweight");
    assert_eq!(kcal_speed_penalty(true, 0.5, 1.0), -50, "emaciated");
    assert_eq!(kcal_speed_penalty(true, 0.5, 0.0), -90, "**zero fat is the floor**");
    assert_eq!(kcal_speed_penalty(true, 0.5, 2.5), -13, "halfway is -12.5, and `round` takes it away from zero to -13");
    assert_eq!(kcal_speed_penalty(true, 0.5, 5.0), 0, "above normal the last value stands: no bonus for fat");
}

/// A book's own time, put through every scaling the reader brings - and skimmed when they have not identified it.
///
/// Reference: `Character::time_to_read` (`character.cpp:12411-12436`) applies the reader's speed, the worse of two vision modifiers and the intelligence shortfall, **and then** divides by ten for a
/// book the character has not identified. **The order matters**: skimming tenths a time that has already been stretched, so a slow reader in the dark skims a longer ten minutes than a quick one.
#[test]
fn a_book_s_own_time_takes_every_scaling_and_the_skim_last() {
    use cdda_sim::reading::{book_time_seconds, skimmed};

    // A book of 35 minutes, as the corpus writes it (`"time": "35 m"`).
    let base = 35 * 60;
    assert_eq!(book_time_seconds(base, 100, 1, 0, true), base, "no scaling at all: the book's own time");
    assert_eq!(book_time_seconds(base, 120, 1, 0, true), 2520, "a slow reader: 35 minutes becomes 42");
    assert_eq!(book_time_seconds(base, 100, 2, 0, true), 4200, "bad eyes double it");
    assert_eq!(book_time_seconds(base, 100, 1, 60, true), 4200, "a minute short of its intelligence adds the book again");
    assert_eq!(
        book_time_seconds(base, 100, 1, 0, false),
        skimmed(base),
        "**unidentified skims the whole scaled time**, not the book's own"
    );
    assert!(
        book_time_seconds(base, 120, 1, 0, false) > skimmed(base),
        "and a slow reader still skims more than a fast one's skim"
    );
}

/// How much fun a book is depends on who is reading it, and on whether there is anything left in it.
///
/// Reference: `Character::book_fun_for` and `fun_to_read` (`character.cpp:11953-11992`). The rules run **in this order**, and the order is what makes them:
///
/// * a cannibal (or psychopath, or sapiovore) reading *To Serve Man* gets the **absolute** fun - a −3 book becomes 3;
/// * otherwise a spiritual character gets **triple** the absolute fun of an `INSPIRATIONAL` book, but only in the `else` branch, so a cannibal spiritualist gets the cannibal rule alone;
/// * `LOVES_BOOKS` adds **one**; `HATES_BOOKS` **zeroes** a book with positive fun rather than flipping it, and takes one **more** off a book that was already no fun;
/// * and only then, a book with more than one fun whose **chapters are all read** is halved.
#[test]
fn how_much_fun_a_book_is_depends_on_who_reads_it() {
    use cdda_sim::reading::{book_fun_for, fun_to_read};

    let book = |fun: i32, chapters: i32, remaining: i32| cdda_sim::reading::FunBook { fun, chapters, remaining };
    let plain = book(2, 0, 0);
    assert_eq!(book_fun_for(plain, &[], false, false), 2, "nobody's traits apply");
    assert!(fun_to_read(plain, &[], false, false), "and a book with fun is fun to read");

    assert_eq!(book_fun_for(plain, &["LOVES_BOOKS"], false, false), 3, "loves books: one more");
    assert_eq!(book_fun_for(plain, &["HATES_BOOKS"], false, false), 0, "**hates books: zeroes a good book rather than flipping it**");
    assert!(!fun_to_read(plain, &["HATES_BOOKS"], false, false), "so it is not fun to read any more");
    assert_eq!(book_fun_for(book(-1, 0, 0), &["HATES_BOOKS"], false, false), -2, "and takes one more off a bad one");

    assert_eq!(book_fun_for(plain, &["SPIRITUAL"], true, false), 6, "**spiritual and inspirational: tripled**");
    assert_eq!(book_fun_for(plain, &["SPIRITUAL"], false, false), 2, "without the flag, the trait does nothing");
    assert_eq!(book_fun_for(book(-3, 0, 0), &["CANNIBAL"], false, true), 3, "a cannibal enjoys the human cookbook in reverse");

    assert_eq!(book_fun_for(book(4, 6, 0), &[], false, false), 2, "**chapters all read halves it**");
    assert_eq!(book_fun_for(book(4, 6, 3), &[], false, false), 4, "with chapters left, no halving");
    assert_eq!(book_fun_for(book(1, 6, 0), &[], false, false), 1, "and a book of fun one is not halved: the test is `> 1`");
}

/// A reader too sad to enjoy a book they have already read is told so - and an unread book does not need cheering up.
///
/// Reference: `avatar::read`'s condition table (`character.cpp:12267-12271`): `if( !fun_to_read( book ) && !has_morale_to_read() && has_identified( book.typeId() ) )` is `MORALE_LOW`, and
/// `has_morale_to_read` is **`get_morale_level() >= -40`** (`character_morale.cpp:171-174`).
///
/// **All three conditions must hold**, and the last one is the surprising half: an **unidentified** book can always be read, however miserable the reader, because there is still something to learn from it.
#[test]
fn a_sad_reader_is_refused_only_by_a_book_they_already_know() {
    use cdda_sim::morale::has_morale_to_read;
    use cdda_sim::reading::morale_permits_reading;

    assert!(has_morale_to_read(-40), "**-40 is the floor and it is inclusive**");
    assert!(!has_morale_to_read(-41), "one below it is too sad");

    assert!(morale_permits_reading(true, -100, true), "a fun book is fine however sad the reader");
    assert!(morale_permits_reading(false, -39, true), "a dull book is fine while morale holds");
    assert!(!morale_permits_reading(false, -41, true), "**a dull book already read, read while miserable, is refused**");
    assert!(morale_permits_reading(false, -100, false), "but an unread book is always readable: there is still something to learn");
}

/// What a reader could get out of a book: nothing at all, nothing more, or something.
///
/// Reference: `Character::get_book_mastery` (`character.cpp:11925-11951`), four verdicts in a fixed order: an **unidentified** book cannot be judged (`CANT_DETERMINE`); a book that teaches **no
/// skill** is `MASTERED` at once; too low a level is `CANT_UNDERSTAND`; **at or above** the level it teaches is `MASTERED`; and anything between is `LEARNING`.
///
/// **Two boundaries carry the rule:** `req > knowledge` is the understanding gate, so **exactly the required level understands** - and `knowledge >= level` is the mastery gate, so **exactly the level it
/// teaches is already mastered**, not one short.
#[test]
fn what_a_reader_could_get_out_of_a_book() {
    use cdda_sim::reading::{BookMastery, book_mastery};

    assert_eq!(book_mastery(false, true, 9, 5, 7), BookMastery::CantDetermine, "an unidentified book cannot be judged");
    assert_eq!(book_mastery(true, false, 0, 0, 0), BookMastery::Mastered, "a book that teaches nothing is mastered on sight");
    assert_eq!(book_mastery(true, true, 4, 5, 7), BookMastery::CantUnderstand, "one level short of the requirement");
    assert_eq!(book_mastery(true, true, 5, 5, 7), BookMastery::Learning, "**exactly the requirement understands**");
    assert_eq!(book_mastery(true, true, 6, 5, 7), BookMastery::Learning, "and so does anything below what it teaches");
    assert_eq!(book_mastery(true, true, 7, 5, 7), BookMastery::Mastered, "**exactly the level it teaches is mastery**");
    assert_eq!(book_mastery(true, true, 9, 5, 7), BookMastery::Mastered, "and anything above it");
}
