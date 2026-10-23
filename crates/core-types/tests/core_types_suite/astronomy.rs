//! The sun's position, against four hundred samples the reference printed.
//!
//! `corpus/astronomy/sun.json` is `--dump-astronomy`: the reference's own
//! `sun_azimuth_altitude`, a week apart across a year, three hours apart. The port has to
//! reproduce it; anything else is a curve I made up that happens to look like a day.

// Days and hours in a dump are small and exactly representable; the cast is how they get into
// the formula.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use cdda_core_types::astronomy::{TURNS_PER_DAY, sun_azimuth_and_altitude_in_year};
use std::path::PathBuf;

struct Sample {
    day: i64,
    hour: i64,
    altitude: f64,
}

/// The dump, and the year length it was taken under.
struct Dump {
    year_length_days: f64,
    samples: Vec<Sample>,
}

fn samples() -> Dump {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/astronomy/sun.json");
    let text = std::fs::read_to_string(path).expect("the astronomy dump exists");
    let mut year_length_days = None;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("\"year_length_days\":") {
            // The line also carries the season length; take the value up to the comma.
            year_length_days = rest
                .split(',')
                .next()
                .and_then(|value| value.trim().parse::<f64>().ok());
            continue;
        }
        let line = line.trim().trim_end_matches(',');
        if !line.starts_with('{') {
            continue;
        }
        let mut day = None;
        let mut hour = None;
        let mut altitude = None;
        for field in line.trim_matches(|c| c == '{' || c == '}').split(',') {
            let (key, value) = field.split_once(':').expect("a field");
            let key = key.trim().trim_matches('"');
            match key {
                "day" => day = value.trim().parse::<i64>().ok(),
                "hour" => hour = value.trim().parse::<i64>().ok(),
                "altitude" => altitude = value.trim().parse::<f64>().ok(),
                _ => {}
            }
        }
        if let (Some(day), Some(hour), Some(altitude)) = (day, hour, altitude) {
            out.push(Sample {
                day,
                hour,
                altitude,
            });
        }
    }
    Dump {
        year_length_days: year_length_days.expect("the dump says which calendar it used"),
        samples: out,
    }
}

#[test]
fn the_sun_reproduces_the_reference_over_a_whole_year() {
    let dump = samples();
    let samples = &dump.samples;
    assert_eq!(
        samples.len(),
        424,
        "the dump is a year of samples, a week apart"
    );
    let mut worst = 0.0_f64;
    let mut worst_at = (0, 0);
    for sample in samples {
        let turns = sample.day as f64 * TURNS_PER_DAY + sample.hour as f64 * 3600.0;
        let mine = sun_azimuth_and_altitude_in_year(turns, dump.year_length_days).1;
        let error = (mine - sample.altitude).abs();
        if error > worst {
            worst = error;
            worst_at = (sample.day, sample.hour);
        }
    }
    // Five thousandths of a degree - five arcseconds - is the agreement this reaches. The
    // residual is the reference's angle helpers rounding on the way through, not a modelling
    // difference: every intermediate matches to the printed digits.
    assert!(
        worst < 5e-3,
        "worst altitude error {worst} degrees at day {} hour {}",
        worst_at.0,
        worst_at.1
    );
    println!(
        "astronomy: {} samples at a {} day year, worst altitude error {:.3e} degrees",
        samples.len(),
        dump.year_length_days,
        worst
    );
}

#[test]
fn the_sun_rises_and_sets_over_a_day() {
    // A sanity check the dump cannot give: the curve has to have a day in it at all.
    let midnight = sun_azimuth_and_altitude_in_year(0.0, 4.0).1;
    let noon = sun_azimuth_and_altitude_in_year(12.0 * 3600.0, 4.0).1;
    assert!(
        midnight < -20.0,
        "midnight is below the horizon: {midnight}"
    );
    assert!(noon > 20.0, "noon is above it: {noon}");
    assert!(noon > midnight);
}

#[test]
fn the_sun_is_up_between_its_dawn_and_its_dusk() {
    // Reference: `calendar.cpp:42` and `:368` - `is_day( p )` is `sun_altitude( p ) >= sunrise_angle`, and `sunrise_angle` is **minus one degree**, not zero. The
    // negative angle is deliberate: refraction and the sun's disc mean the day starts when its centre is still below the horizon, and a port that used zero would
    // shift every dawn and dusk by a few minutes.
    let dawn = cdda_core_types::astronomy::sunrise_angle();
    assert!((dawn - (-1.0)).abs() < 1e-12, "the reference's sunrise angle is -1 degrees: {dawn}");
    // Over a spring day, the sun is up somewhere in the middle and down at midnight and midday's opposite; the exact hours are the dump's business, the ordering is this
    // test's.
    let midnight = 0.0;
    let noon = 12.0 * 3600.0;
    assert!(!cdda_core_types::astronomy::is_day(midnight), "midnight is night");
    assert!(cdda_core_types::astronomy::is_day(noon), "noon is day");
    // And the transition is a single crossing in each direction, not a flush of flapping values.
    let mut changes = 0;
    let mut was = cdda_core_types::astronomy::is_day(midnight);
    for minute in 1..(24 * 60) {
        let now = cdda_core_types::astronomy::is_day(f64::from(minute) * 60.0);
        if now != was {
            changes += 1;
        }
        was = now;
    }
    assert_eq!(changes, 2, "day begins once and ends once");
}
