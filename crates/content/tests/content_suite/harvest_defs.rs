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

/// **A harvest that inherits gets its parent's entries and then its own, in that order.** Most of the content's harvest lists are written that way.
///
/// Reference: `harvest_definition` is a `generic_factory` entry, so `copy-from` is resolved at finalisation and `extend.entries` are appended after the inherited list. The port read only the literal
/// `entries` until a playtest butchered a zombie and got nothing: `zombie_humanoid` is a `copy-from: zombie_decay_bone_human_skull` with an `extend`, so its own list is empty and the parent's was never
/// consulted.
#[test]
fn an_inheriting_harvest_gets_its_parent_s_entries_and_then_its_own() {
    use cdda_content::harvest::{load_harvests, resolve_harvests};

    let text = r#"[
        {"id": "parent", "type": "harvest", "entries": [
            {"drop": "meat", "type": "flesh", "mass_ratio": 0.5}
        ]},
        {"id": "child", "type": "harvest", "copy-from": "parent", "extend": {"entries": [
            {"drop": "bone", "type": "bone", "mass_ratio": 0.1}
        ]}}
    ]"#;
    let value = cdda_json::parse(text.as_bytes()).expect("the fixture parses");
    let mut all = load_harvests(&value);
    assert_eq!(
        all.get("child").map(|definition| definition.entries.len()),
        Some(0),
        "**the child states no entries of its own**, which is what the reference's data looks like"
    );
    resolve_harvests(&mut all);
    let child = all.get("child").expect("the child");
    assert_eq!(child.entries.len(), 2, "the parent's entry and the child's, and no more");
    assert_eq!(child.entries.first().map(|entry| entry.drop.as_str()), Some("meat"), "the parent's comes first");
    assert_eq!(child.entries.last().map(|entry| entry.drop.as_str()), Some("bone"), "the child's comes after");
    let parent = all.get("parent").expect("the parent");
    assert_eq!(parent.entries.len(), 1, "the parent is untouched by its child");
}
