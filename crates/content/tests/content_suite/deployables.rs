//! Two actors that put something into the world: `deploy_furn` (39) and `place_trap` (28).
//!
//! `corpus/items/deploy-furn.json` and `corpus/items/place-traps.json` are `--dump-deploy-furn` and `--dump-place-traps`. `place_trap` is the one with a shape worth
//! pinning: its **unburied** half reads the actor's own object and its **buried** half reads a nested `"bury"` object, and only when there is a `bury_question` to
//! ask (`src/iuse_actor.cpp:4174-4187`) — so the same keys mean different things at the two levels.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::{DeployFurn, PlaceTrap, TrapData, UseAction};
use cdda_json::Value;
use std::collections::BTreeMap;

/// Reads a recorded JSON file's `types` array.
fn recorded(name: &str) -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items")
        .join(name);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    types.clone()
}

/// The loaded items, by id.
fn loaded() -> BTreeMap<String, ItemType> {
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

fn text(object: &cdda_json::Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key} is text: {other:?}"),
    }
}

fn whole(object: &cdda_json::Object, key: &str) -> i32 {
    match object.get(key) {
        Some(Value::Int(value)) => i32::try_from(*value).expect("a small number"),
        other => panic!("{key} is a whole number: {other:?}"),
    }
}

fn flag(object: &cdda_json::Object, key: &str) -> bool {
    matches!(object.get(key), Some(Value::Bool(true)))
}

fn trap_data(object: &cdda_json::Object, key: &str) -> TrapData {
    let Some(Value::Object(half)) = object.get(key) else {
        panic!("{key} is an object")
    };
    TrapData {
        trap: text(half, "trap"),
        done_message: text(half, "done_message"),
        practice: whole(half, "practice"),
        moves: whole(half, "moves"),
    }
}

#[test]
fn every_deployable_furniture_is_read_the_way_the_reference_reads_it() {
    let recorded = recorded("deploy-furn.json");
    assert_eq!(recorded.len(), 39, "every deploy_furn was recorded");
    let loaded = loaded();
    let mut differing: Vec<String> = Vec::new();
    for entry in &recorded {
        let Value::Object(object) = entry else {
            panic!("a deploy_furn")
        };
        let id = text(object, "id");
        let expected = DeployFurn {
            action: text(object, "action"),
            furn_type: text(object, "furn_type"),
        };
        let Some(item) = loaded.get(&id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let found: Vec<&DeployFurn> = item
            .use_actions
            .values()
            .filter_map(|action| match action {
                UseAction::DeployFurn(deploy) => Some(deploy),
                _ => None,
            })
            .collect();
        if found.first().is_none_or(|deploy| **deploy != expected) {
            differing.push(format!(
                "{id}: the port says {found:?}, the reference says {expected:?}"
            ));
        }
    }
    assert!(
        differing.is_empty(),
        "{} of 39 deployables disagree; they are {differing:#?}",
        differing.len()
    );
}

#[test]
fn every_trap_is_read_the_way_the_reference_reads_it() {
    let recorded = recorded("place-traps.json");
    assert_eq!(recorded.len(), 28, "every place_trap was recorded");
    let loaded = loaded();
    let mut differing: Vec<String> = Vec::new();
    let mut buryable = 0;
    let mut conditional = 0;
    let mut layered = 0;
    for entry in &recorded {
        let Value::Object(object) = entry else {
            panic!("a place_trap")
        };
        let id = text(object, "id");
        let expected = PlaceTrap {
            action: text(object, "action"),
            allow_underwater: flag(object, "allow_underwater"),
            allow_under_player: flag(object, "allow_under_player"),
            needs_solid_neighbor: flag(object, "needs_solid_neighbor"),
            needs_neighbor_terrain: text(object, "needs_neighbor_terrain"),
            outer_layer_trap: text(object, "outer_layer_trap"),
            bury_question: text(object, "bury_question"),
            unburied: trap_data(object, "unburied"),
            buried: trap_data(object, "buried"),
        };
        if !expected.bury_question.is_empty() {
            buryable += 1;
        }
        if expected.allow_underwater || expected.allow_under_player || expected.needs_solid_neighbor {
            conditional += 1;
        }
        if expected.outer_layer_trap != "tr_null" || expected.needs_neighbor_terrain != "t_null" {
            layered += 1;
        }
        let Some(item) = loaded.get(&id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let found: Vec<&PlaceTrap> = item
            .use_actions
            .values()
            .filter_map(|action| match action {
                UseAction::PlaceTrap(place) => Some(place),
                _ => None,
            })
            .collect();
        if found.first().is_none_or(|place| **place != expected) {
            differing.push(format!(
                "{id}: the port says {found:?}, the reference says {expected:?}"
            ));
        }
    }
    assert_eq!(buryable, 2, "the content has two buryable traps");
    assert!(conditional > 0, "a condition on placement has to be exercised: {conditional}");
    assert!(layered > 0, "and a trap that needs a neighbour or an outer layer: {layered}");
    assert!(
        differing.is_empty(),
        "{} of 28 traps disagree; they are {differing:#?}",
        differing.len()
    );
}
