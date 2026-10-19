//! Field types against the reference's own resolve.
//!
//! The interesting rule is inheritance *inside* a list: a level starts as a copy
//! of the level before it, so a fire field's third level is its second level
//! plus whatever it states - and the first level starts from the C++ defaults,
//! not from zero.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::fields;
use cdda_json::{Value, parse};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn a_level_inherits_the_one_before_it_but_not_its_concentration() {
    let dir = std::env::temp_dir().join("cdda-field-fixture");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("fields.json"),
        r#"[
        {
          "type": "field_type",
          "id": "fd_test",
          "priority": 7,
          "intensity_levels": [
            { "name": "wisps", "sym": "~", "color": "light_gray", "concentration": 3 },
            { "name": "haze" },
            { "name": "cloud", "sym": "!", "dangerous": true }
          ]
        }
        ]"#,
    )
    .expect("fixture");
    let loaded = fields::load_paths(&[&dir]).expect("loads");
    let field = loaded
        .iter()
        .find(|entry| entry.id == "fd_test")
        .expect("prototype");
    assert_eq!(field.priority, 7);
    assert_eq!(field.levels.len(), 3);
    assert_eq!(field.levels[0].name, "wisps");
    assert_eq!(field.levels[0].symbol, u32::from(b'~'));
    assert_eq!(field.levels[0].concentration, 3);
    // The second level states only a name, so it keeps the glyph and colour.
    assert_eq!(field.levels[1].symbol, u32::from(b'~'));
    assert_eq!(field.levels[1].color_name(), "c_light_gray");
    // ...but concentration is the one field with its own default of 1.
    assert_eq!(field.levels[1].concentration, 1);
    // The third level overrides the glyph and sets danger, keeping the rest.
    assert_eq!(field.levels[2].symbol, u32::from(b'!'));
    assert!(field.levels[2].dangerous);
    assert_eq!(field.levels[2].name, "cloud");
}

/// Compares one level's fields, appending what differs.
fn compare_level(
    id: &str,
    at: usize,
    theirs: &cdda_json::Object,
    ours: &cdda_content::fields::IntensityLevel,
    mismatches: &mut Vec<String>,
) {
    let text_of = |member: &str| match theirs.get(member) {
        Some(Value::Str(value)) => value.clone(),
        _ => String::new(),
    };
    let number_of = |member: &str| match theirs.get(member) {
        Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(i32::MAX),
        _ => 0,
    };
    let flag_of = |member: &str| matches!(theirs.get(member), Some(Value::Bool(true)));
    if ours.name != text_of("name") {
        mismatches.push(format!(
            "{id}[{at}]: name {:?} != {:?}",
            ours.name,
            text_of("name")
        ));
    }
    if ours.color_name() != text_of("color") {
        mismatches.push(format!(
            "{id}[{at}]: color {:?} != {:?}",
            ours.color_name(),
            text_of("color")
        ));
    }
    let sym = match theirs.get("sym") {
        Some(Value::Int(value)) => u32::try_from(*value).unwrap_or(u32::MAX),
        _ => 0,
    };
    if ours.symbol != sym {
        mismatches.push(format!("{id}[{at}]: sym {} != {sym}", ours.symbol));
    }
    for (field, ours_value, theirs_value) in [
        ("move_cost", ours.move_cost, number_of("move_cost")),
        (
            "concentration",
            ours.concentration,
            number_of("concentration"),
        ),
    ] {
        if ours_value != theirs_value {
            mismatches.push(format!(
                "{id}[{at}]: {field} {ours_value} != {theirs_value}"
            ));
        }
    }
    for (field, ours_value, theirs_value) in [
        ("dangerous", ours.dangerous, flag_of("dangerous")),
        ("transparent", ours.transparent, flag_of("transparent")),
    ] {
        if ours_value != theirs_value {
            mismatches.push(format!(
                "{id}[{at}]: {field} {ours_value} != {theirs_value}"
            ));
        }
    }
}

#[test]
fn every_field_type_matches_the_reference() {
    let text =
        std::fs::read_to_string(root().join("corpus/prototypes/fields.json")).expect("artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let Value::Object(root_object) = value else {
        panic!("root")
    };
    let Some(Value::Array(entries)) = root_object.get("prototypes") else {
        panic!("prototypes")
    };
    let loaded = fields::load_paths(&[
        &root().join("references/Cataclysm-DDA/data/core"),
        &root().join("references/Cataclysm-DDA/data/json"),
    ])
    .expect("loads");
    println!(
        "loaded {} field types, reference has {}",
        loaded.len(),
        entries.len()
    );
    let mut mismatches = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        let id = match object.get("id") {
            Some(Value::Str(text)) => text.clone(),
            _ => continue,
        };
        let Some(ours) = loaded.iter().find(|field| field.id == id) else {
            mismatches.push(format!("{id}: missing"));
            continue;
        };
        let priority = match object.get("priority") {
            Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(i32::MAX),
            _ => 0,
        };
        if ours.priority != priority {
            mismatches.push(format!("{id}: priority {} != {priority}", ours.priority));
        }
        let Some(Value::Array(levels)) = object.get("levels") else {
            continue;
        };
        if ours.levels.len() != levels.len() {
            mismatches.push(format!(
                "{id}: {} levels != {}",
                ours.levels.len(),
                levels.len()
            ));
            continue;
        }
        for (at, (theirs, level)) in levels.iter().zip(&ours.levels).enumerate() {
            let Value::Object(theirs) = theirs else {
                continue;
            };
            compare_level(&id, at, theirs, level, &mut mismatches);
        }
    }
    println!("{} mismatches", mismatches.len());
    for line in mismatches.iter().take(15) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
