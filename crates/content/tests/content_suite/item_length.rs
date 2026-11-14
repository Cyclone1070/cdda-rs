//! Every item type's length, replayed against the reference's own answers.
//!
//! `corpus/items/measures.json` now carries `length`: the reference constructs an item of each of its 9,912 types and asks `item::length` (`src/item.cpp:7343-7398`). The
//! rule has three parts that a loader cannot reproduce by copying numbers:
//!
//! * a liquid or a gas has **no** length, and neither does a soft thing with nothing inside it;
//! * an explicit `longest_side` in the data wins — a 235 ml rod that states 750 mm is telling you something the cube root gets wrong;
//! * otherwise the length is the cube root of the **effective** volume, and for a thing counted by charges that is the volume of one charge, not of the stack.
//!
//! **What is declared, not hidden.** Guns and corpses compute their length from their mods and their monster respectively, and both are their own pieces of work; the test
//! names every type that differs, so a new difference cannot hide among the declared ones.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::Value;
use std::collections::BTreeMap;

fn arr<'a>(root: &'a Value, key: &str) -> &'a Vec<Value> {
    let Value::Object(object) = root else {
        panic!("an object");
    };
    let Some(Value::Array(entries)) = object.get(key) else {
        panic!("the dump holds {key}");
    };
    entries
}

fn text(entry: &Value, key: &str) -> String {
    let Value::Object(object) = entry else {
        panic!("an object");
    };
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key} is a string: {other:?}"),
    }
}

fn whole(entry: &Value, key: &str) -> i64 {
    let Value::Object(object) = entry else {
        panic!("an object");
    };
    match object.get(key) {
        Some(Value::Int(value)) => *value,
        other => panic!("{key} is a whole number: {other:?}"),
    }
}

fn dump() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/measures.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

#[test]
fn the_port_gives_every_type_the_length_the_reference_gives_it() {
    let dump = dump();
    let recorded: BTreeMap<String, i64> = arr(&dump, "types")
        .iter()
        .map(|entry| (text(entry, "id"), whole(entry, "length")))
        .collect();
    assert_eq!(recorded.len(), 9_912, "every type was measured");

    let materials: BTreeMap<String, cdda_content::materials::Material> =
        crate::corpus::materials_loaded()
            .into_iter()
            .map(|material| (material.id.clone(), material))
            .collect();
    let types = crate::corpus::items_loaded();
    assert_eq!(types.len(), recorded.len(), "the loader holds every type");

    let mut differing: Vec<String> = Vec::new();
    let mut guns = 0;
    let mut non_zero = 0;
    for item in &types {
        let Some(expected) = recorded.get(&item.id) else {
            continue;
        };
        let got = item.longest_side(&materials);
        if item.is_gun {
            guns += 1;
            continue;
        }
        if got != *expected {
            differing.push(item.id.clone());
        }
        if *expected != 0 {
            non_zero += 1;
        }
    }
    // **One type is declared, and it is not a length rule at all.** The reference constructed each item with the contents its prototype's own default gives it, and
    // `belt40mm` is a magazine that comes with 25 grenades already in it: its recorded length of 112 mm is one of *their* lengths, not its own
    // (`contents.item_length_modifier()`, `src/item.cpp:7395-7397`). The port measures the type alone, so this is a gap in the item-creation default, not in the rule.
    assert_eq!(
        differing,
        vec!["belt40mm".to_owned()],
        "every type but the one with default contents agrees about its length"
    );
    assert!(
        guns > 300,
        "the guns have to be counted, not quietly skipped: there were {guns}"
    );
    assert!(
        non_zero > 5_000,
        "the corpus has to contain lengths that are not zero, or the rule is untested: it had {non_zero}"
    );
}
