//! Harvest definitions: what a monster's corpse yields.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::harvest::{Part, load_harvests};
use std::path::Path;

fn one(text: &str) -> cdda_content::harvest::HarvestDefinition {
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let mut loaded = load_harvests(&value);
    assert_eq!(loaded.len(), 1, "one definition");
    loaded.pop_first().expect("a definition").1
}

#[test]
fn a_count_entry_rolls_between_two_numbers() {
    let harvest = one(
        r#"[{"type":"harvest","id":"seeds","entries":[{"drop":"withered","base_num":[1,2]}]}]"#,
    );
    let entry = harvest.entries.first().expect("an entry");
    assert_eq!(entry.drop, "withered");
    assert_eq!(entry.base_num, Some((1, 2)));
    assert_eq!(entry.mass_ratio, None);
    assert_eq!(entry.part, None, "a count of things has no body part");
}

#[test]
fn a_bulk_entry_is_a_share_of_the_corpse() {
    let harvest = one(
        r#"[{"type":"harvest","id":"fish","entries":[{"drop":"fish","type":"flesh","mass_ratio":0.32}]}]"#,
    );
    let entry = harvest.entries.first().expect("an entry");
    assert_eq!(entry.mass_ratio, Some(0.32));
    assert_eq!(entry.part, Some(Part::Flesh));
    assert_eq!(entry.base_num, None, "bulk is not counted in items");
}

#[test]
fn a_single_number_is_a_pair_of_itself() {
    let harvest =
        one(r#"[{"type":"harvest","id":"small","entries":[{"drop":"bone","base_num":3}]}]"#);
    assert_eq!(
        harvest.entries.first().expect("an entry").base_num,
        Some((3, 3))
    );
}

#[test]
fn every_part_the_reference_names_is_recognised() {
    for (name, part) in [
        ("flesh", Part::Flesh),
        ("offal", Part::Offal),
        ("skin", Part::Skin),
        ("bone", Part::Bone),
        ("blood", Part::Blood),
        ("something_new", Part::Other),
    ] {
        assert_eq!(Part::from_name(name), part, "{name}");
    }
}

#[test]
fn a_definition_that_is_not_a_harvest_is_skipped() {
    let value =
        cdda_json::parse(br#"[{"type":"MONSTER","id":"zombie","entries":[{"drop":"meat"}]}]"#)
            .expect("valid json");
    assert!(load_harvests(&value).is_empty());
}

#[test]
fn the_reference_tree_holds_the_definitions_it_says_it_does() {
    // 247 harvest definitions in data/json, one of which is the empty `null` harvest.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files =
        cdda_content::json_files_in_load_order(&root.join("references/Cataclysm-DDA/data/json"))
            .expect("the data directory");
    let mut all = std::collections::BTreeMap::new();
    for path in &files {
        if let Some(value) = cdda_content::read_json(path) {
            all.extend(load_harvests(&value));
        }
    }
    assert!(all.len() >= 247, "harvest definitions: {}", all.len());
    let null = all.get("null").expect("the empty harvest exists");
    assert!(null.entries.is_empty(), "and yields nothing");
    let spurge = all.get("spurge_harv").expect("a plant harvest exists");
    assert_eq!(spurge.entries.len(), 3);
    assert_eq!(
        spurge.entries.first().map(|entry| entry.drop.as_str()),
        Some("withered")
    );
    let fish = all.get("fish_tiny").expect("a fish harvest exists");
    assert_eq!(
        fish.entries.first().and_then(|entry| entry.mass_ratio),
        Some(0.32)
    );
}
