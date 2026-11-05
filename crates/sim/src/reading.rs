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

/// How long a book takes to read, before anything about the book's identity is known.
///
/// Reference: `Character::time_to_read` (`character.cpp:12411-12436`), the three numeric steps that need no book data:
///
/// ```text
/// time_duration retval = type->time * reading_speed / 100;
/// retval *= std::min( fine_detail_vision_mod(), reader.fine_detail_vision_mod() );
/// if( type->intel > effective_int ) retval += type->time * ( from_seconds( type->intel - effective_int ) / 1_minutes );
/// ```
///
/// **The intelligence penalty is counted in whole minutes**, because the shortfall in seconds is divided by `1_minutes` before it multiplies - so **a shortfall of 30 seconds adds nothing**, and 90
/// seconds adds the same as 60. That is the integer division the gate exists for, and it is the sort of rule a hand-written "...plus a bit per point" would get wrong in the generous direction.
///
/// The caller supplies the **smaller** of the two vision modifiers, which is the reference's `std::min`: the worse eyes win.
#[must_use]
pub fn time_to_read(base_seconds: i64, speed_percent: i32, vision_modifier: i32, intel_shortfall: i32) -> i64 {
    let mut seconds = base_seconds * i64::from(speed_percent) / 100;
    seconds *= i64::from(vision_modifier);
    if intel_shortfall > 0 {
        // Whole minutes of shortfall, in the reference's own order: the duration is divided before it multiplies.
        seconds += base_seconds * i64::from(intel_shortfall / 60);
    }
    seconds
}

/// What skimming a book the character has not identified costs: a tenth of the time.
///
/// Reference: `if( !has_identified( book.typeId() ) ) { retval /= 10; }` (`character.cpp:12433-12435`).
#[must_use]
pub fn skimmed(seconds: i64) -> i64 {
    seconds / 10
}
