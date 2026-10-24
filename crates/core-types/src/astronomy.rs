//! Where the sun is, and how much light it gives.
//!
//! Reference: `sun_azimuth_altitude` and `sun_light_at` (`src/calendar.cpp:123-421`). The chain is astronomical - mean longitude, mean anomaly, ecliptic longitude,
//! equatorial rotation, sidereal time, the horizontal transform - and every constant in it is the reference's, not an astronomer's.
//!
//! Nothing here is trusted on its own: the port is gated against 424 samples the reference printed (`corpus/astronomy/sun.json`), including the five intermediates the
//! chain passes through, because a curve that is plausible and a few minutes off is the failure mode that matters at dawn.
//!
//! **The year length is an integer, and the reference truncates it.** `to_days<int>( calendar::year_length() )` (`src/calendar.cpp:140`) means a four-day year turns the sky
//! ninety degrees a day; passing the days as a fraction instead keeps every value plausible and every value wrong.

use std::f64::consts::{PI, TAU};

/// The reference's default location, `src/calendar.cpp:31`, in radians.
pub const LATITUDE: f64 = 42.36_f64.to_radians();
/// The reference's default location, `src/calendar.cpp:31`, in radians. Negative is west.
pub const LONGITUDE: f64 = -71.06_f64.to_radians();

/// A year is four seasons of 91 days by default, `calendar::year_length()`.
///
/// The season length is an option and the year length follows from it, so this is a default rather than a constant of nature: every function here takes the calendar it is
/// asked about.
pub const YEAR_LENGTH_DAYS: f64 = 4.0 * 91.0;
/// Turns (seconds) in one game day.
pub const TURNS_PER_DAY: f64 = 24.0 * 3600.0;

/// The obliquity of the ecliptic, held fixed at its J2000 value.
pub const OBLIQUITY_DEGREES: f64 = 23.439_279;

/// The sun's right ascension and declination, in radians, from the days since the epoch.
///
/// Reference: `sun_ra_declination` (`src/calendar.cpp:123-166`). "This derivation is mostly from
/// <https://en.wikipedia.org/wiki/Position_of_the_Sun>" - the epoch is Greenwich midnight on the vernal equinox, which the game calendar can name exactly, so the arithmetic
/// works for a year that is not the Earth's.
#[must_use]
pub fn ra_declination(days_since_epoch: f64, year_length_days: i32) -> (f64, f64) {
    let angle_per_day = TAU / f64::from(year_length_days.max(1));
    let mean_longitude = angle_per_day * days_since_epoch;
    // Roughly seventy-seven degrees between the mean longitude and the mean anomaly at the epoch; the drift over a game year is neglected, as the reference neglects it.
    let mean_anomaly = 77.0_f64.to_radians() + mean_longitude;
    // The non-circularity of the orbit: a first-order term and a smaller refinement.
    let ecliptic_longitude = mean_longitude
        + 1.915_f64.to_radians() * mean_anomaly.sin()
        + 0.020_f64.to_radians() * (2.0 * mean_anomaly).sin();
    let obliquity = OBLIQUITY_DEGREES.to_radians();
    let x = ecliptic_longitude.cos();
    let y = ecliptic_longitude.sin() * obliquity.cos();
    let z = ecliptic_longitude.sin() * obliquity.sin();
    (y.atan2(x), z.clamp(-1.0, 1.0).asin())
}

/// The sidereal time at a longitude, in radians.
///
/// Reference: `sidereal_time_at` (`src/calendar.cpp:168-190`). At the epoch - Greenwich midnight on the vernal equinox - the sidereal time is a half turn, because midnight
/// puts the sun on the other side of the sky; it advances a turn a day **plus one a year**, which is the extra turn a sidereal day saves over a solar one.
#[must_use]
pub fn sidereal_time(days_since_epoch: f64, year_length_days: i32, longitude: f64) -> f64 {
    let angle_per_day = TAU / f64::from(year_length_days.max(1));
    PI + (TAU + angle_per_day) * days_since_epoch + longitude
}

/// The sun's azimuth and altitude, in radians. Azimuth is measured from the **south**, turning positive to the west.
///
/// Reference: `sun_azimuth_altitude` (`src/calendar.cpp:205-256`): equatorial coordinates to hour-angle, hour-angle to horizontal, which is where the latitude enters.
#[must_use]
pub fn azimuth_altitude(
    days_since_epoch: f64,
    year_length_days: i32,
    latitude: f64,
    longitude: f64,
) -> (f64, f64) {
    let (right_ascension, declination) = ra_declination(days_since_epoch, year_length_days);
    let hour_angle = sidereal_time(days_since_epoch, year_length_days, longitude) - right_ascension;
    let intermediate_x = hour_angle.cos() * declination.cos();
    let intermediate_y = hour_angle.sin() * declination.cos();
    let intermediate_z = declination.sin();
    let horizontal_x = -intermediate_x * latitude.sin() + intermediate_z * latitude.cos();
    let horizontal_y = intermediate_y;
    let horizontal_z = intermediate_x * latitude.cos() + intermediate_z * latitude.sin();
    let azimuth = normalize(-horizontal_y.atan2(horizontal_x) + PI);
    (azimuth, horizontal_z.clamp(-1.0, 1.0).asin())
}

/// The sunlight an altitude gives, on the reference's scale of 0 to 125.
///
/// Reference: `sun_light_at` (`src/calendar.cpp:401-422`). The curve itself lives in [`crate::light::sun_light_from_altitude`], which is where the light system already
/// reads it and what its own gate compares; this is the same answer in the unit the reference computes it in, so that the sun's chain has one entry point and the game
/// has one curve.
#[must_use]
pub fn light_at(altitude: f64) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let degrees = altitude.to_degrees() as f32;
    crate::light::sun_light_from_altitude(degrees)
}

/// Wraps an angle into `[0, 2π)`.
///
/// Reference: `normalize` (`src/units_utility.cpp:10-17`), which adds the modulus back for a negative answer rather than leaving it below zero.
#[must_use]
pub fn normalize(angle: f64) -> f64 {
    let reduced = angle % TAU;
    if reduced < 0.0 { reduced + TAU } else { reduced }
}

/// The altitude the sun has to reach before any light falls, and the three dawns after it.
///
/// Reference: `astronomical_dawn`, `nautical_dawn`, `civil_dawn` and `sunrise_angle` (`src/calendar.cpp:39-42`).
pub const ASTRONOMICAL_DAWN: f64 = -18.0_f64.to_radians();
/// Six degrees above the astronomical dawn: colour in the sky, but nothing can be seen by it.
pub const NAUTICAL_DAWN: f64 = -12.0_f64.to_radians();
/// Six above that: outlines are visible outdoors.
pub const CIVIL_DAWN: f64 = -6.0_f64.to_radians();
/// The sun's upper limb touching the horizon, which the reference puts one degree below it.
pub const SUNRISE_ANGLE: f64 = -1.0_f64.to_radians();

/// The sun's position, azimuth and altitude in degrees, from turns since the epoch.
#[must_use]
pub fn sun_azimuth_and_altitude(turns: f64) -> (f64, f64) {
    sun_azimuth_and_altitude_in_year(turns, YEAR_LENGTH_DAYS)
}

/// The sun's position in a year of a given length, in days, as degrees.
///
/// The length matters more than it sounds: the reference derives its angles per day from the year, so a four-day year and a 364-day year are different curves, not scaled
/// copies of one another. It is truncated to whole days exactly as the reference truncates it.
#[must_use]
pub fn sun_azimuth_and_altitude_in_year(turns: f64, year_length_days: f64) -> (f64, f64) {
    let days_since_epoch = days_since_epoch(turns, LONGITUDE);
    #[allow(clippy::cast_possible_truncation)]
    let year = year_length_days as i32;
    let (azimuth, altitude) = azimuth_altitude(days_since_epoch, year, LATITUDE, LONGITUDE);
    (azimuth.to_degrees(), altitude.to_degrees())
}

/// The days since the epoch of a moment, with the longitude's timezone taken off.
///
/// Reference: `sun_ra_declination` (`src/calendar.cpp:139`), where `timezone` is `angle_to_time( longitude )` - fifteen degrees to the hour, so a degree of longitude is
/// four minutes.
#[must_use]
pub fn days_since_epoch(turns: f64, longitude: f64) -> f64 {
    (turns - timezone_turns(longitude)) / TURNS_PER_DAY
}

/// The timezone a longitude stands for, in turns.
///
/// Reference: `angle_to_time` (`src/calendar.cpp:108-111`), `a / 15_degrees * 1_hours`. The result is a `time_duration`, which counts **whole turns**, so
/// seventy-one degrees west is 17054 turns and not 17054.4 - and a third of a second is four hundredths of an arc minute of right ascension, which is the size of the
/// difference this had to be measured to see.
#[must_use]
pub fn timezone_turns(longitude: f64) -> f64 {
    (longitude.to_degrees() / 15.0 * 3600.0).trunc()
}

/// The sun's altitude alone, in degrees.
#[must_use]
pub fn sun_altitude_degrees(turns: f64) -> f64 {
    sun_azimuth_and_altitude(turns).1
}

/// The sun's altitude at which the game calls it day, in degrees.
///
/// Not zero: the reference lets the day begin while the sun's centre is still below the horizon, which is what refraction and the width of the disc do in the sky it is
/// modelling, and a port that used the horizon itself would move every dawn and dusk by minutes.
#[must_use]
pub fn sunrise_angle() -> f64 {
    SUNRISE_ANGLE.to_degrees()
}

/// Whether the sun is up (`calendar.cpp:368-371`).
///
/// Reference: `is_day( p )` is `sun_altitude( p ) >= sunrise_angle`, an inclusive comparison - so the instant the sun touches the angle is already day.
#[must_use]
pub fn is_day(turns: f64) -> bool {
    sun_altitude_degrees(turns) >= sunrise_angle()
}

/// Whether it is night (`calendar.cpp:363-366`).
///
/// Reference: `is_night` is `sun_altitude <= civil_dawn`, which is **minus six degrees** - so the two predicates are not each other's negation, and the game has a twilight
/// between them that neither calls.
#[must_use]
pub fn is_night(turns: f64) -> bool {
    sun_altitude_degrees(turns) <= CIVIL_DAWN_DEGREES
}

/// The altitude the game calls night, in degrees (`calendar.cpp:41`).
pub const CIVIL_DAWN_DEGREES: f64 = -6.0;

/// The sunrise angle in degrees, kept under its old name for the callers that only want the number.
pub const SUNRISE_ANGLE_DEGREES: f64 = -1.0;
