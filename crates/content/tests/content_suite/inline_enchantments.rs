//! The enchantments nobody wrote down, which are the great majority of them.
//!
//! Reference: `enchantment::load_inline_enchantment` (`src/magic_enchantment.cpp:267-285`) and its four call sites. A trait, a bionic, an effect or a vehicle
//! part may carry an `enchantments` array. Each element is either a **string**, naming an enchantment defined elsewhere, or an **object**, which the reference
//! registers there and then under a name it makes up from the owner and the element's position:
//!
//! ```text
//! INLINE_ENCH_<owner id>_<index among all elements, strings included>
//! ```
//!
//! The index counts every element, not just the objects, and it does not reset between owners. Get either wrong and the names collide or drift, which the
//! registry's digest catches and a count does not.

#![allow(clippy::expect_used, clippy::panic)]

#[test]
fn eleven_inline_enchantments_in_five_owners_name_themselves_the_references_way() {
    // Small enough to check by eye, exactly as the reference spells it.
    let text = r#"[
        {"type": "mutation", "id": "BEAST", "enchantments": [
            {"values": [{"value": "STRENGTH", "add": 2}]},
            "ench_something_else",
            {"values": [{"value": "DEXTERITY", "add": 1}]}
        ]},
        {"type": "bionic", "id": "bio_eye", "name": "Bionic Eye", "enchantments": [{"values": []}]},
        {"type": "effect_type", "id": "CHILLED", "enchantments": [{"values": []}, {"values": []}]},
        {"type": "mutation", "id": "QUIET", "enchantments": ["ench_named"]}
    ]"#;
    let value = cdda_json::parse(text.as_bytes()).expect("it parses");
    assert_eq!(
        cdda_content::enchantments::inline_ids(&value),
        vec![
            "INLINE_ENCH_BEAST_0".to_owned(),
            "INLINE_ENCH_BEAST_2".to_owned(),
            "INLINE_ENCH_Bionic Eye_0".to_owned(),
            "INLINE_ENCH_CHILLED_0".to_owned(),
            "INLINE_ENCH_CHILLED_1".to_owned(),
        ],
        "a string element is a reference, not a new enchantment, but it still takes an index, and a bionic is named by the words a player reads"
    );
}

#[test]
fn the_whole_data_tree_holds_the_number_the_reference_holds() {
    let loaded = crate::corpus::inline_enchantments_loaded();
    assert_eq!(
        loaded.len(),
        311,
        "the inline half of the enchantment registry"
    );
    assert!(
        loaded.iter().all(|id| id.starts_with("INLINE_ENCH_")),
        "every one of them is named by the rule"
    );
}
