//! Tool qualities: what makes a hammer a hammer.
//!
//! `corpus/qualities/all.json` is `--dump-qualities`: the quality registry (87 of them) and every item type that is worth something as a tool (1,091 of them), with the level
//! it is worth. The levels are the interesting part: the reference uses a **negative** level to mean "worse than bare hands", which is how a saw is a worse butchering
//! tool than a knife.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::{items, qualities};
use cdda_json::Value;
use std::collections::BTreeMap;

/// The dump: every quality with its name, and every item's qualities by id.
fn recorded() -> (
    BTreeMap<String, String>,
    BTreeMap<String, BTreeMap<String, i32>>,
) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bytes = std::fs::read(root.join("corpus/qualities/all.json")).expect("the dump");
    let Value::Object(object) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let text = |object: &cdda_json::Object, key: &str| -> String {
        match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is text: {other:?}"),
        }
    };
    let Some(Value::Array(all)) = object.get("qualities") else {
        panic!("the dump holds qualities")
    };
    let mut registry = BTreeMap::new();
    for entry in all {
        let Value::Object(entry) = entry else {
            panic!("a quality")
        };
        registry.insert(text(entry, "id"), text(entry, "name"));
    }
    let Some(Value::Array(all)) = object.get("items") else {
        panic!("the dump holds items")
    };
    let mut item_qualities = BTreeMap::new();
    for entry in all {
        let Value::Object(entry) = entry else {
            panic!("an item")
        };
        let Some(Value::Array(pairs)) = entry.get("qualities") else {
            panic!("an item's qualities")
        };
        let mut levels = BTreeMap::new();
        for pair in pairs {
            let Value::Array(pair) = pair else {
                panic!("a pair")
            };
            let Some(Value::Str(id)) = pair.first() else {
                panic!("a quality id")
            };
            let Some(Value::Int(level)) = pair.get(1) else {
                panic!("a level")
            };
            levels.insert(id.clone(), i32::try_from(*level).expect("a level"));
        }
        item_qualities.insert(text(entry, "id"), levels);
    }
    (registry, item_qualities)
}

#[test]
fn every_quality_and_every_item_that_has_one_loads_as_the_reference_gives_it() {
    let dirs = crate::corpus::dirs();
    let args: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let (recorded_registry, recorded_items) = recorded();
    assert_eq!(
        recorded_registry.len(),
        87,
        "the reference defines 87 qualities"
    );
    assert_eq!(recorded_items.len(), 1_091, "1,091 item types have one");

    let loaded_registry = qualities::load_paths(&args).expect("qualities load");
    assert_eq!(loaded_registry.len(), 87);
    for (id, name) in &recorded_registry {
        let Some(quality) = loaded_registry.get(id) else {
            panic!("{id} did not load");
        };
        assert_eq!(&quality.name, name, "{id} is named in the reference");
    }

    let mut compared = 0_usize;
    let mut wrong = Vec::new();
    for kind in items::load_paths(&args).expect("items load") {
        match recorded_items.get(&kind.id) {
            Some(expected) if kind.qualities != *expected => wrong.push(format!(
                "{}: {:?}, the reference says {expected:?}",
                kind.id, kind.qualities
            )),
            Some(_) => compared += 1,
            None if !kind.qualities.is_empty() => {
                wrong.push(format!(
                    "{}: has qualities the reference does not give it",
                    kind.id
                ));
            }
            None => {}
        }
    }
    assert!(wrong.is_empty(), "{} differences: {wrong:#?}", wrong.len());
    assert_eq!(compared, 1_091, "every item with a quality was compared");
}
