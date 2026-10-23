//! The loop that turns a set of satisfied weather conditions into a weather id.
//!
//! Reference: `weather_gen.cpp:196-215`. Start at `clear`, walk the sorted types, and replace the answer whenever a type's own requirements are met and its
//! condition holds. Two things decide the result and neither is the condition itself: the **last** match wins, and a type that names `required_weathers` only
//! applies if the current answer is one of them - which is what makes rain follow drizzle rather than appear from a clear sky.
//!
//! The conditions are dialogue expressions and are not ported yet, so the loop takes the evaluation as a parameter. That is not a shortcut around the gate:
//! the loop's own rules are what is being pinned here, and passing the evaluation in is how a caller with no dialogue machine can still be correct about them.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

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
    assert_eq!(answer, "sunny", "the chain above the break is never reached");
    for chained in ["light_drizzle", "drizzle", "rain", "thunder"] {
        assert_ne!(answer, chained, "{chained} cannot apply without the cloud under it");
    }
}

#[test]
fn the_last_type_whose_condition_holds_wins() {
    // `sunny` and `cloudy` both carry no requirements, so both are candidates; the sorted order puts cloudy later and the loop keeps the later one. This is
    // the rule that makes the sort part of the weather's behaviour rather than a detail of storage.
    let types = vec![kind("clear", 0, &[]), kind("sunny", 10, &[]), kind("cloudy", 20, &[])];
    let order = sorted(&types);
    let answer = conditions(&order, &mut |candidate| candidate.id != "clear");
    assert_eq!(answer, "cloudy", "the later of two candidates");
}

#[test]
fn the_answer_starts_at_clear_and_only_moves_forward() {
    // Reference: `weather_type_id current_conditions = WEATHER_CLEAR;`. With nothing satisfied the answer is clear, not null and not the first entry.
    let types = chain();
    let order = sorted(&types);
    assert_eq!(conditions(&order, &mut |_| false), "clear", "nothing holds, so the sky is clear");
}

#[test]
fn the_weather_is_recomputed_on_the_turn_it_is_due_and_not_before() {
    // Reference: `weather.cpp:904` - `if( weather_id == WEATHER_NULL || calendar::turn >= nextweather )`. The boundary is inclusive, and the first update
    // happens because there is no weather yet rather than because a turn has passed.
    let mut schedule = cdda_content::weather::Schedule::default();
    assert!(schedule.needs_update(0), "no weather yet, so the first turn recomputes");
    schedule.reschedule("sunny".to_string(), 100, 300);
    assert_eq!(schedule.next_turn, 400, "the duration is added to the turn it was drawn at");
    assert!(!schedule.needs_update(399), "one turn early is not due");
    assert!(schedule.needs_update(400), "the due turn itself is due");
}

#[test]
fn every_recorded_hour_was_rescheduled_by_the_duration_it_rolled() {
    // The strongest statement the recording can support without the condition machine: each hour the reference recomputed, the distance to its next change is
    // exactly the duration it rolled, and that duration is legal for the weather it chose. Every type in the shipped data defaults to five minutes, which is why
    // the drawn values are all three hundred - a constant that is a *default*, not a rule, and the range check is what tells the two apart.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/series-24h.json");
    let text = std::fs::read_to_string(path).expect("the recorded series");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(steps)) = root.get("steps") else { panic!("no steps") };
    let rolls = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/series-24h.rolls.txt");
    let rolls = std::fs::read_to_string(rolls).expect("the recorded dice");
    let mut hour: i64 = -1;
    let mut last_draw: std::collections::BTreeMap<i64, (i32, i32, i32)> = std::collections::BTreeMap::new();
    for line in rolls.lines() {
        if let Some(number) = line.strip_prefix("mark weather.hour.") {
            hour = number.parse().expect("an hour");
        } else if let Some(rest) = line.strip_prefix("int ") {
            let mut parts = rest.split(' ');
            let parsed: Vec<i32> = parts.by_ref().filter_map(|text| text.parse().ok()).take(3).collect();
            if parsed.len() == 3 && hour >= 0 {
                last_draw.insert(hour, (parsed[0], parsed[1], parsed[2]));
            }
        }
    }
    let mut checked = 0;
    for step in steps {
        let cdda_json::Value::Object(step) = step else { panic!("a step") };
        let hour = match step.get("hour") {
            Some(cdda_json::Value::Int(value)) => *value,
            _ => panic!("an hour"),
        };
        let Some((low, high, drawn)) = last_draw.get(&hour).copied() else { continue };
        let turn = match step.get("turn") {
            Some(cdda_json::Value::Int(value)) => *value,
            _ => panic!("a turn"),
        };
        let next = match step.get("next_weather_turn") {
            Some(cdda_json::Value::Int(value)) => *value,
            _ => panic!("a next turn"),
        };
        assert_eq!(next - turn, i64::from(drawn), "hour {hour}: the due turn is the rolled duration away");
        assert!((low..=high).contains(&drawn), "hour {hour}: the draw is inside its own range");
        assert_eq!(low, cdda_content::weather::DEFAULT_DURATION_TURNS, "hour {hour}: the default is what the data leaves");
        checked += 1;
    }
    assert!(checked >= 20, "most of the day was rescheduled: {checked}");
}
