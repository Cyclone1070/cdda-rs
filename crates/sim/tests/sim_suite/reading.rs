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
