//! Every item group the reference defines.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::item_groups;
use cdda_json::parse;
use std::path::Path;

fn dump() -> (Vec<cdda_content::item_groups::Group>, usize) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/item_groups.json");
    let text = std::fs::read_to_string(path).expect("the item group dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    item_groups::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_item_group_the_reference_has() {
    let (groups, count) = dump();
    assert_eq!(count, 8_230, "the dump's own count");
    assert_eq!(groups.len(), count, "and the port reads exactly that many");
}

#[test]
fn group_ids_are_unique_and_a_known_one_is_present() {
    let (groups, _) = dump();
    let mut sorted: Vec<String> = groups.iter().map(|group| group.id.clone()).collect();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), groups.len(), "no duplicate group ids");
    assert!(
        groups.iter().any(|group| group.id == "10mm_loose"),
        "a known group"
    );
}

#[test]
fn a_broken_dump_is_rejected_rather_than_half_read() {
    // A loader that shrugged at a malformed dump would produce a smaller world and no error.
    let value = parse(b"{\"groups\":[\"a\"],\"count\":\"lots\"}").expect("valid JSON");
    assert!(
        item_groups::from_dump(&value).is_err(),
        "a non-integer count is refused"
    );
    let value = parse(b"{\"count\":1}").expect("valid JSON");
    assert!(
        item_groups::from_dump(&value).is_err(),
        "a missing array is refused"
    );
}

#[test]
fn a_group_carries_what_it_can_spawn() {
    let (groups, _) = dump();
    let with_items = groups.iter().filter(|g| !g.items.is_empty()).count();
    let entries: usize = groups.iter().map(|g| g.items.len()).sum();
    assert_eq!(with_items, 8_203, "groups with anything in them");
    assert_eq!(entries, 281_546, "spawn entries across all groups");
    assert_eq!(
        groups.len() - with_items,
        27,
        "and 27 that spawn nothing at all, which is legal - EMPTY_GROUP is one of them"
    );

    let ten_mm = groups
        .iter()
        .find(|g| g.id == "10mm_loose")
        .expect("exists");
    let fmj = ten_mm
        .items
        .iter()
        .find(|s| s.item == "10mm_fmj")
        .expect("fmj");
    assert_eq!((fmj.min, fmj.max), (1, 10), "a box of one to ten rounds");
}

#[test]
fn inverted_ranges_are_kept_exactly_as_the_reference_gives_them() {
    // The reference sums lower figures and upper figures separately across a distribution, so
    // its own aggregate can put the lower above the upper. 376 entries do. A loader that sorted
    // or clamped them would silently rewrite the content and change what a crate contains.
    let (groups, _) = dump();
    let inverted = groups
        .iter()
        .flat_map(|g| g.items.iter())
        .filter(|s| s.min > s.max)
        .count();
    assert_eq!(inverted, 376, "the reference's own inversions, preserved");
    let bleach = groups
        .iter()
        .find(|g| g.id == "LS_SUS_crate_tier0_supplies")
        .and_then(|g| g.items.iter().find(|s| s.item == "bleach"))
        .expect("the example exists");
    assert_eq!(
        (bleach.min, bleach.max),
        (6, 2),
        "6 above 2, exactly as dumped"
    );
}

/// **A group contains what it produces, however deep the tree.** Reference: `item_group::group_contains_item` (`item_group.cpp:1048-1055`) asks the spawn data `has_item`, which
/// recurses into the groups an entry names.
///
/// The test builds its own tree rather than trusting a data file: a collection holding one item and one nested group, a distribution holding the nested group again, and a group
/// that names **itself** - the cycle the reference would recurse forever on. The nesting is the point: an item two levels down is in the group, and the visited set is what keeps
/// a bad tree from hanging.
#[test]
fn a_group_contains_what_its_tree_produces() {
    let mut definitions = std::collections::BTreeMap::new();
    let item = |id: &str| cdda_content::item_group_draw::Entry::Item {
        id: id.to_owned(),
        probability: 100,
        count: (1, 1),
        event: None,
    };
    let group = |id: &str| cdda_content::item_group_draw::Entry::Group {
        id: id.to_owned(),
        probability: 100,
        event: None,
    };
    definitions.insert(
        "outer".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "outer".to_owned(),
            distribution: false,
            entries: vec![item("rock"), group("middle")],
        },
    );
    definitions.insert(
        "middle".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "middle".to_owned(),
            distribution: true,
            entries: vec![item("stick"), group("inner")],
        },
    );
    definitions.insert(
        "inner".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "inner".to_owned(),
            distribution: false,
            entries: vec![item("gem")],
        },
    );
    definitions.insert(
        "loop".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "loop".to_owned(),
            distribution: false,
            entries: vec![group("loop"), item("stone")],
        },
    );
    assert!(
        cdda_content::item_group_draw::contains(&definitions, "outer", "rock"),
        "**a direct item is in**"
    );
    assert!(
        cdda_content::item_group_draw::contains(&definitions, "outer", "stick"),
        "**one level down is in**"
    );
    assert!(
        cdda_content::item_group_draw::contains(&definitions, "outer", "gem"),
        "**two levels down is in**"
    );
    assert!(
        !cdda_content::item_group_draw::contains(&definitions, "outer", "gemstone"),
        "**and a name nobody produces is not**"
    );
    assert!(
        cdda_content::item_group_draw::contains(&definitions, "loop", "stone"),
        "**a group that names itself still answers for its other entries**"
    );
    assert!(
        !cdda_content::item_group_draw::contains(&definitions, "outer", "unheard_of"),
        "**and a group the tree does not name contains nothing**"
    );
}
