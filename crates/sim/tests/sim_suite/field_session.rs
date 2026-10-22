//! A property of the fixture itself, kept because it is cheap and it fails loudly.
//!
//! `corpus/fields/fire-floor.json` is a trace of one fire on `t_floor`, written by the oracle's `--dump-field-series`. Its ages go up by more than one a
//! turn - the fire eats its ground and banks the fuel, and a banked fire burns *longer* - so at least one step in the trace must go **down**. That downward
//! step is the whole reason the fuel rule has a sign in it, and a future edit that "fixes" ages so they only ever rise would quietly delete it.
//!
//! The replay of this fixture lives in [`crate::corpus_sessions`], which spends the reference's own dice; this only guards the property.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_json::{Value, parse};

/// One row of the trace: what the reference had after a turn.
#[derive(Debug)]
struct Row {
    turn: i64,
    age: i64,
}

fn trace() -> Vec<Row> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture at {}: {error}", path.display()));
    let value = parse(text.as_bytes()).expect("the fixture parses");
    let object = match &value {
        Value::Object(members) => members,
        other => panic!("expected an object, found {other:?}"),
    };
    let turns = match object.get("turns") {
        Some(Value::Array(items)) => items,
        other => panic!("expected a turn list, found {other:?}"),
    };
    let mut rows = Vec::new();
    for (turn, entry) in turns.iter().enumerate() {
        let Value::Object(entry) = entry else {
            continue;
        };
        let Some(Value::Array(fields)) = entry.get("fields") else {
            continue;
        };
        for field in fields {
            let Value::Object(field) = field else {
                continue;
            };
            if !matches!(field.get("type"), Some(Value::Str(kind)) if kind == "fd_fire") {
                continue;
            }
            let age = match field.get("age") {
                Some(Value::Int(value)) => *value,
                other => panic!("expected an age, found {other:?}"),
            };
            rows.push(Row {
                turn: i64::try_from(turn).expect("a turn fits"),
                age,
            });
        }
    }
    rows
}

#[test]
fn the_trace_really_does_have_a_turn_where_the_age_goes_down() {
    let rows = trace();
    let mut ages: Vec<(i64, i64)> = rows.iter().map(|row| (row.turn, row.age)).collect();
    ages.sort_unstable();
    let fell = ages.windows(2).any(|pair| pair[1].1 < pair[0].1);
    assert!(
        fell,
        "the reference's own trace has one downward step: {ages:?}"
    );
}
