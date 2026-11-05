//! The reference's pre-world state: the clock it started on, and the sun it computed for that day.
//!
//! Source: `corpus/env/env-seed12345.json` is `--dump-env` from the instrumented reference
//! (`harness.cpp`): the world's clock, sunrise and sunset for that turn, the options the run used,
//! and the engine's next four draws.
//!
//! Version two of the dump is the one this gate wants, and the reason is worth keeping: version one
//! was taken **before the reference's calendar was configured**, so its sunrise came off an
//! unconfigured four-day year and disagreed with every later run by 3,422 turns. The port was right;
//! the dump was measuring an object before it was built.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::calendar::{
    CalendarConfig, TimePoint, day_of_season, hour_of_day, season_of_year, sunrise, sunset,
};
use cdda_json::{Value, parse};
use std::path::Path;

/// What the dump recorded.
struct Recorded {
    version: i64,
    turn: i32,
    season: i32,
    day_of_season: i32,
    hour: i32,
    sunrise: i32,
    sunset: i32,
    season_length: i32,
}

fn recorded() -> Recorded {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/env/env-seed12345.json");
    let text = std::fs::read(path).expect("the environment dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let number = |key: &str| match root.get(key) {
        Some(Value::Int(found)) => i32::try_from(*found).expect("a small number"),
        Some(Value::Float(found)) => {
            #[allow(clippy::cast_possible_truncation)]
            let whole = *found as i64;
            i32::try_from(whole).expect("a small number")
        }
        _ => panic!("the dump has no {key}"),
    };
    let version = match root.get("version") {
        Some(Value::Int(found)) => *found,
        _ => panic!("the dump has no version"),
    };
    let season_length = match root.get("options") {
        Some(Value::Object(options)) => match options.get("SEASON_LENGTH") {
            Some(Value::Int(found)) => i32::try_from(*found).expect("a season length"),
            _ => panic!("the dump has no SEASON_LENGTH"),
        },
        _ => panic!("the dump has no options"),
    };
    Recorded {
        version,
        turn: number("turn"),
        season: number("season"),
        day_of_season: number("day_of_season"),
        hour: number("hour"),
        sunrise: number("sunrise"),
        sunset: number("sunset"),
        season_length,
    }
}

#[test]
fn the_sun_rises_and_sets_when_the_reference_says_it_does() {
    let dump = recorded();
    assert_eq!(
        dump.version, 2,
        "version one was taken on an unconfigured calendar, so its sun numbers are not comparable"
    );
    let config = CalendarConfig {
        season_length_days: dump.season_length,
        ..CalendarConfig::default()
    };
    let at = TimePoint::from_turn(dump.turn);

    assert_eq!(sunrise(at, &config).to_turn(), dump.sunrise, "sunrise");
    assert_eq!(sunset(at, &config).to_turn(), dump.sunset, "sunset");
}

#[test]
fn the_clock_reads_the_season_day_and_hour_the_reference_reads() {
    let dump = recorded();
    let config = CalendarConfig {
        season_length_days: dump.season_length,
        ..CalendarConfig::default()
    };
    let at = TimePoint::from_turn(dump.turn);

    assert_eq!(season_of_year(at, &config) as i32, dump.season, "season");
    assert_eq!(day_of_season(at, &config), dump.day_of_season, "day of season");
    assert_eq!(hour_of_day(at), dump.hour, "hour of day");
}
