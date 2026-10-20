//! Item types against the reference's own resolve.
//!
//! The oracle is `corpus/prototypes/items.json`: 9,912 item types after the
//! reference loaded its files. This slice covers identity, names, glyph, colour,
//! weight, volume, price, materials, category and flags - the fields every other
//! item field is layered on.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::items;

/// How many fields this gate compares, for the receipt it prints.
const FIELDS: usize = 11;
use cdda_json::{Value, parse};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(text: &str, name: &str) -> Vec<items::ItemType> {
    let dir = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("items.json"), text).expect("fixture");
    items::load_paths(&[&dir]).expect("loads")
}

#[test]
fn a_name_object_gives_a_singular_and_a_plural() {
    let loaded = fixture(
        r#"[
        { "type": "GENERIC", "id": "test_knife", "name": { "str": "knife", "str_pl": "knives" },
          "symbol": "/", "color": "light_gray", "weight": "300 g", "volume": "250 ml",
          "price": "20 USD", "material": [ "steel" ] }
        ]"#,
        "cdda-item-fixture-names",
    );
    let knife = loaded
        .iter()
        .find(|item| item.id == "test_knife")
        .expect("prototype");
    assert_eq!(knife.name, "knife");
    assert_eq!(knife.name_plural, "knives");
    assert_eq!(knife.symbol, u32::from(b'/'));
    assert_eq!(knife.color_name(), "c_light_gray");
    // Weight is carried in milligrams, as the content states it; the dump
    // reports grams, so the comparison divides.
    assert_eq!(knife.weight, 300_000);
    assert_eq!(knife.volume, 250);
    assert_eq!(knife.price, 2_000);
    assert_eq!(knife.materials.get("steel"), Some(&1));
}

#[test]
fn a_weighted_material_and_a_plain_name_both_load() {
    let loaded = fixture(
        r#"[
        { "type": "GENERIC", "id": "test_mixed", "name": "mixed thing", "symbol": "x",
          "color": "white", "material": [ [ "iron", 3 ], "wood" ] }
        ]"#,
        "cdda-item-fixture-materials",
    );
    let item = loaded
        .iter()
        .find(|item| item.id == "test_mixed")
        .expect("prototype");
    assert_eq!(item.materials.get("iron"), Some(&3));
    assert_eq!(item.materials.get("wood"), Some(&1));
    // A name without a plural object gets the reference's default plural.
    assert_eq!(item.name_plural, "mixed things");
}

#[test]
fn every_item_type_matches_the_reference() {
    let started = std::time::Instant::now();
    let text =
        std::fs::read_to_string(root().join("corpus/prototypes/items.json")).expect("artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let Value::Object(root_object) = value else {
        panic!("root")
    };
    let Some(Value::Array(entries)) = root_object.get("prototypes") else {
        panic!("prototypes")
    };
    let loaded = items::load_paths(&[
        &root().join("references/Cataclysm-DDA/data/core"),
        &root().join("references/Cataclysm-DDA/data/json"),
    ])
    .expect("loads");
    println!(
        "loaded {} items, reference has {}",
        loaded.len(),
        entries.len()
    );
    let by_id: std::collections::BTreeMap<&str, &items::ItemType> =
        loaded.iter().map(|item| (item.id.as_str(), item)).collect();
    let mut mismatches = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        let text_of = |member: &str| match object.get(member) {
            Some(Value::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let id = text_of("id");
        let Some(ours) = by_id.get(id.as_str()) else {
            mismatches.push(format!("{id}: missing"));
            continue;
        };
        compare_item(&id, object, ours, &mut mismatches);
    }
    println!(
        "items: {} prototypes, {} fields, {} mismatches in {:.1}s",
        entries.len(),
        FIELDS,
        mismatches.len(),
        started.elapsed().as_secs_f64()
    );
    for line in mismatches.iter().take(20) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

fn compare_item(
    id: &str,
    theirs: &cdda_json::Object,
    ours: &items::ItemType,
    mismatches: &mut Vec<String>,
) {
    let text_of = |member: &str| match theirs.get(member) {
        Some(Value::Str(value)) => value.clone(),
        _ => String::new(),
    };
    let integer_of = |member: &str| match theirs.get(member) {
        Some(Value::Int(value)) => *value,
        _ => 0,
    };
    for (field, ours_value, theirs_value) in [
        ("name", ours.name.clone(), text_of("name")),
        ("color", ours.color_name().to_owned(), text_of("color")),
    ] {
        if ours_value != theirs_value {
            mismatches.push(format!("{id}: {field} {ours_value:?} != {theirs_value:?}"));
        }
    }
    for (field, ours_value, theirs_value) in [
        ("weight", ours.weight / 1_000, integer_of("weight")),
        ("volume", ours.volume, integer_of("volume")),
        ("price", ours.price, integer_of("price")),
    ] {
        if ours_value != theirs_value {
            mismatches.push(format!("{id}: {field} {ours_value} != {theirs_value}"));
        }
    }
    // The glyph is a character; the dump carries it as text so that a
    // multi-byte symbol survives.
    let symbol = text_of("symbol")
        .chars()
        .next()
        .map(|c| i64::from(u32::from(c)));
    if symbol.is_some() && i64::from(ours.symbol) != symbol.unwrap_or(0) {
        mismatches.push(format!(
            "{id}: symbol {} != {}",
            ours.symbol,
            symbol.unwrap_or(0)
        ));
    }
    let mut theirs_materials = std::collections::BTreeMap::new();
    if let Some(Value::Array(items)) = theirs.get("materials") {
        for item in items {
            if let Value::Array(pair) = item
                && let Some(Value::Str(name)) = pair.first()
            {
                let weight = match pair.get(1) {
                    Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(1),
                    _ => 1,
                };
                theirs_materials.insert(name.clone(), weight);
            }
        }
    }
    if ours.materials != theirs_materials {
        mismatches.push(format!(
            "{id}: materials {:?} != {theirs_materials:?}",
            ours.materials
        ));
    }
    if ours.plural_name() != text_of("name_plural") {
        mismatches.push(format!(
            "{id}: plural {:?} != {:?}",
            ours.plural_name(),
            text_of("name_plural")
        ));
    }
    if ours.category != text_of("category") {
        mismatches.push(format!(
            "{id}: category {:?} != {:?} (subtypes {:?})",
            ours.category,
            text_of("category"),
            ours.subtypes
        ));
    }
    let ours_flags: Vec<String> = ours.flags.iter().cloned().collect();
    let mut theirs_flags: Vec<String> = match theirs.get("flags") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    theirs_flags.sort();
    if ours_flags != theirs_flags {
        let missing: Vec<&String> = theirs_flags
            .iter()
            .filter(|flag| !ours_flags.contains(flag))
            .collect();
        let extra: Vec<&String> = ours_flags
            .iter()
            .filter(|flag| !theirs_flags.contains(flag))
            .collect();
        mismatches.push(format!("{id}: flags missing {missing:?} extra {extra:?}"));
    }
}

#[test]
fn a_category_comes_from_the_slots_a_stated_one_that_names_nothing_does_not() {
    let loaded = fixture(
        r#"[
        { "type": "ITEM", "subtypes": [ "TOOL" ], "id": "test_wrench",
          "name": { "str": "wrench" }, "symbol": ";", "color": "light_gray",
          "weight": "300 g", "volume": "250 ml" },
        { "type": "ITEM", "subtypes": [ "COMESTIBLE" ], "id": "test_pill",
          "name": { "str": "pill" }, "symbol": "!", "color": "white",
          "weight": "1 g", "volume": "1 ml", "comestible_type": "MED" },
        { "type": "ITEM", "subtypes": [ "ARMOR" ], "id": "test_coat",
          "name": { "str": "coat" }, "symbol": "[", "color": "brown",
          "weight": "1 kg", "volume": "2 L", "category": "clothing" },
        { "type": "ITEM", "subtypes": [ "TOOL" ], "id": "test_odd",
          "name": { "str": "odd" }, "symbol": ";", "color": "black",
          "weight": "300 g", "volume": "250 ml", "category": "tool" },
        { "type": "ITEM_CATEGORY", "id": "clothing", "name_header": { "str": "Clothing" },
          "name_noun": { "str": "item" }, "sort_rank": 1 }
        ]"#,
        "cdda-item-fixture-category",
    );
    let by_id = |id: &str| {
        loaded
            .iter()
            .find(|item| item.id == id)
            .expect("prototype")
            .clone()
    };
    assert_eq!(by_id("test_wrench").category, "tools");
    // A medicinal comestible is a drug, not food.
    assert_eq!(by_id("test_pill").category, "drugs");
    // A category the content registers is kept as written.
    assert_eq!(by_id("test_coat").category, "clothing");
    // A category that names nothing is replaced by the derived one.
    assert_eq!(by_id("test_odd").category, "tools");
}

#[test]
fn stacking_ammunition_cannot_be_repaired_and_materials_bring_their_flags() {
    let loaded = fixture(
        r#"[
        { "type": "ITEM", "subtypes": [ "AMMO" ], "id": "test_round",
          "name": { "str": "round" }, "symbol": "=", "color": "yellow",
          "weight": "10 g", "volume": "10 ml", "material": [ "flesh" ],
          "flags": [ "NPC_THROWN", "LIGHT_2" ] }
        ]"#,
        "cdda-item-fixture-flags",
    );
    let round = loaded
        .iter()
        .find(|item| item.id == "test_round")
        .expect("prototype");
    // Ammunition counts by charges, so the reference makes it unrepairable.
    assert!(round.flags.contains("NO_REPAIR"));
    // Flesh is an allergen and a carnivore's food.
    assert!(round.flags.contains("ALLERGEN_MEAT"));
    assert!(round.flags.contains("CARNIVORE_OK"));
    // A thrown item becomes an alternative attack, and both are trader-avoided.
    assert!(round.flags.contains("NPC_ALT_ATTACK"));
    // A light level is read into the emission and dropped from the flags.
    assert!(!round.flags.iter().any(|flag| flag.starts_with("LIGHT_")));
}

#[test]
fn a_proportional_weight_beats_a_relative_one() {
    let loaded = fixture(
        r#"[
        { "type": "GENERIC", "id": "test_base", "name": { "str": "thing" },
          "symbol": "x", "color": "white", "weight": "3600 g", "volume": "1 L" },
        { "type": "GENERIC", "id": "test_both", "copy-from": "test_base",
          "name": { "str": "thing" }, "proportional": { "weight": 1.25 },
          "relative": { "weight": "400 g" } }
        ]"#,
        "cdda-item-fixture-operators",
    );
    let both = loaded
        .iter()
        .find(|item| item.id == "test_both")
        .expect("prototype");
    // 3600 g * 1.25, not 3600 g + 400 g: the reader checks proportional first.
    assert_eq!(both.weight, 4_500_000);
}
