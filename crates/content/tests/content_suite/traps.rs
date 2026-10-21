//! Traps against the reference's own resolve.
//!
//! The oracle is `corpus/prototypes/traps.json`. Traps are the smallest loader
//! that still has a mandatory field set, a one-character symbol rule, and
//! inheritance, so a mistake here is a mistake the item loader would repeat.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::traps;
use cdda_json::{Value, parse};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn a_trap_takes_its_glyph_and_its_numbers() {
    let dir = std::env::temp_dir().join("cdda-trap-fixture");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_test", "name": "a test trap", "color": "red",
          "symbol": "^", "visibility": 3, "avoidance": 2, "difficulty": 1,
          "trap_radius": 1, "benign": true, "flags": [ "UNDODGEABLE" ] }
        ]"#,
    )
    .expect("fixture");
    let loaded = traps::load_paths(&[&dir]).expect("loads");
    let trap = loaded
        .iter()
        .find(|entry| entry.id == "tr_test")
        .expect("prototype");
    assert_eq!(trap.name, "a test trap");
    assert_eq!(trap.symbol, i32::from(b'^'));
    assert_eq!(trap.color_name(), "c_red");
    assert_eq!(trap.visibility, 3);
    assert_eq!(trap.avoidance, 2);
    assert_eq!(trap.difficulty, 1);
    assert_eq!(trap.radius, 1);
    assert!(trap.benign);
    assert!(trap.flags.iter().any(|flag| flag == "UNDODGEABLE"));
}

#[test]
fn a_symbol_that_is_not_one_character_is_rejected() {
    let dir = std::env::temp_dir().join("cdda-trap-bad-symbol");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_bad", "name": "x", "color": "red", "symbol": "ab",
          "visibility": 1, "avoidance": 0, "difficulty": 0 }
        ]"#,
    )
    .expect("fixture");
    let error = traps::load_paths(&[&dir]).expect_err("two characters are not a glyph");
    assert!(error.to_string().contains("one character"), "{error}");
}

#[test]
fn every_trap_matches_the_reference() {
    let text =
        std::fs::read_to_string(root().join("corpus/prototypes/traps.json")).expect("artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let Value::Object(root_object) = value else {
        panic!("root")
    };
    let Some(Value::Array(entries)) = root_object.get("prototypes") else {
        panic!("prototypes")
    };
    let loaded = traps::load_paths(&[
        &root().join("references/Cataclysm-DDA/data/core"),
        &root().join("references/Cataclysm-DDA/data/json"),
    ])
    .expect("loads");
    println!(
        "loaded {} traps, reference has {}",
        loaded.len(),
        entries.len()
    );
    let mut mismatches = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        let text_of = |member: &str| match object.get(member) {
            Some(Value::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let number_of = |member: &str| match object.get(member) {
            Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(i32::MAX),
            _ => 0,
        };
        let flag_of = |member: &str| match object.get(member) {
            Some(Value::Bool(value)) => *value,
            _ => false,
        };
        let id = text_of("id");
        let Some(ours) = loaded.iter().find(|trap| trap.id == id) else {
            mismatches.push(format!("{id}: missing"));
            continue;
        };
        for (field, ours_value, theirs) in [
            ("name", ours.name.clone(), text_of("name")),
            ("color", ours.color_name().to_owned(), text_of("color")),
        ] {
            if ours_value != theirs {
                mismatches.push(format!("{id}: {field} {ours_value:?} != {theirs:?}"));
            }
        }
        for (field, ours_value, theirs) in [
            ("symbol", ours.symbol, number_of("symbol")),
            ("visibility", ours.visibility, number_of("visibility")),
            ("avoidance", ours.avoidance, number_of("avoidance")),
            ("difficulty", ours.difficulty, number_of("difficulty")),
            ("radius", ours.radius, number_of("radius")),
        ] {
            if ours_value != theirs {
                mismatches.push(format!("{id}: {field} {ours_value} != {theirs}"));
            }
        }
        for (field, ours_value, theirs) in [
            ("benign", ours.benign, flag_of("benign")),
            (
                "always_invisible",
                ours.always_invisible,
                flag_of("always_invisible"),
            ),
        ] {
            if ours_value != theirs {
                mismatches.push(format!("{id}: {field} {ours_value} != {theirs}"));
            }
        }
        // The reference keeps flags in a sorted set whose order is the flag's
        // registration order, which is not something a port should reproduce;
        // the set is what has to match.
        let ours_flags: Vec<String> = ours.flags.iter().cloned().collect();
        let theirs_flags: Vec<String> = match object.get("flags") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| match item {
                    Value::Str(text) => Some(text.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let mut theirs_flags = theirs_flags;
        theirs_flags.sort();
        if ours_flags != theirs_flags {
            mismatches.push(format!("{id}: flags {ours_flags:?} != {theirs_flags:?}"));
        }
    }
    println!("{} mismatches", mismatches.len());
    for line in mismatches.iter().take(15) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
