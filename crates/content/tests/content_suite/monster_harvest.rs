//! Which harvest a monster's corpse belongs to, checked against the reference's own dump.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::harvest::load_harvests;
use cdda_content::monsters;
use cdda_json::{Object, Value, parse};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn loaded() -> Vec<monsters::MonsterType> {
    let data = root().join("references/Cataclysm-DDA/data");
    monsters::load_paths(&[&data.join("core"), &data.join("json")]).expect("monsters load")
}

fn dump() -> Vec<Object> {
    let bytes = std::fs::read(root().join("corpus/prototypes/monsters.json")).expect("the dump");
    let value = parse(&bytes).expect("valid json");
    let Value::Object(root_object) = value else {
        panic!("the dump is an object");
    };
    let Some(Value::Array(entries)) = root_object.get("prototypes") else {
        panic!("the dump holds prototypes");
    };
    entries
        .iter()
        .filter_map(|entry| match entry {
            Value::Object(object) => Some(object.clone()),
            _ => None,
        })
        .collect()
}

fn text(object: &Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        _ => String::new(),
    }
}

#[test]
fn every_monster_carries_the_harvest_the_reference_names() {
    // The dump gained `harvest`, `dissect` and `size` this round, so this compares the loader against the
    // reference's own answer for all 1,154 kinds instead of against an assumption.
    let ours = loaded();
    let theirs = dump();
    assert_eq!(ours.len(), theirs.len());
    let mut mismatches = Vec::new();
    for entry in &theirs {
        let id = text(entry, "id");
        let Some(monster) = ours.iter().find(|monster| monster.id == id) else {
            mismatches.push(format!("{id}: missing"));
            continue;
        };
        if monster.harvest != text(entry, "harvest") {
            mismatches.push(format!(
                "{id}: harvest {} vs {}",
                monster.harvest,
                text(entry, "harvest")
            ));
        }
        if monster.dissect != text(entry, "dissect") {
            mismatches.push(format!(
                "{id}: dissect {} vs {}",
                monster.dissect,
                text(entry, "dissect")
            ));
        }
        if monster.size != text(entry, "size") {
            mismatches.push(format!(
                "{id}: size {} vs {}",
                monster.size,
                text(entry, "size")
            ));
        }
    }
    for line in mismatches.iter().take(20) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

#[test]
fn a_plain_zombie_is_meat_bone_and_guts_and_a_medium_body() {
    let ours = loaded();
    let zombie = ours
        .iter()
        .find(|monster| monster.id == "mon_zombie")
        .expect("the plain zombie");
    assert_eq!(zombie.harvest, "zombie_humanoid");
    assert_eq!(zombie.size, "medium");
}

#[test]
fn every_harvest_a_monster_names_is_a_definition_that_exists() {
    let ours = loaded();
    let files =
        cdda_content::json_files_in_load_order(&root().join("references/Cataclysm-DDA/data/json"))
            .expect("the data directory");
    let mut harvests = BTreeSet::new();
    for path in &files {
        if let Some(value) = cdda_content::read_json(path) {
            harvests.extend(load_harvests(&value).into_keys());
        }
    }
    let missing: Vec<&str> = ours
        .iter()
        .map(|monster| monster.harvest.as_str())
        .filter(|id| !harvests.contains(*id))
        .collect();
    assert!(
        missing.is_empty(),
        "harvest ids with no definition: {missing:?}"
    );
}

#[test]
fn a_dissection_that_is_not_defined_is_named_rather_than_guessed() {
    // The reference leaves `dissect` empty for most kinds; the port keeps it empty rather than borrowing the
    // butchery list, because dissecting a dog is not butchering a dog.
    let ours = loaded();
    let empty = ours
        .iter()
        .filter(|monster| monster.dissect.is_empty())
        .count();
    assert!(empty > 0, "some monsters have no dissection of their own");
    let files =
        cdda_content::json_files_in_load_order(&root().join("references/Cataclysm-DDA/data/json"))
            .expect("the data directory");
    let mut harvests = BTreeSet::new();
    for path in &files {
        if let Some(value) = cdda_content::read_json(path) {
            harvests.extend(load_harvests(&value).into_keys());
        }
    }
    let unknown: Vec<&str> = ours
        .iter()
        .map(|monster| monster.dissect.as_str())
        .filter(|id| !id.is_empty() && !harvests.contains(*id))
        .collect();
    assert!(
        unknown.is_empty(),
        "dissection ids with no definition: {unknown:?}"
    );
}

#[test]
fn the_sizes_are_the_five_the_reference_uses() {
    let ours = loaded();
    let sizes: BTreeSet<&str> = ours.iter().map(|monster| monster.size.as_str()).collect();
    assert_eq!(
        sizes,
        BTreeSet::from(["huge", "large", "medium", "small", "tiny"])
    );
}
