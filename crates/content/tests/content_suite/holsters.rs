//! Every `holster` use action: 76 of them, two messages each, and one behavioural rule.
//!
//! `corpus/items/holsters.json` is `--dump-holsters`. The actor carries no storage data at all — the pocket stores — so the gate is short, and the rule worth testing
//! is the one line in `holster_actor::can_holster` (`src/iuse_actor.cpp:2911-2914`): **an active item cannot go into a holster even when it fits**.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::{Holster, UseAction, can_holster};
use cdda_json::Value;
use std::collections::BTreeMap;

/// Reads the recorded holsters, keyed by the item type that declares them.
fn dump() -> BTreeMap<String, Holster> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/holsters.json");
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
            panic!("a holster")
        };
        let text = |key: &str| match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is text: {other:?}"),
        };
        out.insert(
            text("id"),
            Holster {
                action: text("action"),
                holster_prompt: text("holster_prompt"),
                holster_msg: text("holster_msg"),
            },
        );
    }
    out
}

#[test]
fn every_holster_is_read_the_way_the_reference_reads_it() {
    let recorded = dump();
    assert_eq!(recorded.len(), 76, "every holster was recorded");
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, ItemType> = load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    // Both halves of the data are exercised: 45 state their own messages, and the rest leave them empty.
    let stated = recorded
        .values()
        .filter(|holster| !holster.holster_prompt.is_empty())
        .count();
    let silent = recorded.len() - stated;

    let mut differing: Vec<String> = Vec::new();
    for (id, expected) in &recorded {
        let Some(item) = loaded.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let found: Vec<&Holster> = item
            .use_actions
            .values()
            .filter_map(|action| match action {
                UseAction::Holster(holster) => Some(holster),
                _ => None,
            })
            .collect();
        if found.first().is_none_or(|holster| *holster != expected) {
            differing.push(format!(
                "{id}: the port says {found:?}, the reference says {expected:?}"
            ));
        }
    }
    assert!(stated > 20, "a stated prompt has to be exercised: {stated}");
    assert!(silent > 20, "and so does one left empty: {silent}");
    assert!(
        differing.is_empty(),
        "{} of {} holsters disagree; the first ten are {:#?}",
        differing.len(),
        recorded.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

#[test]
fn an_active_item_cannot_be_holstered_even_when_it_fits() {
    // The extra rule the holster adds on top of the pocket check, in all four combinations. The one that matters is the third: a lit torch fits a holster and still
    // may not go in, which is why the pocket's own answer is not enough.
    assert!(can_holster(false, true), "fits and is not active");
    assert!(!can_holster(true, true), "fits but is active");
    assert!(!can_holster(false, false), "does not fit");
    assert!(!can_holster(true, false), "neither");
}
