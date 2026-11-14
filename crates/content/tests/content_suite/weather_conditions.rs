//! The loop that turns a set of satisfied weather conditions into a weather id.
//!
//! Reference: `weather_gen.cpp:196-215`. Start at `clear`, walk the sorted types, and replace the answer whenever a type's own requirements are met and its
//! condition holds. Two things decide the result and neither is the condition itself: the **last** match wins, and a type that names `required_weathers` only
//! applies if the current answer is one of them - which is what makes rain follow drizzle rather than appear from a clear sky.
//!
//! The conditions are dialogue expressions and are not ported yet, so the loop takes the evaluation as a parameter. That is not a shortcut around the gate:
//! the loop's own rules are what is being pinned here, and passing the evaluation in is how a caller with no dialogue machine can still be correct about them.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use cdda_content::weather::{WeatherType, conditions, sorted};

fn kind(id: &str, priority: i32, required: &[&str]) -> WeatherType {
    WeatherType {
        id: id.to_string(),
        priority,
        required_weathers: required.iter().map(|id| (*id).to_string()).collect(),
        ..Default::default()
    }
}

/// The shape the real data has: drizzle needs the cloud, rain needs the drizzle, and a thunderstorm needs the rain.
fn chain() -> Vec<WeatherType> {
    vec![
        kind("clear", 0, &[]),
        kind("sunny", 10, &[]),
        kind("cloudy", 20, &[]),
        kind("light_drizzle", 30, &["cloudy"]),
        kind("drizzle", 40, &["light_drizzle"]),
        kind("rain", 50, &["light_drizzle", "drizzle"]),
        kind("thunder", 70, &["rain"]),
    ]
}

#[test]
fn a_weather_that_names_requirements_only_follows_one_of_them() {
    // Every condition holds, so the answer is the last type in the sorted order whose requirements are met. `thunder` needs rain, and rain can only appear
    // after drizzle, which needs the cloud - so the whole chain has to walk in order for the answer to be thunder. If the loop ignored `required_weathers`
    // it would answer `thunder` immediately and this test would pass; if it required *all* of rain's two entries at once it would stop at drizzle, and this
    // test would fail.
    let types = chain();
    let order = sorted(&types);
    let answer = conditions(&order, &mut |_| true);
    assert_eq!(answer, "thunder", "each type follows the one before it");
}

#[test]
fn a_broken_chain_stops_where_it_breaks() {
    // Take the cloud away and the drizzle never starts, so nothing above it can apply - even though every *other* condition holds, and even though `rain` and
    // `thunder` are checked first in the sense that they are later in the order. The answer falls back to the last type that needs nothing: `sunny`.
    let types = chain();
    let order = sorted(&types);
    let answer = conditions(&order, &mut |candidate| candidate.id != "cloudy");
    assert_eq!(
        answer, "sunny",
        "the chain above the break is never reached"
    );
    for chained in ["light_drizzle", "drizzle", "rain", "thunder"] {
        assert_ne!(
            answer, chained,
            "{chained} cannot apply without the cloud under it"
        );
    }
}

#[test]
fn the_last_type_whose_condition_holds_wins() {
    // `sunny` and `cloudy` both carry no requirements, so both are candidates; the sorted order puts cloudy later and the loop keeps the later one. This is
    // the rule that makes the sort part of the weather's behaviour rather than a detail of storage.
    let types = vec![
        kind("clear", 0, &[]),
        kind("sunny", 10, &[]),
        kind("cloudy", 20, &[]),
    ];
    let order = sorted(&types);
    let answer = conditions(&order, &mut |candidate| candidate.id != "clear");
    assert_eq!(answer, "cloudy", "the later of two candidates");
}

#[test]
fn the_answer_starts_at_clear_and_only_moves_forward() {
    // Reference: `weather_type_id current_conditions = WEATHER_CLEAR;`. With nothing satisfied the answer is clear, not null and not the first entry.
    let types = chain();
    let order = sorted(&types);
    assert_eq!(
        conditions(&order, &mut |_| false),
        "clear",
        "nothing holds, so the sky is clear"
    );
}

#[test]
fn the_weather_is_recomputed_on_the_turn_it_is_due_and_not_before() {
    // Reference: `weather.cpp:904` - `if( weather_id == WEATHER_NULL || calendar::turn >= nextweather )`. The boundary is inclusive, and the first update
    // happens because there is no weather yet rather than because a turn has passed.
    let mut schedule = cdda_content::weather::Schedule::default();
    assert!(
        schedule.needs_update(0),
        "no weather yet, so the first turn recomputes"
    );
    schedule.reschedule("sunny".to_string(), 100, 300);
    assert_eq!(
        schedule.next_turn, 400,
        "the duration is added to the turn it was drawn at"
    );
    assert!(!schedule.needs_update(399), "one turn early is not due");
    assert!(schedule.needs_update(400), "the due turn itself is due");
}

#[test]
fn every_recorded_hour_was_rescheduled_by_the_duration_it_rolled() {
    // The strongest statement the recording can support without the condition machine: each hour the reference recomputed, the distance to its next change is
    // exactly the duration it rolled, and that duration is legal for the weather it chose. Every type in the shipped data defaults to five minutes, which is why
    // the drawn values are all three hundred - a constant that is a *default*, not a rule, and the range check is what tells the two apart.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/weather/series-24h.json");
    let text = std::fs::read_to_string(path).expect("the recorded series");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON")
    else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(steps)) = root.get("steps") else {
        panic!("no steps")
    };
    let rolls = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/weather/series-24h.rolls.txt");
    let rolls = std::fs::read_to_string(rolls).expect("the recorded dice");
    let mut hour: i64 = -1;
    let mut last_draw: std::collections::BTreeMap<i64, (i32, i32, i32)> =
        std::collections::BTreeMap::new();
    for line in rolls.lines() {
        if let Some(number) = line.strip_prefix("mark weather.hour.") {
            hour = number.parse().expect("an hour");
        } else if let Some(rest) = line.strip_prefix("int ") {
            let mut parts = rest.split(' ');
            let parsed: Vec<i32> = parts
                .by_ref()
                .filter_map(|text| text.parse().ok())
                .take(3)
                .collect();
            if parsed.len() == 3 && hour >= 0 {
                last_draw.insert(hour, (parsed[0], parsed[1], parsed[2]));
            }
        }
    }
    let mut checked = 0;
    for step in steps {
        let cdda_json::Value::Object(step) = step else {
            panic!("a step")
        };
        let hour = match step.get("hour") {
            Some(cdda_json::Value::Int(value)) => *value,
            _ => panic!("an hour"),
        };
        let Some((low, high, drawn)) = last_draw.get(&hour).copied() else {
            continue;
        };
        let turn = match step.get("turn") {
            Some(cdda_json::Value::Int(value)) => *value,
            _ => panic!("a turn"),
        };
        let next = match step.get("next_weather_turn") {
            Some(cdda_json::Value::Int(value)) => *value,
            _ => panic!("a next turn"),
        };
        assert_eq!(
            next - turn,
            i64::from(drawn),
            "hour {hour}: the due turn is the rolled duration away"
        );
        assert!(
            (low..=high).contains(&drawn),
            "hour {hour}: the draw is inside its own range"
        );
        assert_eq!(
            low,
            cdda_content::weather::DEFAULT_DURATION_TURNS,
            "hour {hour}: the default is what the data leaves"
        );
        checked += 1;
    }
    assert!(checked >= 20, "most of the day was rescheduled: {checked}");
}

/// The weather point a condition sees, with the real sun instead of a stub.
struct DaylitPoint {
    temperature: f64,
    humidity: f64,
    pressure: f64,
    windpower: f64,
    turn: i64,
    functions: cdda_content::jmath::Registry,
}

impl cdda_content::math_expr::Context for DaylitPoint {
    fn number(&self, name: &str) -> Option<f64> {
        // `is_day` is a named dialogue condition; the reference answers it from the sun (`calendar.cpp:368`), and so does this.
        match name {
            "condition:is_day" => Some(if cdda_core_types::astronomy::is_day(self.turn as f64) {
                1.0
            } else {
                0.0
            }),
            _ => None,
        }
    }
    fn call(
        &self,
        name: &str,
        args: &[cdda_content::math_expr::Value],
    ) -> Result<cdda_content::math_expr::Value, cdda_content::math_expr::MathError> {
        use cdda_content::math_expr::Value;
        let missing = || cdda_content::math_expr::MathError::UnknownName {
            name: name.to_string(),
        };
        match (name, args) {
            ("weather", [Value::Text(field)]) => match field.as_str() {
                "temperature" => Ok(Value::Number(self.temperature)),
                "humidity" => Ok(Value::Number(self.humidity)),
                "pressure" => Ok(Value::Number(self.pressure)),
                "windpower" => Ok(Value::Number(self.windpower)),
                other => Err(cdda_content::math_expr::MathError::UnknownName {
                    name: other.to_string(),
                }),
            },
            ("celsius", [Value::Number(kelvin)]) => Ok(Value::Number(kelvin - 273.15)),
            ("abs", [Value::Number(value)]) => Ok(Value::Number(value.abs())),
            ("log", [Value::Number(value)]) => Ok(Value::Number(value.ln())),
            _ => cdda_content::jmath::call(&self.functions, &Builtins, name, args)
                .map_err(|_| missing()),
        }
    }
}

/// The two functions the reference's parser has that the weather conditions use; anything else must come from the data.
struct Builtins;

impl cdda_content::math_expr::Context for Builtins {
    fn number(&self, _name: &str) -> Option<f64> {
        None
    }
}

#[test]
fn the_conditions_and_the_sun_together_reproduce_the_recorded_weather() {
    // The whole loop, on the day the reference recorded: the data's own conditions, the data's own data-defined functions, the ported sun, and the ported sort. Nothing here
    // is transcribed except the numbers the reference produced. If this passes, the weather's *decision* is ported; what remains is the manager that calls it.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
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
        let required = match object.get("required_weathers") {
            Some(cdda_json::Value::Array(list)) => list
                .iter()
                .filter_map(|id| match id {
                    cdda_json::Value::Str(id) => Some(id.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let condition = object
            .get("condition")
            .map(cdda_content::condition::Condition::from_value)
            .transpose()
            .expect("a condition");
        types.push((
            cdda_content::weather::WeatherType {
                id: id.clone(),
                priority,
                required_weathers: required,
                ..Default::default()
            },
            condition,
        ));
    }
    // `clear` and `null` are made by code and carry no condition; every other type has one.
    assert!(
        types.len() >= 17,
        "the data's weather types: {}",
        types.len()
    );

    let series = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/weather/series-24h.json"),
    )
    .expect("the recorded series");
    let cdda_json::Value::Object(root) = cdda_json::parse(series.as_bytes()).expect("valid JSON")
    else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(steps)) = root.get("steps") else {
        panic!("steps")
    };

    let mut wrong = Vec::new();
    let mut checked = 0;
    for step in steps {
        let cdda_json::Value::Object(step) = step else {
            panic!("a step")
        };
        let field = |key: &str| match step.get(key) {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => *value as f64,
            other => panic!("{key} is a number: {other:?}"),
        };
        let recorded = match step.get("weather") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            other => panic!("a weather id: {other:?}"),
        };
        let point = DaylitPoint {
            temperature: field("temperature_k"),
            humidity: field("humidity"),
            pressure: field("pressure"),
            windpower: field("windpower"),
            turn: field("turn") as i64,
            functions: functions.clone(),
        };
        let kinds: Vec<cdda_content::weather::WeatherType> =
            types.iter().map(|(kind, _)| kind.clone()).collect();
        let order = cdda_content::weather::sorted(&kinds);
        let conditions_by_id: std::collections::BTreeMap<
            &str,
            &cdda_content::condition::Condition,
        > = types
            .iter()
            .filter_map(|(kind, condition)| {
                condition
                    .as_ref()
                    .map(|condition| (kind.id.as_str(), condition))
            })
            .collect();
        let answer = cdda_content::weather::conditions(&order, &mut |candidate| {
            conditions_by_id
                .get(candidate.id.as_str())
                .is_some_and(|condition| condition.holds(&point))
        });
        checked += 1;
        if answer != recorded && wrong.len() < 6 {
            wrong.push((field("hour"), answer, recorded));
        }
    }
    assert!(checked >= 20, "the whole day was decided: {checked}");
    assert!(
        wrong.is_empty(),
        "the port decides the same sky as the reference: {wrong:?}"
    );
}
