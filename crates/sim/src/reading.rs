//! How fast a character reads.
//!
//! Reference: `Character::read_speed()` (`character.cpp:3669-3684`), which turns intelligence into a duration per "unit" of reading, clamps it, and answers **a percentage of a minute** - the units
//! every reading activity multiplies its time by.

/// The percentage of a minute a character with this intelligence takes to read one unit.
///
/// Reference: `Character::read_speed()`:
///
/// ```text
/// const float intel = 2.0f + get_int() / 8.0f;
/// time_duration ret = 180_seconds / intel;
/// ret = enchantment_cache->modify_value( READING_SPEED_MULTIPLIER, ret );   // P5, not ported
/// if( ret < 1_seconds ) { ret = 1_seconds; }
/// return ret * 100 / 1_minutes;
/// ```
///
/// **The comment in the reference gives the whole curve and is the gate's oracle:** *4 int = 72 seconds, 8 int = 60 seconds, 12 int = 51 seconds, 16 int = 45 seconds, 20 int = 40 seconds* - nine
/// percent a point at the bottom, under four at the top. The duration is a whole number of seconds, so 51.4 becomes **51**, and the answer is `seconds * 100 / 60`.
#[must_use]
pub fn read_speed_percent(intelligence: i32) -> i32 {
    let intel = 2.0 + f64::from(intelligence) / 8.0;
    // The duration truncates to whole seconds, as `time_duration` does when 180 is divided by a float.
    #[allow(clippy::cast_possible_truncation)]
    let mut seconds = (180.0 / intel).trunc() as i32;
    if seconds < 1 {
        seconds = 1;
    }
    seconds * 100 / 60
}
