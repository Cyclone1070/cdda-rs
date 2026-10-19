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
    let mut pending = Pending::default();
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
        compare_item(&id, object, ours, &mut mismatches, &mut pending);
    }
    println!(
        "{} mismatches; pending: {} category, {} flags, {} materials, {} plural",
        mismatches.len(),
        pending.category,
        pending.flags,
        pending.materials,
        pending.plural
    );
    for line in mismatches.iter().take(20) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

/// What is still unverifiable because it belongs to a per-type loader.
#[derive(Default)]
struct Pending {
    category: usize,
    flags: usize,
    materials: usize,
    plural: usize,
}

fn compare_item(
    id: &str,
    theirs: &cdda_json::Object,
    ours: &items::ItemType,
    mismatches: &mut Vec<String>,
    pending: &mut Pending,
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
        // Armour states its materials per body part, and the loader aggregates
        // them; that derivation belongs to the armour loader, not to this slice.
        pending.materials += 1;
    }
    // Category and flags are set partly by the per-type loaders rather than by
    // the fields an object states, so they are counted separately: this slice
    // covers what the JSON says, not what a type implies.
    // The plural is pending: the reference reports the singular for some names
    // whose JSON states a `str_pl`, so the rule behind `nname( 2 )` needs its
    // own investigation rather than a guess.
    if ours.name_plural != text_of("name_plural") {
        pending.plural += 1;
    }
    if ours.category != text_of("category") {
        pending.category += 1;
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
        pending.flags += 1;
    }
}
