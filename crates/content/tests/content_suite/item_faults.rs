//! Which faults each item type can acquire, with what weights.
//!
//! `corpus/items/item-faults.json` is `--dump-item-faults`: 915 types, each with the weighted fault list the reference resolves. The interesting part is that the
//! list is **not** what the type states: a type may point at a `fault_group`, and the group is expanded in a finalisation pass (`item_factory.cpp:919-936`), so the
//! resolved list mixes direct weights, replaced weights and appended ones in the order the type happens to state them.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::faults::load_groups;
use cdda_content::items::{ItemType, expand_fault_groups, load_paths};
use cdda_json::Value;
use std::collections::BTreeMap;

/// Reads the recorded lists.
fn dump() -> BTreeMap<String, Vec<(String, i32)>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/item-faults.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    let mut out = BTreeMap::new();
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("an id")
        };
        let Some(Value::Array(faults)) = entry.get("faults") else {
            panic!("{id} has faults")
        };
        let list = faults
            .iter()
            .map(|fault| {
                let Value::Object(fault) = fault else {
                    panic!("a fault")
                };
                let id = match fault.get("fault") {
                    Some(Value::Str(id)) => id.clone(),
                    other => panic!("a fault id: {other:?}"),
                };
                let weight = match fault.get("weight") {
                    Some(Value::Int(weight)) => i32::try_from(*weight).expect("a small weight"),
                    other => panic!("a weight: {other:?}"),
                };
                (id, weight)
            })
            .collect();
        out.insert(id.clone(), list);
    }
    out
}

/// The reference's data directories.
fn dirs() -> Vec<std::path::PathBuf> {
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    vec![reference.join("core"), reference.join("json")]
}

fn loaded() -> BTreeMap<String, ItemType> {
    let dirs = dirs();
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect()
}

#[test]
fn every_type_can_acquire_the_faults_the_reference_gives_it() {
    let recorded = dump();
    assert_eq!(recorded.len(), 915, "every type with faults was recorded");
    let dirs = dirs();
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let groups = load_groups(&paths).expect("the groups load");
    assert_eq!(groups.len(), 8, "the eight groups the content defines");
    let mut items = loaded();
    let mut pointed_at_groups = 0;
    for item in items.values_mut() {
        if !item.fault_groups.is_empty() {
            pointed_at_groups += 1;
        }
        expand_fault_groups(item, &groups);
    }
    assert!(
        pointed_at_groups > 100,
        "the expansion has to be exercised, or it is untested: {pointed_at_groups} types point at a group"
    );

    let mut differing: Vec<String> = Vec::new();
    for (id, expected) in &recorded {
        let Some(item) = items.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        if &item.faults != expected {
            differing.push(format!(
                "{id}: the port says {:?}, the reference says {expected:?}",
                item.faults
            ));
        }
    }
    assert!(
        differing.is_empty(),
        "{} of {} types disagree; the first twenty are {:#?}",
        differing.len(),
        recorded.len(),
        differing.get(..20).unwrap_or(&differing)
    );
}
