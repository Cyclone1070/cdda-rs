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
