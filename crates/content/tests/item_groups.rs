//! Every item group the reference defines.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::item_groups;
use cdda_json::parse;
use std::path::Path;

fn dump() -> (Vec<String>, usize) {
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
    let mut sorted = groups.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), groups.len(), "no duplicate group ids");
    assert!(groups.iter().any(|id| id == "10mm_loose"), "a known group");
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
