//! The order the reference tests weather types in.
//!
//! Source: `--dump-weather-order`, which writes both the registry's own order and the sorted order the game tests against. `get_weather_conditions` walks that
//! sorted list and lets the **last** type whose condition holds win (`weather_gen.cpp:196-215`), so the order is behaviour: two weathers that could both apply
//! are decided by which comes later, and a port that sorted differently would report a different sky from the same numbers.
//!
//! The sort is `std::sort` on `priority`, which is not stable, but the data's own order survives for the ties that exist - `mist` and `fog` both carry 15 and
//! the game keeps them in declaration order - so the port sorts stably and the fixture proves it.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

use cdda_json::{Object, Value, parse};
use std::path::Path;

fn fixture() -> (Vec<cdda_content::weather::WeatherType>, Vec<String>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/order.json");
    let text = std::fs::read_to_string(path).expect("the recorded order");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let entries = |key: &str| -> Vec<Object> {
        let Some(Value::Array(list)) = root.get(key) else {
            panic!("no {key}")
        };
        list.iter()
            .map(|entry| match entry {
                Value::Object(object) => object.clone(),
                _ => panic!("{key} holds objects"),
            })
            .collect()
    };
    let registry: Vec<cdda_content::weather::WeatherType> = entries("registry")
        .iter()
        .map(|object| {
            let Value::Str(id) = object.get("id").expect("an id") else {
                panic!("an id is a string")
            };
            let priority = match object.get("priority") {
                Some(Value::Int(value)) => *value as i32,
                Some(Value::Float(value)) => *value as i32,
                _ => 0,
            };
            cdda_content::weather::WeatherType {
                id: id.clone(),
                priority,
                required_weathers: required(object),
                ..Default::default()
            }
        })
        .collect();
    let order: Vec<String> = entries("order")
        .iter()
        .map(|object| match object.get("id").expect("an id") {
            Value::Str(id) => id.clone(),
            _ => panic!("an overt id"),
        })
        .collect();
    (registry, order)
}

fn required(object: &Object) -> Vec<String> {
    let Some(Value::Array(list)) = object.get("required") else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|id| match id {
            Value::Str(id) => Some(id.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_port_sorts_the_weather_types_the_way_the_reference_sorts_them() {
    let (registry, expected) = fixture();
    assert_eq!(
        registry.len(),
        20,
        "the registry holds twenty weather types"
    );
    let sorted: Vec<String> = cdda_content::weather::sorted(&registry)
        .iter()
        .map(|kind| kind.id.clone())
        .collect();
    assert_eq!(
        sorted, expected,
        "the sort by priority, stable within a tie"
    );
}

#[test]
fn the_tie_at_fifteen_is_the_data_s_order_and_not_a_lucky_one() {
    // `mist` and `fog` both carry priority 15, and the reference's sort is not stable - so this tie is the one place where the port's stable sort could quietly
    // disagree. The reference keeps mist first because the data declares it first; if the port ever sorted by, say, priority then name, this would fail while
    // the previous test still passed on a coincidence of the other eighteen priorities.
    let (mut registry, _) = fixture();
    let mist = registry
        .iter()
        .position(|kind| kind.id == "mist")
        .expect("mist");
    let fog = registry
        .iter()
        .position(|kind| kind.id == "fog")
        .expect("fog");
    assert!(mist < fog, "the data declares mist first");
    let sorted = cdda_content::weather::sorted(&registry);
    let ids: Vec<&str> = sorted.iter().map(|kind| kind.id.as_str()).collect();
    let mist_at = ids
        .iter()
        .position(|id| *id == "mist")
        .expect("mist sorted");
    let fog_at = ids.iter().position(|id| *id == "fog").expect("fog sorted");
    assert!(
        mist_at + 1 == fog_at,
        "the tie keeps the data's order: {ids:?}"
    );
    registry.reverse();
    let reversed = cdda_content::weather::sorted(&registry);
    let ids: Vec<&str> = reversed.iter().map(|kind| kind.id.as_str()).collect();
    assert!(
        ids.iter().position(|id| *id == "fog").expect("fog")
            < ids.iter().position(|id| *id == "mist").expect("mist"),
        "and the order follows the data when the data is reordered, which is what makes it stable rather than fixed"
    );
}
