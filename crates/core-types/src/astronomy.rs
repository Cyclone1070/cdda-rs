//! Where the sun is, given the time.
//!
//! Reference: `sun_azimuth_altitude` and its helpers (`src/calendar.cpp:123-239`). The
//! chain is astronomical - mean longitude, mean anomaly, ecliptic longitude, equatorial
//! rotation, sidereal time, then the horizontal transform - and every constant in it is the
//! reference's, not an astronomer's.
//!
//! Nothing here is trusted on its own: the port is gated against 424 samples the reference
//! printed (`corpus/astronomy/sun.json`), because a curve that is plausible and a few
//! minutes off is the failure mode that matters at dawn.

/// The reference's default location, `src/calendar.cpp:31`.
pub const LATITUDE_DEGREES: f64 = 42.36;
/// The reference's default location, `src/calendar.cpp:31`.
pub const LONGITUDE_DEGREES: f64 = -71.06;

/// A year is four seasons of 91 days by default, `calendar::year_length()`.
///
/// The season length is an option and the year length follows from it, so this is a default
/// rather than a constant of nature: [`sun_azimuth_and_altitude_in_year`] takes the calendar
/// it is asked about.
pub const YEAR_LENGTH_DAYS: f64 = 4.0 * 91.0;
/// Turns (seconds) in one game day.
pub const TURNS_PER_DAY: f64 = 24.0 * 3600.0;

/// The obliquity of the ecliptic, held fixed at its J2000 value.
pub const OBLIQUITY_DEGREES: f64 = 23.439_279;

/// The sun's position, azimuth and altitude in degrees.
///
/// `turns` counts from the calendar's epoch, which is midnight on the vernal equinox at
/// Greenwich - the reference's choice, and the reason mean longitude starts at zero.
#[must_use]
pub fn sun_azimuth_and_altitude(turns: f64) -> (f64, f64) {
    sun_azimuth_and_altitude_in_year(turns, YEAR_LENGTH_DAYS)
}

/// The sun's position in a year of a given length, in days.
///
/// The length matters more than it sounds: the reference derives its angles per day from the
/// year, so a four-day year and a 364-day year are different curves, not scaled copies of
/// one another.
#[must_use]
pub fn sun_azimuth_and_altitude_in_year(turns: f64, year_length_days: f64) -> (f64, f64) {
    // `angle_to_time( location.longitude )`: a degree of longitude is four minutes.
    let timezone_turns = LONGITUDE_DEGREES / 15.0 * 3600.0;
    let days_since_epoch = (turns - timezone_turns) / TURNS_PER_DAY;

    let angle_per_day = 360.0 / year_length_days;
    let mean_longitude = angle_per_day * days_since_epoch;
    let mean_anomaly = 77.0 + mean_longitude;
    let ecliptic_longitude = mean_longitude
        + 1.915 * mean_anomaly.to_radians().sin()
        + 0.020 * (2.0 * mean_anomaly).to_radians().sin();

    // Ecliptic to equatorial.
    let ecliptic = ecliptic_longitude.to_radians();
    let obliquity = OBLIQUITY_DEGREES.to_radians();
    let right_ascension = (ecliptic.sin() * obliquity.cos()).atan2(ecliptic.cos());
    let declination = (ecliptic.sin() * obliquity.sin()).asin();

    // Sidereal time: 180 degrees at the epoch, advancing a turn per day and a turn per year.
    let sidereal = 180.0 + (360.0 + angle_per_day) * days_since_epoch + LONGITUDE_DEGREES;
    let hour_angle = (sidereal - right_ascension.to_degrees()).to_radians();

    // Equatorial to horizontal.
    let intermediate = (
        hour_angle.cos() * declination.cos(),
        hour_angle.sin() * declination.cos(),
        declination.sin(),
    );
    let latitude = LATITUDE_DEGREES.to_radians();
    let horizontal_z = intermediate.0 * latitude.cos() + intermediate.2 * latitude.sin();

    // Azimuth is measured from the south, positive to the west.
    let azimuth = (-intermediate
        .1
        .atan2(-intermediate.0 * latitude.sin() + intermediate.2 * latitude.cos())
        + std::f64::consts::PI)
        .to_degrees();
    let altitude = horizontal_z.asin().to_degrees();
    (azimuth, altitude)
}

/// The sun's altitude alone, in degrees.
#[must_use]
pub fn sun_altitude_degrees(turns: f64) -> f64 {
    sun_azimuth_and_altitude(turns).1
}
