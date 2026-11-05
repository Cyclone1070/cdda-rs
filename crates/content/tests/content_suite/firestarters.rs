//! Every `firestarter` use action: 42 of them, three fields each, and one derived default.
//!
//! `corpus/items/firestarters.json` is `--dump-firestarters`, from `firestarter_actor` (`src/iuse_actor.h:481-511`) and its loader (`src/iuse_actor.cpp:1460-1465`).

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::{Firestarter, UseAction};
use cdda_json::Value;
use std::collections::BTreeMap;

/// Reads the recorded firestarters, keyed by the item type that declares them.
fn dump() -> BTreeMap<String, Firestarter> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/firestarters.json");
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
        let Value::Object(object) = entry else {
            panic!("a firestarter")
        };
        let text = |key: &str| match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is text: {other:?}"),
        };
        let whole = |key: &str| match object.get(key) {
            Some(Value::Int(value)) => i32::try_from(*value).expect("a small number"),
            other => panic!("{key} is a whole number: {other:?}"),
        };
        out.insert(
            text("id"),
            Firestarter {
                action: text("action"),
                moves: whole("moves"),
                moves_slow: whole("moves_slow"),
                need_sunlight: matches!(object.get("need_sunlight"), Some(Value::Bool(true))),
            },
        );
    }
    out
}

#[test]
fn every_firestarter_is_read_the_way_the_reference_reads_it() {
    let recorded = dump();
    assert_eq!(recorded.len(), 42, "every firestarter was recorded");
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, ItemType> = load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    // **The derived default has to be exercised, or the multiplication is untested**: a firestarter whose slow cost is exactly ten times its fast one and whose
    // fast cost is not the member's own 100 either got there by the fallback and not by the member's 1,000.
    let derived = recorded
        .values()
        .filter(|fire| fire.moves_slow == fire.moves.saturating_mul(10))
        .count();
    let stated = recorded
        .values()
        .filter(|fire| fire.moves_slow != fire.moves.saturating_mul(10))
        .count();
    let sunlight = recorded.values().filter(|fire| fire.need_sunlight).count();

    let mut differing: Vec<String> = Vec::new();
    for (id, expected) in &recorded {
        let Some(item) = loaded.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let found: Vec<&Firestarter> = item
            .use_actions
            .values()
            .filter_map(|action| match action {
                UseAction::Firestarter(fire) => Some(fire),
                _ => None,
            })
            .collect();
        if found.first().is_none_or(|fire| *fire != expected) {
            differing.push(format!(
                "{id}: the port says {found:?}, the reference says {expected:?}"
            ));
        }
    }
    assert!(derived > 20, "the derived default has to be exercised: {derived}");
    assert!(stated > 5, "and so does an actor that states its own slow cost: {stated}");
    assert!(sunlight > 5, "and the sunlight-only ones: {sunlight}");
    assert!(
        differing.is_empty(),
        "{} of {} firestarters disagree; the first ten are {:#?}",
        differing.len(),
        recorded.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

#[test]
fn a_missing_slow_cost_is_ten_times_the_fast_one_and_not_the_members_thousand() {
    // The rule in one line: `moves_slow` falls back to `moves * 10`, not to the member's 1,000 (`src/iuse_actor.cpp:1462-1463`). A firestarter that states
    // `"moves": 50` therefore has a slow cost of **500**, and one that states nothing at all has 100 and 1,000 — which is the same answer by luck, so the test that
    // matters is the one with a fast cost that is not 100.
    let object = cdda_json::parse(br#"{"type":"firestarter","moves":50}"#).expect("valid json");
    let Value::Object(object) = object else {
        panic!("an object")
    };
    let fire = Firestarter::read("firestarter", &object).expect("a firestarter");
    assert_eq!(fire.moves, 50);
    assert_eq!(fire.moves_slow, 500, "ten times the fast cost, not the member's 1,000");

    let object = cdda_json::parse(br#"{"type":"firestarter"}"#).expect("valid json");
    let Value::Object(object) = object else {
        panic!("an object")
    };
    let fire = Firestarter::read("firestarter", &object).expect("a firestarter");
    assert_eq!(fire.moves, 100, "the member's own fast default");
    assert_eq!(fire.moves_slow, 1_000, "and the same by luck");

    // And a stated slow cost wins, even when it is *less* than ten times the fast one: two matches-like tools say 40 and 1,000.
    let object =
        cdda_json::parse(br#"{"type":"firestarter","moves":40,"moves_slow":1000}"#).expect("valid json");
    let Value::Object(object) = object else {
        panic!("an object")
    };
    let fire = Firestarter::read("firestarter", &object).expect("a firestarter");
    assert_eq!(fire.moves_slow, 1_000);
}
