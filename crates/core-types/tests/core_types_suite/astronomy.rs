//! The sun, as the reference computes it.
//!
//! `corpus/astronomy/sun.json` is 424 samples of the reference's `sun_azimuth_altitude` and `sun_light_at` (`src/calendar.cpp:237-402`), and beside them the five values
//! that chain works from — right ascension, declination, sidereal time, hour angle and days since the epoch. They are dumped because the chain crosses all five and a port
//! with one wrong constant produces a plausible curve that is minutes off at dawn, which is exactly where the light-level boundary sits.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use cdda_core_types::astronomy::{
    LATITUDE, LONGITUDE, azimuth_altitude, light_at, ra_declination, sidereal_time, timezone_turns,
};
use cdda_json::Value;

/// One sample of the reference's own numbers.
struct Sample {
    day: i64,
    hour: i64,
    altitude: f64,
    light: f64,
    ra: f64,
    dec: f64,
    sidereal: f64,
    hour_angle: f64,
    days: f64,
}

fn samples() -> (i32, Vec<Sample>) {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/astronomy/sun.json");
    let text = std::fs::read_to_string(&path).expect("the astronomy dump");
    let Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let number = |object: &cdda_json::Object, key: &str| -> f64 {
        match object.get(key) {
            Some(Value::Float(value)) => *value,
            Some(Value::Int(value)) => *value as f64,
            other => panic!("{key} is a number: {other:?}"),
        }
    };
    let Some(Value::Int(year)) = root.get("year_length_days") else {
        panic!("a year length")
    };
    let Some(Value::Array(list)) = root.get("samples") else {
        panic!("samples")
    };
    let mut out = Vec::new();
    for entry in list {
        let Value::Object(entry) = entry else {
            panic!("a sample")
        };
        out.push(Sample {
            day: match entry.get("day") {
                Some(Value::Int(value)) => *value,
                other => panic!("a day: {other:?}"),
            },
            hour: match entry.get("hour") {
                Some(Value::Int(value)) => *value,
                other => panic!("an hour: {other:?}"),
            },
            altitude: number(entry, "altitude"),
            light: number(entry, "light"),
            ra: number(entry, "ra"),
            dec: number(entry, "dec"),
            sidereal: number(entry, "sidereal"),
            hour_angle: number(entry, "hour_angle"),
            days: number(entry, "days"),
        });
    }
    (i32::try_from(*year).expect("a year length"), out)
}

/// Every stage of the chain against the reference's own intermediates.
#[test]
fn the_sun_s_whole_chain_matches_the_reference() {
    let (year_length, samples) = samples();
    assert_eq!(samples.len(), 424, "the dump holds 424 samples");
    let mut worst = [0.0_f64; 5];
    let mut worst_light = 0.0_f64;
    for sample in &samples {
        let seconds = f64::from(i32::try_from(sample.day).expect("a day")) * 86_400.0
            + f64::from(i32::try_from(sample.hour).expect("an hour")) * 3_600.0;
        // The reference subtracts the timezone, which is the longitude turned into hours - fifteen degrees to the hour - before it counts days
        // (`src/calendar.cpp:139`, `angle_to_time`).
        let days = (seconds - timezone_turns(LONGITUDE)) / 86_400.0;
        worst[0] = worst[0].max((days - sample.days).abs() * 86_400.0);
        let (ra, dec) = ra_declination(days, year_length);
        let (ra, dec) = (ra.to_degrees(), dec.to_degrees());
        worst[1] = worst[1].max((ra - sample.ra).abs());
        worst[1] = worst[1].max((dec - sample.dec).abs());
        let sidereal = sidereal_time(days, year_length, LONGITUDE).to_degrees();
        worst[2] = worst[2].max((sidereal - sample.sidereal).abs());
        let hour_angle = (sidereal - ra) - sample.hour_angle;
        worst[3] = worst[3].max(hour_angle.abs());
        let (_, altitude_radians) = azimuth_altitude(days, year_length, LATITUDE, LONGITUDE);
        let altitude = altitude_radians.to_degrees();
        worst[4] = worst[4].max((altitude - sample.altitude).abs());
        // The light takes the altitude in **radians**, as the reference's `sun_light_at` does.
        worst_light = worst_light.max((f64::from(light_at(altitude_radians)) - sample.light).abs());
    }
    assert!(
        worst[0] < 1.0,
        "the day count is within a second: {} off",
        worst[0]
    );
    // **A hundredth of an arc second is the floor**, because the obliquity the reference quotes is `23.439279` degrees - six decimals - and half a millionth of a
    // degree in that constant moves the declination by about a millionth. The rest of the chain is exact against the dump's twelve printed places.
    assert!(
        worst[1] < 1e-5,
        "right ascension and declination: {} degrees off",
        worst[1]
    );
    assert!(worst[2] < 1e-5, "sidereal time: {} degrees off", worst[2]);
    assert!(worst[3] < 1e-5, "hour angle: {} degrees off", worst[3]);
    assert!(worst[4] < 1e-5, "altitude: {} degrees off", worst[4]);
    assert!(
        worst_light < 1e-3,
        "sunlight: {worst_light} off, which is a light level"
    );
}
