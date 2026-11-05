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
