//! Every `transform` use action, resolved: what an item turns into and what that takes.
//!
//! `corpus/items/transforms.json` is `--dump-transforms`: 459 actions over 459 types, 431 `transform` and 28 `delayed_transform`. `delayed_transform` is not a
//! different shape — the reference loads it with the **same** loader and adds a `not_ready_msg` — so the port reads both through one reader.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::{Transform, UseAction};
use cdda_json::Value;
use std::collections::BTreeMap;

/// Reads the recorded transforms, keyed by the item type that declares them.
fn dump() -> BTreeMap<String, Transform> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/transforms.json");
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
        {
            let Value::Object(entry) = entry else {
                panic!("a transform")
            };
            let text = |key: &str| match entry.get(key) {
                Some(Value::Str(value)) => value.clone(),
                other => panic!("{key} is text: {other:?}"),
            };
            let flag = |key: &str| matches!(entry.get(key), Some(Value::Bool(true)));
            let whole = |key: &str| match entry.get(key) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("a small number"),
                other => panic!("{key} is a whole number: {other:?}"),
            };
            let charges = match entry.get("rand_target_charges") {
                Some(Value::Array(entries)) => entries
                    .iter()
                    .map(|charge| match charge {
                        Value::Int(charge) => i32::try_from(*charge).expect("a small number"),
                        other => panic!("a charge: {other:?}"),
                    })
                    .collect(),
                other => panic!("rand_target_charges is a list: {other:?}"),
            };
            let transform = Transform {
                action: text("action"),
                cost: whole("cost"),
                target: text("target"),
                target_group: text("target_group"),
                container: text("container"),
                variant_type: text("variant_type"),
                sealed: flag("sealed"),
                target_charges: whole("target_charges"),
                target_ammo: text("target_ammo"),
                target_timer_seconds: i64::from(whole("target_timer_seconds")),
                active: flag("active"),
                need_worn: flag("need_worn"),
                need_wielding: flag("need_wielding"),
                need_empty: flag("need_empty"),
                moves: whole("moves"),
                need_fire: whole("need_fire"),
                need_charges: whole("need_charges"),
                rand_target_charges: charges,
            };
            out.insert(text("id"), transform);
        }
    }
    out
}

#[test]
fn every_transform_an_item_declares_is_read_the_way_the_reference_reads_it() {
    let recorded = dump();
    assert_eq!(recorded.len(), 459, "every transform was recorded");
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, ItemType> = load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    // The two kinds, and the fields the corpus actually exercises.
    let mut delayed = 0;
    let mut with_timer = 0;
    let mut with_container = 0;
    let mut with_conditions = 0;
    let mut differing: Vec<String> = Vec::new();
    for (id, case) in &recorded {
        // **Per type, not "somewhere in the content".** The recording is keyed by the item type, so a transform that landed on the wrong type is a disagreement.
        let Some(item) = loaded.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let found = item.use_actions.iter().any(|action| match action {
            UseAction::Transform(transform) => transform == case,
            UseAction::ConsumeDrug(_) | UseAction::LinkUp(_) | UseAction::Other { .. } => false,
        });
        if !found {
            differing.push(format!(
                "{id}: the port's list is {:?}, the reference's transform is {case:?}",
                item.use_actions
            ));
        }
        if case.action == "delayed_transform" {
            delayed += 1;
        }
        if case.target_timer_seconds > 0 {
            with_timer += 1;
        }
        if !case.container.is_empty() {
            with_container += 1;
        }
        if case.need_charges > 0 || case.need_fire > 0 || case.need_worn || case.need_wielding {
            with_conditions += 1;
        }
    }
    assert!(delayed > 20, "delayed_transform has to be exercised: {delayed}");
    assert!(with_timer > 20, "and so does the timer: {with_timer}");
    assert!(with_container >= 4, "and the container: {with_container}");
    assert!(
        with_conditions > 20,
        "and the conditions that gate a transformation: {with_conditions}"
    );
    assert!(
        differing.is_empty(),
        "{} of {} transforms are missing; the first twenty are {:#?}",
        differing.len(),
        recorded.len(),
        differing.get(..20).unwrap_or(&differing)
    );
}
