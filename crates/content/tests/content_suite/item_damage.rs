//! An item's damage: its maximum, how many increments of degradation it has, and how damaged it looks.
//!
//! `corpus/items/damage.json` is `--dump-damage`: every one of the 9,912 types, with its category, whether it is counted in charges, its maximum damage, its
//! degradation increments, and the damage level the reference reports at six probe values (0, 1, a quarter, a half, one short of the maximum, and the maximum).
//!
//! Three rules, and only one of them is data:
//!
//! * the maximum is a **constant** — 4,000, or 0 for anything counted in charges — so the data says nothing about it;
//! * the degradation increments are **zero for almost everything**, because the loader throws away the stated multiplier unless the type is a vehicle part;
//! * the damage level multiplies before it divides, which is what makes its four bands equal.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_json::Value;
use std::collections::BTreeMap;

/// One recorded type.
struct Recorded {
    id: String,
    category: String,
    count_by_charges: bool,
    max_damage: i32,
    degrade_increments: i32,
    levels: Vec<i32>,
    probe: Vec<i32>,
}

fn dump() -> Vec<Recorded> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/damage.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    let whole = |object: &cdda_json::Object, key: &str| match object.get(key) {
        Some(Value::Int(value)) => i32::try_from(*value).expect("a small number"),
        other => panic!("{key} is a whole number: {other:?}"),
    };
    let numbers = |object: &cdda_json::Object, key: &str| match object.get(key) {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| match value {
                Value::Int(number) => *number,
                other => panic!("{key} holds whole numbers: {other:?}"),
            })
            .map(|number| i32::try_from(number).expect("a small number"))
            .collect::<Vec<i32>>(),
        other => panic!("{key} is a list: {other:?}"),
    };
    types
        .iter()
        .map(|entry| {
            let Value::Object(entry) = entry else {
                panic!("a type")
            };
            let text = |key: &str| match entry.get(key) {
                Some(Value::Str(value)) => value.clone(),
                other => panic!("{key} is text: {other:?}"),
            };
            Recorded {
                id: text("id"),
                category: text("category"),
                count_by_charges: matches!(entry.get("count_by_charges"), Some(Value::Bool(true))),
                max_damage: whole(entry, "max_damage"),
                degrade_increments: whole(entry, "degrade_increments"),
                levels: numbers(entry, "levels"),
                probe: numbers(entry, "probe"),
            }
        })
        .collect()
}

fn types() -> BTreeMap<String, ItemType> {
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect()
}

#[test]
fn every_type_reports_the_maximum_and_the_levels_the_reference_does() {
    let recorded = dump();
    assert_eq!(recorded.len(), 9_912, "every type was measured");
    let loaded = types();
    let mut differing: Vec<String> = Vec::new();
    let mut degrading = 0;
    for case in &recorded {
        let Some(item) = loaded.get(&case.id) else {
            differing.push(format!("{} is not loaded", case.id));
            continue;
        };
        // The category is what decides whether a type degrades, so a mismatch there would hide a mismatch in the rule.
        if item.category != case.category {
            differing.push(format!(
                "{} category: the port says {:?}, the reference says {:?}",
                case.id, item.category, case.category
            ));
            continue;
        }
        if u32::from(item.counts_by_charge()) != u32::from(case.count_by_charges) {
            differing.push(format!(
                "{} counts by charge: the port says {}, the reference says {}",
                case.id,
                item.counts_by_charge(),
                case.count_by_charges
            ));
            continue;
        }
        if item.damage_max() != case.max_damage {
            differing.push(format!(
                "{} max damage: the port says {}, the reference says {}",
                case.id,
                item.damage_max(),
                case.max_damage
            ));
        }
        if item.degrade_increments(1.0) != case.degrade_increments {
            differing.push(format!(
                "{} degradation increments: the port says {}, the reference says {}",
                case.id,
                item.degrade_increments(1.0),
                case.degrade_increments
            ));
        }
        if case.degrade_increments > 0 {
            degrading += 1;
        }
        for (index, damage) in case.probe.iter().enumerate() {
            let Some(expected) = case.levels.get(index) else {
                panic!("{} has a level per probe", case.id)
            };
            let got = item.damage_level(*damage);
            if got != *expected {
                differing.push(format!(
                    "{} at {damage} damage: the port says {got}, the reference says {expected}",
                    case.id
                ));
            }
        }
    }
    // The gate has to exercise both branches: 230 types degrade (every vehicle part) and 1,261 are counted in charges, so neither is a corner.
    assert!(
        degrading > 200,
        "the degradation branch has to be exercised: {degrading} types degrade"
    );
    assert!(
        recorded.iter().filter(|case| case.max_damage == 0).count() > 1_000,
        "and so does the charge-counted branch"
    );
    assert!(
        differing.is_empty(),
        "{} of {} measurements disagree; the first twenty are {:#?}",
        differing.len(),
        recorded.len() * 8,
        differing.get(..20).unwrap_or(&differing)
    );
}
