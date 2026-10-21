//! The ids the reference makes for itself.
//!
//! Two rules are checked here on small fixtures: how an inline enchantment is
//! named, and which items become turret parts. The corpus test
//! (`prototype_ids`) holds the same rules against the whole content.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::{generated, items};
use std::path::PathBuf;

fn fixture(text: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("content.json"), text).expect("fixture");
    dir
}

#[test]
fn an_inline_enchantment_is_named_after_its_parent_and_position() {
    let dir = fixture(
        r#"[
        { "type": "mutation", "id": "AMORPHOUS",
          "enchantments": [ "a_string_id", { "condition": "ALWAYS" }, { "condition": "ALWAYS" } ] },
        { "type": "bionic", "id": "bio_alarm", "name": { "str": "Alarm System" },
          "enchantments": [ { "condition": "ALWAYS" } ] },
        { "type": "enchantment", "id": "WRITTEN_DOWN" }
        ]"#,
        "cdda-generated-enchantments",
    );
    let ids = generated::enchantment_ids(&[&dir]).expect("ids");
    let listed: Vec<&str> = ids.iter().map(String::as_str).collect();
    // The leading string is not an inline object, but it still takes a position,
    // so the first object is named `_1`; the bionic names itself after the text
    // it shows rather than its id.
    assert_eq!(
        listed,
        vec![
            "INLINE_ENCH_AMORPHOUS_1",
            "INLINE_ENCH_AMORPHOUS_2",
            "INLINE_ENCH_Alarm System_0",
            "WRITTEN_DOWN"
        ]
    );
}

#[test]
fn every_mountable_gun_gets_a_turret_part() {
    let _dir = fixture(
        r#"[
        { "type": "GENERIC", "subtypes": [ "GUN" ], "id": "test_rifle", "name": "rifle", "symbol": "(", "color": "black",
          "weight": "3 kg", "volume": "2 L", "price": "100 USD", "material": [ "steel" ] },
        { "type": "GENERIC", "subtypes": [ "GUN" ], "id": "test_no_turret", "name": "gun", "symbol": "(", "color": "black",
          "weight": "3 kg", "volume": "2 L", "price": "100 USD", "material": [ "steel" ],
          "flags": [ "NO_TURRET" ] },
        { "type": "GENERIC", "subtypes": [ "GUN", "GUNMOD" ], "id": "test_scope", "name": "scope", "symbol": ":", "color": "black",
          "weight": "300 g", "volume": "250 ml", "price": "20 USD", "material": [ "steel" ] }
        ]"#,
        "cdda-generated-turrets",
    );
    // The fixture above is what this test is about, so it loads its own copy.
    let loaded: Vec<items::ItemType> =
        items::load_paths(&[&std::env::temp_dir().join("cdda-generated-turrets")])
            .expect("fixture loads");
    let parts = generated::turret_part_ids(&loaded);
    assert_eq!(parts.len(), 1);
    assert!(parts.contains("turret_test_rifle"));
}
