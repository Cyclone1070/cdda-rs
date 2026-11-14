//! Every item type's use methods, and the two maps they live in.
//!
//! `corpus/items/use-methods.json` is `--dump-use-methods`: 2,653 types, each with its actions **in the map's own order**, and `corpus/items/ammo-scales.json` is
//! `--dump-ammo-scales`: 1,054 types with a per-action charge scale. Three rules are visible only here, and all three follow from the reference storing these in
//! `std::map`s (`src/itype.h:1410` and `use_methods`):
//! * the actions come back **sorted by name**, not in the order the data writes them;
//! * two entries with the same name **collapse**, the first winning, because the loader `emplace`s (`src/generic_factory.h:1040-1050`);
//! * a `repair_item` actor is registered under its **`item_action_type`** (`src/item_factory.cpp:1858-1865`), so a repairkit answers to `repair_metal`.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn recorded(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items")
        .join(name);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

fn loaded() -> BTreeMap<String, ItemType> {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect()
}

#[test]
fn every_type_has_the_use_methods_the_reference_has_in_the_order_it_has_them() {
    let Value::Object(root) = recorded("use-methods.json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    assert_eq!(types.len(), 2653, "every type with an action was recorded");
    let items = loaded();

    let mut differing: Vec<String> = Vec::new();
    let mut renamed = 0;
    let mut sorted_differently = 0;
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("a type id")
        };
        let Some(Value::Array(methods)) = entry.get("methods") else {
            panic!("{id} has methods")
        };
        let expected: Vec<(String, String)> = methods
            .iter()
            .map(|method| {
                let Value::Object(method) = method else {
                    panic!("a method")
                };
                (
                    match method.get("key") {
                        Some(Value::Str(key)) => key.clone(),
                        other => panic!("a key: {other:?}"),
                    },
                    match method.get("actor") {
                        Some(Value::Str(actor)) => actor.clone(),
                        other => panic!("an actor: {other:?}"),
                    },
                )
            })
            .collect();
        if expected.iter().any(|(key, _)| key.starts_with("repair_")) {
            renamed += 1;
        }
        // The reference's order is a map's, so it is sorted; a type whose data order differs from that is proof the port is sorting and not echoing.
        let mut data_order = expected.clone();
        data_order.sort_by(|a, b| b.0.cmp(&a.0));
        if data_order != expected {
            sorted_differently += 1;
        }
        let Some(item) = items.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let ours: Vec<(String, String)> = item
            .use_actions
            .iter()
            .map(|(name, action)| (name.clone(), action.kind().to_owned()))
            .collect();
        if ours != expected {
            differing.push(format!(
                "{id}: the port says {ours:?}, the reference says {expected:?}"
            ));
        }
    }
    assert!(
        renamed > 0,
        "the `repair_item` rename has to be exercised: {renamed}"
    );
    assert!(
        sorted_differently > 100,
        "and the map's ordering has to be visible rather than coincidental: {sorted_differently}"
    );
    assert!(
        differing.is_empty(),
        "{} of 2653 types disagree; the first ten are {:#?}",
        differing.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

#[test]
fn every_type_has_the_charge_scales_the_reference_has() {
    let Value::Object(root) = recorded("ammo-scales.json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    assert_eq!(types.len(), 1054, "every type with a scale was recorded");
    let items = loaded();

    let mut differing: Vec<String> = Vec::new();
    let mut from_arrays = 0;
    let mut values: BTreeSet<i32> = BTreeSet::new();
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("a type id")
        };
        let Some(Value::Object(scales)) = entry.get("scales") else {
            panic!("{id} has scales")
        };
        let expected: BTreeMap<String, i32> = scales
            .members()
            .iter()
            .map(|(name, value)| match value {
                Value::Int(value) => (name.clone(), i32::try_from(*value).expect("a small scale")),
                other => panic!("a scale: {other:?}"),
            })
            .collect();
        for (name, value) in &expected {
            values.insert(*value);
            // The array shape `[ "CROWBAR", 0 ]` is where a scale carries no member of its own.
            if name == "CROWBAR" || name == "HACKSAW" {
                from_arrays += 1;
            }
        }
        let Some(item) = items.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        if item.ammo_scale != expected {
            differing.push(format!(
                "{id}: the port says {:?}, the reference says {expected:?}",
                item.ammo_scale
            ));
        }
    }
    assert!(
        from_arrays > 0,
        "a scale read from an array entry: {from_arrays}"
    );
    // **A zero is a real value, not "unset"**: every scale in this content is zero, which is exactly the case a port that treated zero as absent would blur into
    // "scale of one" and never notice.
    assert_eq!(
        values,
        [0].into_iter().collect(),
        "every recorded scale is zero: {values:?}"
    );
    assert!(
        differing.is_empty(),
        "{} of 1054 types disagree; the first ten are {:#?}",
        differing.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}
