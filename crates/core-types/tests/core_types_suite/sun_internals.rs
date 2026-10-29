//! The six numbers the sun's position is computed from, compared one at a time.
//!
//! Source: `corpus/env/sun-internals.json` is `--dump-sun-internals`, written by the instrumented
//! reference through its `harness_sun_internals` hook (`calendar.cpp:192`): right ascension,
//! declination, sidereal time, hour angle, days since the epoch and the latitude, sampled every
//! hundred turns across the first four hours of the first day, plus the reference's own sunrise and
//! sunset for that day.
//!
//! It exists because the environment dump's sunrise disagreed by 3,422 turns and every function in
//! the chain read identically. One answer cannot say which input is wrong; six can.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::calendar::{CalendarConfig, TimePoint, sun_internals};
use cdda_json::{Value, parse};
use std::path::Path;

/// One sample: the turn, and the six numbers the reference worked out there.
struct Sample {
    turn: i32,
    ra: f64,
    dec: f64,
    sidereal: f64,
    hour_angle: f64,
    days: f64,
    latitude: f64,
}

/// The options the dump was taken with: the year length enters `angle_per_day`, so a sample taken
/// with one-day seasons and one taken with ninety-one-day seasons are not comparable at all.
fn options() -> (i32, bool) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/env/sun-internals.json");
    let text = std::fs::read(path).expect("the sun-internals dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Object(options)) = root.get("options") else {
        panic!("the dump records no options, so its numbers mean nothing without a guess");
    };
    let season_length = match options.get("SEASON_LENGTH") {
        Some(Value::Int(found)) => i32::try_from(*found).expect("a season length"),
        _ => panic!("the dump has no SEASON_LENGTH"),
    };
    let eternal = matches!(options.get("ETERNAL_SEASON"), Some(Value::Bool(true)));
    (season_length, eternal)
}

fn samples() -> Vec<Sample> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/env/sun-internals.json");
    let text = std::fs::read(path).expect("the sun-internals dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(entries)) = root.get("samples") else {
        panic!("the dump has no samples");
    };
    let number = |entry: &cdda_json::Value, key: &str| match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Float(found)) => *found,
            Some(Value::Int(found)) => f64::from(i32::try_from(*found).expect("a small number")),
            _ => panic!("a sample has no {key}"),
        },
        _ => panic!("a sample is not an object"),
    };
    entries
        .iter()
        .map(|entry| {
            let Value::Object(object) = entry else {
                panic!("a sample is not an object");
            };
            let turn = match object.get("turn") {
                Some(Value::Int(found)) => i32::try_from(*found).expect("a turn fits"),
                _ => panic!("a sample has no turn"),
            };
            Sample {
                turn,
                ra: number(entry, "ra"),
                dec: number(entry, "dec"),
                sidereal: number(entry, "sidereal"),
                hour_angle: number(entry, "hour_angle"),
                days: number(entry, "days"),
                latitude: number(entry, "latitude"),
            }
        })
        .collect()
}

#[test]
fn every_number_the_sun_is_computed_from_matches_the_reference() {
    let (season_length, eternal) = options();
    let config = CalendarConfig {
        season_length_days: season_length,
        eternal_season: eternal,
        ..CalendarConfig::default()
    };
    let samples = samples();
    assert!(
        samples.len() > 100,
        "the dump holds {} samples, which is too few to say which step diverges",
        samples.len()
    );
    let mut worst = [(0.0_f64, "ra"), (0.0, "dec"), (0.0, "sidereal"), (0.0, "hour_angle"), (0.0, "days")];
    let mut disagreements = Vec::new();
    for sample in &samples {
        let ours = sun_internals(TimePoint::from_turn(sample.turn), &config);
        for (ours, theirs, name) in [
            (ours.right_ascension, sample.ra, "ra"),
            (ours.declination, sample.dec, "dec"),
            (ours.sidereal_time, sample.sidereal, "sidereal"),
            (ours.hour_angle, sample.hour_angle, "hour_angle"),
            (ours.days_since_epoch, sample.days, "days"),
            (ours.latitude, sample.latitude, "latitude"),
        ] {
            let difference = (ours - theirs).abs();
            if let Some(slot) = worst.iter_mut().find(|(_, slot)| *slot == name)
                && difference > slot.0
            {
                slot.0 = difference;
            }
            // A ten-thousandth of a degree is 0.36 arc seconds: tight enough that a wrong rule cannot
            // hide, loose enough for the port's single-precision vector rotations. The measured
            // worst gap is 2.4e-6, and it is in sidereal time alone.
            if difference > 1e-4 {
                disagreements.push(format!(
                    "turn {}: {name} reference {theirs}, port {ours}",
                    sample.turn
                ));
            }
        }
    }
    assert!(
        disagreements.is_empty(),
        "{} of {} sample(s) disagreed; worst gaps {:?}\nfirst few:\n{}",
        disagreements.len(),
        samples.len(),
        worst,
        disagreements.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}
