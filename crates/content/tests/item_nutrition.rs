//! What the reference says about eating, read from its own dump.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::items::nutrition_from_dump;
use cdda_json::parse;

fn dump() -> std::collections::BTreeMap<String, (i32, i32, i32)> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let text = std::fs::read_to_string(path).expect("the item dump");
    let value = parse(text.as_bytes()).expect("valid json");
    nutrition_from_dump(&value)
}

#[test]
fn every_item_is_in_the_table() {
    assert_eq!(dump().len(), 9_912);
}

#[test]
fn food_carries_its_nutrition_and_water_does_not() {
    let all = dump();
    assert_eq!(
        all.get("meat"),
        Some(&(46, 0, 1)),
        "meat: 46 nutr, no quench (it is not a drink), one charge a bite"
    );
    assert_eq!(
        all.get("water_clean").map(|entry| entry.0),
        Some(0),
        "water has no nutrition"
    );
    assert_eq!(all.get("apple").map(|entry| entry.0), Some(10));
}

#[test]
fn a_thousand_or_more_things_are_food() {
    let all = dump();
    let food = all.values().filter(|entry| entry.0 > 0).count();
    assert_eq!(food, 1_315, "items the reference gives nutrition to");
}
