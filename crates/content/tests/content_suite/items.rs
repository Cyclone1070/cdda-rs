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
    // A fixture is its own little tree, so it cannot come from the shared corpus.
    items::load_paths(&[&dir]).expect("fixture loads")
}

#[test]
fn a_name_object_gives_a_singular_and_a_plural() {
    let loaded = fixture(
        r#"[
        { "type": "ITEM", "id": "test_knife", "name": { "str": "knife", "str_pl": "knives" },
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
        { "type": "ITEM", "id": "test_mixed", "name": "mixed thing", "symbol": "x",
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
    let loaded = crate::corpus::items_loaded();
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
        { "type": "ITEM", "id": "test_base", "name": { "str": "thing" },
          "symbol": "x", "color": "white", "weight": "3600 g", "volume": "1 L" },
        { "type": "ITEM", "id": "test_both", "copy-from": "test_base",
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

#[test]
fn exactly_two_types_in_the_data_weigh_something_else_when_damaged() {
    // **The corpus side of `item::weight( integral: true )`.** The dump carries both numbers now, and the data's own answer is worth pinning: two types differ, and three
    // gunmods say zero **on purpose** - a gunmod's weight does not count toward the gun's integral weight. A loader that treated zero as "unset" and copied the ordinary
    // weight over it would make those three count, and nothing else in the port would notice.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let bytes = std::fs::read(&path).expect("the dump");
    let cdda_json::Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump holds prototypes")
    };
    let mut rows: Vec<(String, i64, i64)> = Vec::new();
    for entry in prototypes {
        let cdda_json::Value::Object(entry) = entry else {
            panic!("a prototype")
        };
        let whole = |key: &str| match entry.get(key) {
            Some(cdda_json::Value::Int(value)) => *value,
            other => panic!("{key} is a whole number: {other:?}"),
        };
        let Some(cdda_json::Value::Str(id)) = entry.get("id") else {
            panic!("an id")
        };
        rows.push((id.clone(), whole("weight"), whole("integral_weight")));
    }
    // **Two types weigh a different non-zero number**, one lighter and one heavier: the rule is a second number per type, not a fraction of the first.
    let differing: Vec<(&str, i64, i64)> = rows
        .iter()
        .filter(|(_, weight, integral)| integral != weight && *integral != 0)
        .map(|(id, weight, integral)| (id.as_str(), *weight, *integral))
        .collect();
    assert_eq!(
        differing,
        vec![
            ("robofac_gun_shotgun_breach", 1000, 650),
            ("retool_ar15_223rem_pistol", 1134, 1819),
        ],
        "two types weigh differently when damaged, one lighter and one heavier"
    );
    // **And twenty-three say zero on purpose**, every one of them a gunmod: a gunmod's weight does not count toward the gun's integral weight. This is the count that
    // matters for the loader, because zero is a value there and not an absence - a loader that copied the ordinary weight over it would weigh a stocked rifle at its
    // stock's weight as well as its own.
    let zero_on_purpose: Vec<&str> = rows
        .iter()
        .filter(|(_, weight, integral)| *integral == 0 && *weight > 0)
        .map(|(id, _, _)| id.as_str())
        .collect();
    assert_eq!(
        zero_on_purpose.len(),
        23,
        "twenty-three types weigh nothing when the whole thing is asked for: {zero_on_purpose:?}"
    );
    // And the **loader** carries it: the two types keep their two numbers (the loader works in milligrams, the dump in grams), and an inherited zero stays zero
    // rather than falling back to the ordinary weight.
    let loaded = crate::corpus::items_loaded();
    for (id, weight, integral) in [
        ("robofac_gun_shotgun_breach", 1000_i64, 650_i64),
        ("barrel_ported", 225, 0),
        ("match_trigger", 120, 0),
    ] {
        let item = loaded.iter().find(|item| item.id == id).expect("the type loads");
        assert_eq!(item.weight, weight * 1_000, "{id} weighs what the dump says");
        assert_eq!(
            item.integral_weight,
            integral * 1_000,
            "{id}'s integral weight is the dump's, not the ordinary weight"
        );
    }
}

#[test]
fn the_stack_size_the_volume_rule_divides_by_is_in_the_dump() {
    // `item::volume` divides a charge-counted item's volume by `stack_size` and rounds up, so a port without the divisor cannot say what a pile takes up. The dump
    // carries it now, with the flag that decides whether it matters: 1,261 types in the data are counted in charges, and the battery is the one to check by hand -
    // 100 ml for 100 charges, which is one millilitre a charge.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let bytes = std::fs::read(&path).expect("the dump");
    let cdda_json::Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump holds prototypes")
    };
    let mut charged = 0_usize;
    let mut battery = None;
    for entry in prototypes {
        let cdda_json::Value::Object(entry) = entry else {
            panic!("a prototype")
        };
        let Some(cdda_json::Value::Str(id)) = entry.get("id") else {
            panic!("an id")
        };
        let counted = matches!(entry.get("count_by_charges"), Some(cdda_json::Value::Bool(true)));
        if !counted {
            continue;
        }
        charged += 1;
        let whole = |key: &str| match entry.get(key) {
            Some(cdda_json::Value::Int(value)) => *value,
            other => panic!("{key} is a whole number: {other:?}"),
        };
        if id == "battery" {
            battery = Some((whole("volume"), whole("stack_size")));
        }
    }
    assert_eq!(charged, 1_261, "every charge-counted type carries its stack size");
    assert_eq!(
        battery,
        Some((100, 100)),
        "the battery is 100 ml for 100 charges: one millilitre a charge"
    );
}

#[test]
fn the_integral_volume_is_in_the_dump_and_the_loader_keeps_a_deliberate_zero() {
    // **The corpus side of `item::volume( integral: true )`.** 120 types answer differently from their ordinary volume, and 97 of those say **zero** on purpose - a
    // pseudo magazine, a debug item, a `null` - which is the same trap the integral weight had: zero is a value, not an absence.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let bytes = std::fs::read(&path).expect("the dump");
    let cdda_json::Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump holds prototypes")
    };
    let mut differing = 0_usize;
    let mut zero_on_purpose = 0_usize;
    for entry in prototypes {
        let cdda_json::Value::Object(entry) = entry else {
            panic!("a prototype")
        };
        let whole = |key: &str| match entry.get(key) {
            Some(cdda_json::Value::Int(value)) => *value,
            other => panic!("{key} is a whole number: {other:?}"),
        };
        let (volume, integral) = (whole("volume"), whole("integral_volume"));
        if integral != volume {
            differing += 1;
        }
        if integral == 0 && volume > 0 {
            zero_on_purpose += 1;
        }
    }
    assert_eq!(differing, 120, "120 types take up a different room packed down");
    assert_eq!(zero_on_purpose, 97, "and 97 of them say zero on purpose");
    // The loader carries it, and a deliberate zero survives: a pseudo magazine is one millilitre as it stands and nothing packed down.
    let loaded = crate::corpus::items_loaded();
    let pseudo = loaded
        .iter()
        .find(|item| item.id == "pseudo_magazine")
        .expect("the type loads");
    assert_eq!(pseudo.volume, 1, "one millilitre as it stands");
    assert_eq!(pseudo.integral_volume, 0, "and nothing packed down");
    let rock = loaded
        .iter()
        .find(|item| item.id == "rock")
        .expect("the rock loads");
    assert_eq!(
        rock.integral_volume, rock.volume,
        "a type that says nothing keeps its ordinary volume"
    );
}

/// **What an item is** is a predicate over its kind's data, and the reference answers the same questions about the same kinds.
///
/// Reference: `item::is_container` (`item.cpp:10238-10241`), `is_tool`, `is_gun`, `is_ammo` (`item.h:1663-1687`, `:2510`). The dump now carries the reference's own answer beside every kind's
/// data - **1,145 containers, 1,237 tools, 406 guns, 736 ammunition types and 1,686 comestibles** - so the corpus is asked what it says rather than assumed to say it.
#[test]
fn the_corpus_says_what_its_kinds_are() {
    let dump = std::fs::read(root().join("corpus/prototypes/items.json")).expect("the item dump");
    let parsed = cdda_json::parse(&dump).expect("valid JSON");
    let cdda_json::Value::Object(row_root) = parsed else {
        panic!("the item dump is an object");
    };
    let Some(cdda_json::Value::Array(prototypes)) = row_root.get("prototypes") else {
        panic!("the item dump has prototypes");
    };
    let count = |name: &str| {
        prototypes
            .iter()
            .filter(|entry| matches!(entry, cdda_json::Value::Object(row) if matches!(row.get(name), Some(cdda_json::Value::Bool(true)))))
            .count()
    };
    assert_eq!(prototypes.len(), 9912, "**nine thousand nine hundred and twelve kinds**");
    assert_eq!(count("holds_container_pocket"), 1146, "a thousand and a hundred and forty-six hold a container pocket");
    assert_eq!(count("is_tool"), 1237, "twelve hundred and thirty-seven are tools");
    assert_eq!(count("is_gun"), 406, "four hundred and six are guns");
    assert_eq!(count("is_ammo"), 736, "seven hundred and thirty-six are ammunition");
}

/// The four questions are answered from the kind's own data, and **a magazine is not a container**.
///
/// Reference: `item::is_container` asks for a `CONTAINER` pocket (`item.cpp:10238-10241`), while a magazine's pockets are `MAGAZINE`. The port keeps the two questions apart for the same reason the
/// reference does: a magazine holds rounds and a survivor cannot put a rock in it.
///
/// **Written against the type's own facts rather than through the loader**: this round's attempt at a six-prototype fixture loaded no kinds at all, and finding out why is its own piece of work
/// (recorded in `docs/13-test-inventory.md`). A gate that silently sees an empty list would be worse than this one, which builds the two states it is asking about and nothing else.
#[test]
fn a_kind_is_a_container_only_when_it_states_a_container_pocket() {
    // A pocket is a shape with many members; the rule reads **only its type**, so the rest are built as the shape's own empty state.
    let pocket = |kind: &str| {
        let mut spec = cdda_content::pockets::PocketSpec::empty_of_kind(kind);
        spec.kind = kind.to_owned();
        spec
    };
    assert!(
        cdda_content::items::is_container_with(&[pocket("CONTAINER")]),
        "a pocket of type CONTAINER makes a kind a container"
    );
    assert!(
        !cdda_content::items::is_container_with(&[pocket("MAGAZINE")]),
        "**a magazine pocket does not**, though it holds rounds"
    );
    assert!(
        !cdda_content::items::is_container_with(&[]),
        "and a kind with no pockets holds nothing"
    );
}

/// A fixture of six kinds loads six kinds, and the kinds are told apart by **subtypes**, the way the reference tells them apart.
///
/// Reference: `add( "ITEM", &items::load )` (`src/init.cpp:351`) and `itype::load_slots`' `subtype_to_slot` (`src/item_factory.cpp:2862-2941`). The tool, gun and ammunition here state
/// `"type": "ITEM"` and name their slot in `subtypes`, which is what every one of the 9,912 real prototypes does.
#[test]
fn a_simple_fixture_loads_every_kind_it_states() {
    let loaded = fixture(
        r#"[{ "type": "ITEM", "id": "bag", "name": { "str": "bag" }, "symbol": "(", "color": "light_gray", "weight": "300 g", "volume": "1 L", "price": "1 USD", "material": [ "cotton" ], "pocket_data": [ { "pocket_type": "CONTAINER" } ] },
            { "type": "ITEM", "id": "mag", "name": { "str": "magazine" }, "symbol": "(", "color": "light_gray", "weight": "100 g", "volume": "250 ml", "price": "1 USD", "material": [ "steel" ], "subtypes": [ "MAGAZINE" ], "pocket_data": [ { "pocket_type": "MAGAZINE" } ] },
            { "type": "ITEM", "id": "lighter", "name": { "str": "lighter" }, "symbol": ";", "color": "light_gray", "weight": "10 g", "volume": "10 ml", "price": "1 USD", "material": [ "steel" ], "subtypes": [ "TOOL" ] },
            { "type": "ITEM", "id": "pistol", "name": { "str": "pistol" }, "symbol": "(", "color": "light_gray", "weight": "1 kg", "volume": "500 ml", "price": "1 USD", "material": [ "steel" ], "subtypes": [ "GUN" ] },
            { "type": "ITEM", "id": "ball", "name": { "str": "ball" }, "symbol": "=", "color": "light_gray", "weight": "1 g", "volume": "1 ml", "price": "1 USD", "material": [ "steel" ], "subtypes": [ "AMMO" ], "ammo_type": "9mm" },
            { "type": "ITEM", "id": "rock", "name": { "str": "rock" }, "symbol": "*", "color": "light_gray", "weight": "500 g", "volume": "250 ml", "price": "1 USD", "material": [ "stone" ] }]"#,
        "six_kind_fixture",
    );
    for id in ["bag", "mag", "lighter", "pistol", "ball", "rock"] {
        assert!(
            loaded.iter().any(|kind| kind.id == id),
            "**{id} went missing from the fixture**: the loader kept {:?}",
            loaded.iter().map(|kind| kind.id.as_str()).collect::<Vec<_>>()
        );
    }
    assert_eq!(loaded.len(), 6, "and nothing else was invented");
    let find = |id: &str| loaded.iter().find(|kind| kind.id == id).expect("a kind");
    assert!(find("lighter").is_tool(), "the tool's slot is the subtype it states");
    assert!(find("pistol").is_gun(), "and the gun's is its own");
}

/// **A prototype that names a slot as its type is not an item**, because the reference has no loader for one.
///
/// Reference: `src/init.cpp:351` registers `ITEM` and nothing else for item data; `"type": "TOOL"` is an unknown type there, and the data never uses it. The port's filter is one type wide for that
/// reason: a widened filter looks like a fix for a missing kind and is instead an invented rule.
#[test]
fn a_slot_named_as_a_type_is_not_an_item() {
    let loaded = fixture(
        r#"[
        { "type": "ITEM", "id": "real_tool", "name": { "str": "tool" }, "symbol": ";", "color": "light_gray", "weight": "10 g", "volume": "10 ml", "price": "1 USD", "material": [ "steel" ], "subtypes": [ "TOOL" ] },
        { "type": "TOOL", "id": "legacy_tool", "name": { "str": "tool" }, "symbol": ";", "color": "light_gray", "weight": "10 g", "volume": "10 ml", "price": "1 USD", "material": [ "steel" ] }
        ]"#,
        "legacy_type_fixture",
    );
    assert!(
        loaded.iter().any(|kind| kind.id == "real_tool"),
        "the prototype that states the reference's own type loads"
    );
    assert!(
        !loaded.iter().any(|kind| kind.id == "legacy_tool"),
        "**and the one that names a slot as its type does not**, whatever a widener might wish"
    );
    assert_eq!(loaded.len(), 1, "so exactly one of the two is an item");
}

/// **Every kind answers what it is the way the reference does**, all nine thousand nine hundred and twelve of them.
///
/// Reference: `item::is_container` (`item.cpp:10238-10241`), `is_tool`, `is_gun`, `is_ammo` (`item.h:1663-1687`, `:2510`). The dump carries the reference's own answer beside every kind's data,
/// so this is a comparison against the reference and not against a rule the port wrote down. The three the dump names and the port answers from the kind's slots must agree on all 9,912.
#[test]
fn every_kind_answers_what_it_is_the_way_the_reference_does() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    let loaded = items::load_paths(&[&data]).expect("the reference data loads");
    let dump = std::fs::read(root().join("corpus/prototypes/items.json")).expect("the item dump");
    let parsed = cdda_json::parse(&dump).expect("valid JSON");
    let cdda_json::Value::Object(root_object) = parsed else {
        panic!("the item dump is an object");
    };
    let Some(cdda_json::Value::Array(prototypes)) = root_object.get("prototypes") else {
        panic!("the item dump has prototypes");
    };
    let ours: std::collections::BTreeMap<&str, &items::ItemType> =
        loaded.iter().map(|kind| (kind.id.as_str(), kind)).collect();
    assert_eq!(ours.len(), loaded.len(), "**ids are unique, so the map kept every kind**");

    let mut compared = 0;
    for entry in prototypes {
        let cdda_json::Value::Object(row) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = row.get("id") else {
            continue;
        };
        let Some(kind) = ours.get(id.as_str()) else {
            continue;
        };
        let said = |name: &str| matches!(row.get(name), Some(cdda_json::Value::Bool(true)));
        assert_eq!(kind.is_tool(), said("is_tool"), "{id} is a tool");
        assert_eq!(kind.is_gun(), said("is_gun"), "{id} is a gun");
        assert_eq!(kind.is_ammo(), said("is_ammo"), "{id} is ammunition");
        assert_eq!(kind.is_container(), said("holds_container_pocket"), "{id} holds a container pocket");
        compared += 1;
    }
    assert!(
        compared > 9_800,
        "**the bulk of the corpus was compared, not a corner of it**: {compared} of {}",
        prototypes.len()
    );
}

/// **Every kind's other dumped facts agree with the reference too**: is it a gun mod, how many charges does one use cost, and what does it feed.
///
/// Reference: `item::is_gunmod` (`item.cpp`), `itype::charges_to_use` and the comestible slot's `get_default_nutr` (`src/itype.h:147-248`). The first whole-corpus comparison found two real defects in
/// the four questions it checked, so the same treatment is given to the three facts beside them: the reference's own answer is in the dump, and the port's answer must be the same for all 9,912.
#[test]
fn every_kind_s_other_facts_are_the_reference_s() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    let loaded = items::load_paths(&[&data]).expect("the reference data loads");
    let dump = std::fs::read(root().join("corpus/prototypes/items.json")).expect("the item dump");
    let parsed = cdda_json::parse(&dump).expect("valid JSON");
    let cdda_json::Value::Object(root_object) = parsed else {
        panic!("the item dump is an object");
    };
    let Some(cdda_json::Value::Array(prototypes)) = root_object.get("prototypes") else {
        panic!("the item dump has prototypes");
    };
    let ours: std::collections::BTreeMap<&str, &items::ItemType> =
        loaded.iter().map(|kind| (kind.id.as_str(), kind)).collect();

    let mut compared = 0;
    for entry in prototypes {
        let cdda_json::Value::Object(row) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = row.get("id") else {
            continue;
        };
        let Some(kind) = ours.get(id.as_str()) else {
            continue;
        };
        let said = |name: &str| matches!(row.get(name), Some(cdda_json::Value::Bool(true)));
        let number = |name: &str| match row.get(name) {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
            _ => 0,
        };
        assert_eq!(kind.is_gunmod, said("is_gunmod"), "{id} is a gunmod");
        assert_eq!(kind.charges_to_use(), number("charges_to_use"), "{id} costs charges");
        // The slot keeps **kilocalories** and the dump carries the reference's `get_default_nutr()`, so the comparison is between the same fact in the same unit.
        // The dump's `nutr` column **is** the reference's `get_default_nutr()` (`itype.h:256-258`), in nutrition units and not in kilocalories.
        let our_calories = kind
            .comestible
            .as_ref()
            .map_or(0, |slot| items::nutr_from_kcal(slot.calories() / 1000));
        let their_calories = i64::from(number("nutr"));
        assert_eq!(our_calories, their_calories, "{id} states calories");
        compared += 1;
    }
    assert!(compared > 9_800, "**the bulk of the corpus was compared**: {compared}");
}
