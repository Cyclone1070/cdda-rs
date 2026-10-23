//! The weather generator against the reference's own recorded hours.
//!
//! Source: `--dump-weather-series` on seed `weatherwatch`, which writes, for each hour, the generator's intermediate values (location, day and year
//! fraction, season, the modded seed) and the temperature it produced. The port feeds the same intermediates to `weather_gen::temperature_k` and has
//! to land on the same kelvin.
//!
//! Gating the formula before the calendar is honest rather than lazy: the calendar that produces a day fraction and a season is a separate piece of
//! machinery with its own rules, and a single all-or-nothing test would say only "something between the turn counter and the thermometer is wrong".

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_json::{Value, parse};
use std::path::Path;

fn corpus() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/series-24h.json")
}

/// One recorded hour: the generator's inputs and the temperature it produced.
struct Hour {
    hour: i64,
    temperature_k: f64,
    x: f64,
    y: f64,
    z: f64,
    year_fraction: f64,
    day_fraction: f64,
    season: usize,
    mod_seed: u32,
}

fn number(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(Value::Float(value)) => *value,
        Some(Value::Int(value)) => *value as f64,
        other => panic!("{key} is a number, not {other:?}"),
    }
}

fn hours() -> Vec<Hour> {
    let text = std::fs::read_to_string(corpus()).expect("the recorded weather");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else { panic!("root") };
    let Some(Value::Array(steps)) = root.get("steps") else { panic!("no steps") };
    steps
        .iter()
        .map(|step| {
            let Value::Object(step) = step else { panic!("a step is an object") };
            Hour {
                hour: number(step, "hour") as i64,
                temperature_k: number(step, "temperature_k"),
                x: number(step, "common_x"),
                y: number(step, "common_y"),
                z: number(step, "common_z"),
                year_fraction: number(step, "year_fraction"),
                day_fraction: number(step, "day_fraction"),
                season: number(step, "season") as usize,
                mod_seed: number(step, "mod_seed") as u32,
            }
        })
        .collect()
}

#[test]
fn the_generator_reproduces_the_recorded_temperatures() {
    let hours = hours();
    assert_eq!(hours.len(), 24, "a day of recorded weather");
    // The default region's settings, from `data/json/regional_map_settings.json`: an average of 6.5 C and no seasonal manual adjustment.
    let generator = cdda_sim::weather_gen::Generator {
        base_temperature: 6.5,
        seasonal_temp_mod: [0.0; 4],
    };
    let mut worst = 0.0f64;
    let mut wrong = Vec::new();
    for hour in &hours {
        let common = cdda_sim::weather_gen::CommonData {
            x: hour.x,
            y: hour.y,
            z: hour.z,
            cyf: cdda_sim::weather_gen::year_cosine(hour.year_fraction),
            mod_seed: hour.mod_seed,
            season: hour.season,
        };
        let ours = cdda_sim::weather_gen::temperature_k(&generator, &common, hour.day_fraction);
        let difference = (ours - hour.temperature_k).abs();
        worst = worst.max(difference);
        if difference > 1e-6 && wrong.len() < 6 {
            wrong.push((hour.hour, ours, hour.temperature_k));
        }
    }
    assert!(
        wrong.is_empty(),
        "every recorded temperature, within a millionth of a kelvin; worst {worst:.9} K, wrong {wrong:?}"
    );
}

#[test]
fn the_year_cosine_peaks_at_midwinter_and_troughs_at_midsummer() {
    // Reference: `weather_gen.cpp:75-79` - one eighth is added to the year fraction so that the cosine is `+1` at midwinter even though a Cataclysm year
    // starts in spring. That puts the peak at seven-eighths of a year and the trough at three-eighths, which is midsummer, not the start of the summer
    // quarter: the eighth shifts the whole curve, it does not renumber the quarters. Getting that wrong makes every winter warm.
    let spring_start = cdda_sim::weather_gen::year_cosine(0.0);
    let midsummer = cdda_sim::weather_gen::year_cosine(0.375);
    let midwinter = cdda_sim::weather_gen::year_cosine(0.875);
    assert!(midsummer < -0.99, "midsummer is the trough: {midsummer}");
    assert!(midwinter > 0.99, "midwinter is the peak: {midwinter}");
    assert!(
        (spring_start - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12,
        "the year opens at the eighth: {spring_start}"
    );
}
