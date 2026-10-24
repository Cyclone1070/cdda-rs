//! How hard everything is to break.
//!
//! Source: `--dump-bash-info` from the instrumented reference (`harness.cpp`), which walks every terrain and furniture prototype that carries a `bash` block and prints the
//! whole block as the reference holds it **after inheritance**.
//!
//! Half of the block is absence: `-1` is "not given", a tile whose block exists can still be unbashable, and `ter_set_bashed_from_above` defaults to `ter_set` rather than to
//! nothing. A gate that compared only the fields the data mentions would leave every default free to be invented.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::mapdata::{BashInfo, MapDataKind, load_paths};
use cdda_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/bash/info.json")
}

fn text(entry: &Value, key: &str) -> String {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is a string: {other:?}"),
        },
        other => panic!("an entry: {other:?}"),
    }
}

fn flag(entry: &Value, key: &str) -> bool {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Bool(value)) => *value,
            other => panic!("{key} is a bool: {other:?}"),
        },
        other => panic!("an entry: {other:?}"),
    }
}

fn number(entry: &Value, key: &str) -> i32 {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
            other => panic!("{key} is a number: {other:?}"),
        },
        other => panic!("an entry: {other:?}"),
    }
}

/// The reference's blocks, one map per kind.
fn recorded() -> (BTreeMap<String, BashInfo>, BTreeMap<String, BashInfo>) {
    let json = std::fs::read_to_string(corpus()).expect("the bash dump");
    let Value::Object(root) = cdda_json::parse(json.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let read = |key: &str, terrain: bool| -> BTreeMap<String, BashInfo> {
        let Some(Value::Array(list)) = root.get(key) else {
            panic!("{key}")
        };
        let mut out = BTreeMap::new();
        for entry in list {
            let group = text(entry, "items");
            out.insert(
                text(entry, "id"),
                BashInfo {
                    str_min: number(entry, "str_min"),
                    str_max: number(entry, "str_max"),
                    str_min_blocked: number(entry, "str_min_blocked"),
                    str_max_blocked: number(entry, "str_max_blocked"),
                    str_min_supported: number(entry, "str_min_supported"),
                    str_max_supported: number(entry, "str_max_supported"),
                    destroy_only: flag(entry, "destroy_only"),
                    bash_below: flag(entry, "bash_below"),
                    sound_vol: number(entry, "sound_vol"),
                    sound_fail_vol: number(entry, "sound_fail_vol"),
                    drop_group: if group.is_empty() { None } else { Some(group) },
                    set: text(entry, if terrain { "ter_set" } else { "furn_set" }),
                    set_from_above: if terrain {
                        Some(text(entry, "ter_set_bashed_from_above"))
                    } else {
                        None
                    },
                },
            );
        }
        out
    };
    (read("terrain", true), read("furniture", false))
}

#[test]
fn every_bash_block_loads_as_the_reference_holds_it() {
    let (terrain, furniture) = recorded();
    assert_eq!(terrain.len(), 958, "the reference has 958 bashable terrains");
    assert_eq!(furniture.len(), 612, "and 612 pieces of furniture");
    let dirs = crate::corpus::dirs();
    let args: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    let loaded = load_paths(&args).expect("the map data loads");
    let mut ported_terrain: BTreeMap<String, BashInfo> = BTreeMap::new();
    let mut ported_furniture: BTreeMap<String, BashInfo> = BTreeMap::new();
    for data in loaded {
        let Some(bash) = data.bash else { continue };
        match data.kind {
            MapDataKind::Terrain => ported_terrain.insert(data.id, bash),
            MapDataKind::Furniture => ported_furniture.insert(data.id, bash),
        };
    }
    let mut wrong = Vec::new();
    for (kind, ported, expected) in [
        ("terrain", &ported_terrain, &terrain),
        ("furniture", &ported_furniture, &furniture),
    ] {
        if ported.len() != expected.len() {
            wrong.push(format!("{kind}: {} blocks against {}", ported.len(), expected.len()));
        }
        for (id, want) in expected {
            match ported.get(id) {
                None => wrong.push(format!("{kind} {id}: no bash block")),
                Some(have) if have != want => {
                    wrong.push(format!("{kind} {id}: the port has {have:?}, the reference has {want:?}"));
                }
                Some(_) => {}
            }
        }
        for id in ported.keys() {
            if !expected.contains_key(id) {
                wrong.push(format!("{kind} {id}: a bash block the reference does not have"));
            }
        }
    }
    assert!(wrong.is_empty(), "{} wrong of 1,570:\n{}", wrong.len(), wrong.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}

/// The two defaults that are not zero, pinned on their own so that a port that got them wrong says which one.
#[test]
fn the_defaults_are_the_reference_s() {
    let (terrain, furniture) = recorded();
    // `ter_set_bashed_from_above` defaults to `ter_set` (`src/mapdata.cpp:423`), so the two agree unless the data says otherwise.
    let differ = terrain.values().filter(|bash| bash.set_from_above.as_deref() != Some(bash.set.as_str())).count();
    assert!(differ > 0, "some terrain bashes differently from above: {differ}");
    assert!(
        terrain.values().all(|bash| bash.set_from_above.is_some()),
        "every terrain block says what it becomes from above, whether the data names it or not"
    );
    // The loader's other default is `-1`, and the data exercises it on the fields it does not care about rather than on `str_min`: every block in the set names a strength,
    // while most say nothing about how loud failing is.
    let unnamed_failure = terrain
        .values()
        .chain(furniture.values())
        .filter(|bash| bash.sound_fail_vol == -1)
        .count();
    let named_failure = terrain
        .values()
        .chain(furniture.values())
        .filter(|bash| bash.sound_fail_vol >= 0)
        .count();
    assert!(
        unnamed_failure > 500 && named_failure > 0,
        "both sides of the loader's -1 default are exercised: {named_failure} named, {unnamed_failure} not"
    );
    assert!(
        terrain.values().all(|bash| bash.str_min >= 0),
        "every block in this set names a strength: -1 is the loader's default and would mean unbashable"
    );
    assert!(
        furniture.values().all(|bash| bash.set_from_above.is_none()),
        "furniture has no from-above field"
    );
}

#[test]
fn the_item_groups_are_names_and_the_rest_are_anonymous() {
    let (terrain, furniture) = recorded();
    let named = terrain.values().chain(furniture.values()).filter(|bash| bash.drop_group.is_some()).count();
    let anonymous = terrain
        .values()
        .chain(furniture.values())
        .filter(|bash| bash.drop_group.is_none())
        .count();
    assert!(named > 100, "the data names most of its drop groups: {named}");
    assert!(
        anonymous > 1_000,
        "and writes the rest inline, which the reference turns into an anonymous group: {anonymous}"
    );
    // A fresh block that names no items is **not** a block with no group: the reference gives it `EMPTY_GROUP`.
    let empty = terrain.values().chain(furniture.values()).filter(|bash| bash.drop_group.as_deref() == Some("EMPTY_GROUP")).count();
    assert!(empty > 0, "the default group is a name, not an absence: {empty}");
}
