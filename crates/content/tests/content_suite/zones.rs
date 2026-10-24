//! The zone registry: what a zone type is, and every one the data defines.
//!
//! Source: `--dump-zone-types` from the instrumented reference (`harness.cpp`), which walks `zone_type::get_all()`. A zone type is loaded from JSON under the type string
//! `LOOT_ZONE` (`src/clzones.cpp:147-166`, registered at `src/init.cpp:370`), so the name of the JSON tag is the one thing about this registry that does not read the way it
//! looks.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::zones::{ZoneType, load_paths};
use cdda_json::Value;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/zones/all.json")
}

/// The directories the reference loads from, in its order.
fn dirs() -> [PathBuf; 2] {
    crate::corpus::dirs()
}

/// The reference's own rows, read back.
struct Row {
    id: String,
    name: String,
    desc: String,
    display_field: String,
    can_be_personal: bool,
    hidden: bool,
}

fn text(object: &cdda_json::Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key} is a string: {other:?}"),
    }
}

fn flag(object: &cdda_json::Object, key: &str) -> bool {
    match object.get(key) {
        Some(Value::Bool(value)) => *value,
        other => panic!("{key} is a bool: {other:?}"),
    }
}

fn reference() -> Vec<Row> {
    let json = std::fs::read_to_string(corpus()).expect("the zone type dump");
    let Value::Object(root) = cdda_json::parse(json.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(Value::Array(rows)) = root.get("zones") else {
        panic!("zones")
    };
    rows.iter()
        .map(|row| {
            let Value::Object(row) = row else {
                panic!("a row")
            };
            Row {
                id: text(row, "id"),
                name: text(row, "name"),
                desc: text(row, "desc"),
                display_field: text(row, "display_field"),
                can_be_personal: flag(row, "can_be_personal"),
                hidden: flag(row, "hidden"),
            }
        })
        .collect()
}

/// The registry, in the order the data gives it.
#[test]
fn every_zone_type_the_data_defines_loads_as_the_reference_holds_it() {
    let reference = reference();
    assert_eq!(reference.len(), 64, "the dump holds every zone type of the dda set");
    let dirs = dirs();
    let args: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    let loaded = load_paths(&args).expect("the data loads");
    assert_eq!(
        loaded.len(),
        reference.len(),
        "the port defines the same number of zone types"
    );
    let mut wrong = Vec::new();
    for row in &reference {
        let Some(zone) = loaded.get(&row.id) else {
            wrong.push(format!("{}: missing from the port", row.id));
            continue;
        };
        let expected = ZoneType {
            id: row.id.clone(),
            name: row.name.clone(),
            description: row.desc.clone(),
            display_field: row.display_field.clone(),
            can_be_personal: row.can_be_personal,
            hidden: row.hidden,
        };
        if *zone != expected {
            wrong.push(format!("{}: the port has {zone:?}, the reference has {expected:?}", row.id));
        }
    }
    for id in loaded.keys() {
        if !reference.iter().any(|row| &row.id == id) {
            wrong.push(format!("{id}: the port invents a zone type"));
        }
    }
    assert!(wrong.is_empty(), "{} wrong of 64:\n{}", wrong.len(), wrong.join("\n"));
}

/// The flags a zone carries decide whether a character may own it and whether the UI offers it, so they are asserted on both sides rather than trusted to the count.
#[test]
fn personal_and_hidden_zone_types_are_the_reference_s() {
    let reference = reference();
    let dirs = dirs();
    let args: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    let loaded = load_paths(&args).expect("the data loads");
    let mut personal: Vec<&str> = reference
        .iter()
        .filter(|row| row.can_be_personal)
        .map(|row| row.id.as_str())
        .collect();
    let mut hidden: Vec<&str> = reference
        .iter()
        .filter(|row| row.hidden)
        .map(|row| row.id.as_str())
        .collect();
    personal.sort_unstable();
    hidden.sort_unstable();
    assert!(!personal.is_empty() && !hidden.is_empty(), "both flags are exercised");
    for id in &personal {
        assert!(loaded[*id].can_be_personal, "{id} may be owned");
    }
    for id in &hidden {
        assert!(loaded[*id].hidden, "{id} is hidden");
    }
    let mut ported_personal: Vec<&str> = loaded
        .values()
        .filter(|zone| zone.can_be_personal)
        .map(|zone| zone.id.as_str())
        .collect();
    // Sorted: the port keeps the registry as a map, so its order is the ids' order, while the reference returns the factory's. The **set** is what is being compared.
    ported_personal.sort_unstable();
    assert_eq!(ported_personal, personal, "the same zones may be owned");
}
