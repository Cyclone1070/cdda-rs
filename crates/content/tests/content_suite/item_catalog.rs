//! Names to numbers, and the harvest drops that need them.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::catalog::ItemCatalog;
use cdda_content::harvest::load_harvests;
use cdda_content::items;
use cdda_json::{Value, parse};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn items_loaded() -> Vec<items::ItemType> {
    let _data = root().join("references/Cataclysm-DDA/data");
    crate::corpus::items_loaded()
}

fn dump_ids() -> BTreeSet<String> {
    let bytes = std::fs::read(root().join("corpus/prototypes/items.json")).expect("the dump");
    let value = parse(&bytes).expect("valid json");
    let Value::Object(root_object) = value else {
        panic!("the dump is an object");
    };
    let Some(Value::Array(prototypes)) = root_object.get("prototypes") else {
        panic!("the dump holds prototypes");
    };
    prototypes
        .iter()
        .filter_map(|entry| match entry {
            Value::Object(object) => match object.get("id") {
                Some(Value::Str(id)) => Some(id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn the_catalogue_numbers_every_item_the_reference_has() {
    let catalog = ItemCatalog::new(&items_loaded());
    assert_eq!(catalog.len(), 9_912, "every item in the reference");
    assert_eq!(
        catalog.len(),
        dump_ids().len(),
        "and the same ones the dump lists"
    );
}

#[test]
fn a_name_and_a_number_are_the_same_thing_seen_from_two_sides() {
    let catalog = ItemCatalog::new(&items_loaded());
    let meat = catalog.id_of("meat").expect("meat exists");
    assert_eq!(catalog.name_of(meat), Some("meat"));
    assert!(catalog.id_of("not_an_item_at_all").is_none());
    assert!(
        catalog.name_of(u32::MAX).is_none(),
        "a number the catalog did not make"
    );
}

#[test]
fn every_harvest_in_the_reference_names_drops_that_exist() {
    // Across the whole tree: 331 distinct drop ids. Every one of them is an item the content really has - if any
    // did not resolve, a butchery would silently drop nothing where the reference drops meat.
    let catalog = ItemCatalog::new(&items_loaded());
    let files =
        cdda_content::json_files_in_load_order(&root().join("references/Cataclysm-DDA/data/json"))
            .expect("the data directory");
    let mut drops = BTreeSet::new();
    for path in &files {
        if let Some(value) = cdda_content::read_json(path) {
            for definition in load_harvests(&value).into_values() {
                for entry in definition.entries {
                    drops.insert(entry.drop);
                }
            }
        }
    }
    assert_eq!(
        drops.len(),
        331,
        "distinct harvest drops across the reference tree"
    );
    // A drop is either an item or an **item group**, which is why the reference branches on
    // `entry.type->is_itype()` before making anything: 221 of the 331 are items and 110 are groups.
    let as_items = drops
        .iter()
        .filter(|drop| catalog.id_of(drop).is_some())
        .count();
    assert_eq!(as_items, 221, "drops that name an item directly");
    let groups: BTreeSet<String> = drops
        .iter()
        .filter(|drop| catalog.id_of(drop).is_none())
        .cloned()
        .collect();
    assert_eq!(groups.len(), 110, "and the rest have to be item groups");
    // Every one of those really is a group the content defines, so nothing here is a typo in the data.
    let files =
        cdda_content::json_files_in_load_order(&root().join("references/Cataclysm-DDA/data/json"))
            .expect("the data directory");
    let mut defined = BTreeSet::new();
    for path in &files {
        if let Some(value) = cdda_content::read_json(path) {
            let cdda_json::Value::Array(entries) = &*value else {
                continue;
            };
            for entry in entries {
                let cdda_json::Value::Object(object) = entry else {
                    continue;
                };
                if let Some(cdda_json::Value::Str(kind)) = object.get("type")
                    && kind == "item_group"
                    && let Some(cdda_json::Value::Str(id)) = object.get("id")
                {
                    defined.insert(id.clone());
                }
            }
        }
    }
    let unknown: Vec<&String> = groups.iter().filter(|id| !defined.contains(*id)).collect();
    assert!(
        unknown.is_empty(),
        "drops that are neither item nor group: {unknown:?}"
    );
}

#[test]
fn the_catalogue_refuses_to_invent_a_name_for_a_number_it_did_not_make() {
    let catalog = ItemCatalog::default();
    assert!(catalog.is_empty());
    assert!(catalog.id_of("meat").is_none());
    let one = parse(br#"[{"id":"meat"}]"#).expect("valid json");
    let Value::Array(_) = one else {
        panic!("an array");
    };
    // A catalog of nothing knows nothing, which is the property that keeps a stale table from answering.
    assert!(ItemCatalog::default().name_of(0).is_none());
}
