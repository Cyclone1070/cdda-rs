//! The weather as a dialogue question.
//!
//! Reference: `f_is_weather` (`condition.cpp:1812-1818`) compares `get_weather().weather_id` with the id the guard names. The port's
//! `Weather` carries only the sight penalty the light needs, so the id is its own component - data beside a number, and the
//! condition asks the store rather than a table the caller built.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::weather;

fn condition(text: &str) -> cdda_content::dialogue::Condition {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    cdda_content::dialogue::read_condition(&value).expect("the port reads this guard")
}

#[test]
fn the_condition_names_the_weather_that_is_happening() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let drizzle = condition(r#"{"is_weather": "drizzle"}"#);
    let any_rain = condition(r#"{"is_weather": ["rain", "thunder"]}"#);

    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &drizzle),
        "a sky nobody has named is not drizzle"
    );

    weather::set_id(&mut world, "rain");
    assert!(cdda_sim::conditions::holds(&world, survivor, &any_rain));
    assert!(!cdda_sim::conditions::holds(&world, survivor, &drizzle));
    assert_eq!(weather::id_of(&world), Some("rain".to_owned()));

    weather::set_id(&mut world, "drizzle");
    assert!(cdda_sim::conditions::holds(&world, survivor, &drizzle));
    assert!(!cdda_sim::conditions::holds(&world, survivor, &any_rain));

    // The id and the sight penalty are separate: naming the weather does not change what the light costs.
    cdda_sim::singletons::set_weather(&mut world, weather::Weather::clear());
    weather::set_id(&mut world, "snowing");
    assert_eq!(weather::id_of(&world), Some("snowing".to_owned()));
    assert_eq!(
        cdda_sim::singletons::weather(&world).map(weather::Weather::sight_penalty),
        Some(1.0),
        "the sky's penalty is still the clear-air one"
    );
}

/// The weather ids the tests use are ids the data defines, checked the way the trait and mission tests are.
#[test]
fn the_weather_ids_in_the_data_are_what_the_tests_use() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut ids = std::collections::BTreeSet::new();
    let mut stack = vec![data];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
                continue;
            }
            let Some(shared) = cdda_content::read_json(&path) else {
                continue;
            };
            let cdda_json::Value::Array(ref items) = *shared else {
                continue;
            };
            for item in items {
                let cdda_json::Value::Object(object) = item else {
                    continue;
                };
                if matches!(object.get("type"), Some(cdda_json::Value::Str(kind)) if kind == "weather_type")
                    && let Some(cdda_json::Value::Str(id)) = object.get("id")
                {
                    ids.insert(id.clone());
                }
            }
        }
    }
    println!("weather types in the data: {}", ids.len());
    // `snow` is not a weather id; `snowing` and `snowstorm` are. The check earns its keep again.
    for id in ["drizzle", "rain", "snowing", "thunder"] {
        assert!(ids.contains(id), "`{id}` is a real weather type");
    }
}
