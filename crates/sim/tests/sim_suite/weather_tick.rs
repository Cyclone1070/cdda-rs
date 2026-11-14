//! The weather as a tick: one call that produces the numbers, decides the sky, and books the next change.
//!
//! Every piece of this has its own gate - the generator against recorded numbers, the conditions against the data, the manager against the recorded day - and each of those gates
//! hands its piece its inputs. This one hands the pieces nothing: it drives the whole thing from the reference's recorded dice and asks whether the result is the recording.
//!
//! Reference: `weather_manager::update_weather` (`weather.cpp:901-947`) calling `weather_generator::get_weather` (`weather_gen.cpp:125-190`) and
//! `weather_generator::get_weather_conditions`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use cdda_content::weather::{Point, Rules};
use cdda_sim::weather_gen::{CommonData, Generator};
use std::collections::BTreeMap;
use std::path::Path;

struct Step {
    hour: i64,
    turn: i64,
    weather: String,
    temperature_k: f64,
    humidity: f64,
    pressure: f64,
    windpower: f64,
    winddirection: f64,
    next_weather_turn: i64,
    common: CommonData,
    day_fraction: f64,
}

fn number(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(cdda_json::Value::Float(value)) => *value,
        Some(cdda_json::Value::Int(value)) => *value as f64,
        other => panic!("{key} is a number: {other:?}"),
    }
}

fn steps() -> Vec<Step> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/series-24h.json");
    let text = std::fs::read_to_string(path).expect("the recorded series");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON")
    else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(list)) = root.get("steps") else {
        panic!("steps")
    };
    list.iter()
        .map(|step| {
            let cdda_json::Value::Object(step) = step else {
                panic!("a step")
            };
            Step {
                hour: number(step, "hour") as i64,
                turn: number(step, "turn") as i64,
                weather: match step.get("weather") {
                    Some(cdda_json::Value::Str(id)) => id.clone(),
                    other => panic!("a weather id: {other:?}"),
                },
                temperature_k: number(step, "temperature_k"),
                humidity: number(step, "humidity"),
                pressure: number(step, "pressure"),
                windpower: number(step, "windpower"),
                winddirection: number(step, "winddirection"),
                next_weather_turn: number(step, "next_weather_turn") as i64,
                common: CommonData {
                    x: number(step, "common_x"),
                    y: number(step, "common_y"),
                    z: number(step, "common_z"),
                    cyf: cdda_sim::weather_gen::year_cosine(number(step, "year_fraction")),
                    mod_seed: number(step, "mod_seed") as u32,
                    season: number(step, "season") as usize,
                },
                day_fraction: number(step, "day_fraction"),
            }
        })
        .collect()
}

/// The dice the reference rolled, hour by hour.
fn dice() -> BTreeMap<i64, Vec<(i32, i32, i32)>> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/series-24h.rolls.txt");
    let text = std::fs::read_to_string(path).expect("the recorded dice");
    let mut by_hour: BTreeMap<i64, Vec<(i32, i32, i32)>> = BTreeMap::new();
    let mut hour: Option<i64> = None;
    for line in text.lines() {
        if let Some(number) = line.strip_prefix("mark weather.hour.") {
            let parsed: i64 = number.parse().expect("an hour");
            hour = Some(parsed);
            by_hour.entry(parsed).or_default();
        } else if let Some(rest) = line.strip_prefix("int ") {
            let mut parts = rest.split(' ');
            let (Some(low), Some(high), Some(drawn)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if let (Ok(low), Ok(high), Ok(drawn), Some(hour)) = (
                low.parse::<i32>(),
                high.parse::<i32>(),
                drawn.parse::<i32>(),
                hour,
            ) {
                by_hour.entry(hour).or_default().push((low, high, drawn));
            }
        }
    }
    by_hour
}

/// The weather types and conditions out of the reference's own data file.
pub(crate) fn rules() -> Rules {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/weather_type.json");
    let text = std::fs::read_to_string(path).expect("the weather data");
    let cdda_json::Value::Array(entries) = cdda_json::parse(text.as_bytes()).expect("valid JSON")
    else {
        panic!("array")
    };
    let objects: Vec<cdda_json::Object> = entries
        .iter()
        .filter_map(|entry| match entry {
            cdda_json::Value::Object(object) => Some(object.clone()),
            _ => None,
        })
        .collect();
    let functions = cdda_content::jmath::read_entries(&objects).expect("jmath");
    let mut types = Vec::new();
    let mut conditions = BTreeMap::new();
    for object in &objects {
        let is_weather = matches!(object.get("type"), Some(cdda_json::Value::Str(text)) if text == "weather_type");
        if !is_weather {
            continue;
        }
        let Some(cdda_json::Value::Str(id)) = object.get("id") else {
            continue;
        };
        let priority = match object.get("priority") {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
            _ => 0,
        };
        types.push(cdda_content::weather::WeatherType {
            id: id.clone(),
            priority,
            ..Default::default()
        });
        if let Some(condition) = object.get("condition") {
            conditions.insert(
                id.clone(),
                cdda_content::condition::Condition::from_value(condition).expect("a condition"),
            );
        }
    }
    Rules::new(types, conditions, functions)
}

#[test]
fn a_tick_of_the_whole_weather_reproduces_the_recorded_day() {
    // The capstone: no piece is handed a number the reference produced. The point is computed from the generator, the sky is decided from the data's own conditions, the schedule is
    // booked by the manager, and the only inputs from the recording are the dice and the sector draws - which are exactly the draws a caller owns.
    let steps = steps();
    let dice = dice();
    let mut system = cdda_sim::weather::System::new(rules());
    let generator = Generator::default_region();
    // **Hour zero is the recording's starting state, not a tick.** The reference had already chosen its weather before the recording began - that is why the trace shows the first
    // hour rolling no dice at all - so the port is seeded from it and the ticks start at the second hour.
    let first = &steps[0];
    system.manager.weather = Some(first.weather.clone());
    system.manager.point = Some(Point {
        turn: first.turn,
        temperature: first.temperature_k,
        humidity: first.humidity,
        pressure: first.pressure,
        windpower: first.windpower as i32,
        winddirection: first.winddirection as i32,
    });
    system.manager.next_turn = first.next_weather_turn;
    system.wind.degrees = Some(first.winddirection as i32);
    // The wind changes direction once in the recorded day: the thirteenth hour, whose air is dead calm, so no sector is drawn before it.
    let mut sectors = vec![14usize].into_iter();
    let mut wrong: Vec<(i64, String, String, String)> = Vec::new();
    let mut checked = 0;
    for step in &steps[1..] {
        let recorded = dice.get(&step.hour).expect("every hour rolls");
        let mut supplied = recorded.iter();
        let mut roll = |low: i32, high: i32| {
            let (want_low, want_high, drawn) = *supplied
                .next()
                .expect("the tick asks for no more dice than the reference did");
            assert_eq!(
                (low, high),
                (want_low, want_high),
                "hour {} asks for the dice the reference asked for",
                step.hour
            );
            drawn
        };
        let mut sector = |_season: usize| {
            sectors
                .next()
                .expect("a sector was drawn when the direction changed")
        };
        let change = system.tick(
            &cdda_sim::weather::Forecast {
                generator: &generator,
                common: step.common,
                day_fraction: step.day_fraction,
                turn: step.turn,
            },
            &mut roll,
            &mut sector,
        );
        assert!(
            matches!(change, cdda_content::weather::Change::Weather { .. }),
            "hour {} was due",
            step.hour
        );
        let point: Point = system.manager.point.expect("a point");
        checked += 1;
        let mut note = |what: &str, ours: &str, theirs: &str| {
            if ours != theirs && wrong.len() < 8 {
                wrong.push((
                    step.hour,
                    what.to_string(),
                    ours.to_string(),
                    theirs.to_string(),
                ));
            }
        };
        note(
            "weather",
            system.manager.weather.as_deref().unwrap_or("none"),
            step.weather.as_str(),
        );
        let mut near = |what: &str, ours: f64, theirs: f64, tolerance: f64| {
            if (ours - theirs).abs() > tolerance && wrong.len() < 8 {
                wrong.push((
                    step.hour,
                    what.to_string(),
                    format!("{ours}"),
                    format!("{theirs}"),
                ));
            }
        };
        near("temperature", point.temperature, step.temperature_k, 1e-6);
        near("humidity", point.humidity, step.humidity, 1e-6);
        near("pressure", point.pressure, step.pressure, 1e-6);
        near("windpower", f64::from(point.windpower), step.windpower, 0.0);
        near(
            "winddirection",
            f64::from(point.winddirection),
            step.winddirection,
            0.0,
        );
        near(
            "next turn",
            system.manager.next_turn as f64,
            step.next_weather_turn as f64,
            0.0,
        );
    }
    assert!(
        checked >= 20,
        "every hour after the seeded one ticked: {checked}"
    );
    assert!(
        wrong.is_empty(),
        "the tick disagrees with the recording: {wrong:?}"
    );
}
