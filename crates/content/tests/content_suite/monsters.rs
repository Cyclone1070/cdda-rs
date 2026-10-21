//! Monster types against the reference's own resolve.
//!
//! The oracle is `corpus/prototypes/monsters.json`: 1,154 monster types after the
//! reference loaded its files.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::monsters;
use cdda_json::{Object, Value, parse};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_monster_type_matches_the_reference() {
    let data = root().join("references/Cataclysm-DDA/data");
    let loaded = monsters::load_paths(&[&data.join("core"), &data.join("json")]).expect("loads");
    let bytes = std::fs::read(root().join("corpus/prototypes/monsters.json")).expect("corpus");
    let dump = parse(&bytes).expect("parses");
    let Value::Object(dump) = dump else {
        panic!("the corpus is an object");
    };
    let Some(Value::Array(theirs)) = dump.get("prototypes") else {
        panic!("the corpus holds prototypes");
    };
    println!(
        "loaded {} monsters, reference has {}",
        loaded.len(),
        theirs.len()
    );
    assert_eq!(loaded.len(), theirs.len());

    let mut mismatches = Vec::new();
    for entry in theirs {
        let Value::Object(entry) = entry else {
            continue;
        };
        let id = text_of(entry, "id");
        let Some(ours) = loaded.iter().find(|monster| monster.id == id) else {
            mismatches.push(format!("{id}: missing"));
            continue;
        };
        compare(ours, entry, &mut mismatches);
    }
    for line in mismatches.iter().take(20) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

fn compare(ours: &monsters::MonsterType, theirs: &Object, mismatches: &mut Vec<String>) {
    let id = ours.id.as_str();
    let mut check = |field: &str, same: bool, shown: String| {
        if !same {
            mismatches.push(format!("{id}: {field} {shown}"));
        }
    };
    check(
        "name",
        ours.name == text_of(theirs, "name"),
        format!("{:?} != {:?}", ours.name, text_of(theirs, "name")),
    );
    check(
        "symbol",
        ours.symbol == text_of(theirs, "symbol"),
        format!("{:?} != {:?}", ours.symbol, text_of(theirs, "symbol")),
    );
    check(
        "color",
        cdda_core_types::colors::string_from_color(ours.color) == text_of(theirs, "color"),
        format!(
            "{:?} != {:?}",
            cdda_core_types::colors::string_from_color(ours.color),
            text_of(theirs, "color")
        ),
    );
    check(
        "bodytype",
        ours.bodytype == text_of(theirs, "bodytype"),
        format!("{:?} != {:?}", ours.bodytype, text_of(theirs, "bodytype")),
    );
    for (field, value) in [("hp", ours.hp), ("speed", ours.speed), ("diff", ours.diff)] {
        let theirs_value = number_of(theirs, field);
        check(
            field,
            i64::from(value) == theirs_value,
            format!("{value} != {theirs_value}"),
        );
    }
    for (field, value) in [
        ("melee_skill", ours.melee_skill),
        ("dodge", ours.dodge),
        ("morale", ours.morale),
        ("aggression", ours.aggression),
    ] {
        let theirs_value = number_of(theirs, field);
        check(
            field,
            i64::from(value) == theirs_value,
            format!("{value} != {theirs_value}"),
        );
    }
    // Weight is carried in milligrams, as the content states it; the dump
    // reports grams, so the comparison divides.
    let theirs_weight = number_of(theirs, "weight");
    check(
        "weight",
        ours.weight / 1_000 == theirs_weight,
        format!("{} != {theirs_weight}", ours.weight / 1_000),
    );
    let theirs_volume = number_of(theirs, "volume");
    check(
        "volume",
        ours.volume == theirs_volume,
        format!("{} != {theirs_volume}", ours.volume),
    );
    // The reference keeps species, categories and materials in registration
    // order rather than sorted, so the comparison is by contents.
    let mut theirs_species = string_array(theirs, "species");
    theirs_species.sort();
    let ours_species: Vec<String> = ours.species.iter().cloned().collect();
    check(
        "species",
        ours_species == theirs_species,
        format!("{ours_species:?} != {theirs_species:?}"),
    );
    let mut theirs_categories = string_array(theirs, "categories");
    theirs_categories.sort();
    let ours_categories: Vec<String> = ours.categories.iter().cloned().collect();
    check(
        "categories",
        ours_categories == theirs_categories,
        format!("{ours_categories:?} != {theirs_categories:?}"),
    );
    let mut theirs_materials = material_pairs(theirs);
    theirs_materials.sort();
    let ours_materials: Vec<(String, i64)> = ours
        .materials
        .iter()
        .map(|(name, portion)| (name.clone(), i64::from(*portion)))
        .collect();
    check(
        "materials",
        ours_materials == theirs_materials,
        format!("{ours_materials:?} != {theirs_materials:?}"),
    );
}

fn text_of(object: &Object, member: &str) -> String {
    match object.get(member) {
        Some(Value::Str(text)) => text.clone(),
        _ => String::new(),
    }
}

fn number_of(object: &Object, member: &str) -> i64 {
    match object.get(member) {
        Some(Value::Int(value)) => *value,
        _ => i64::MIN,
    }
}

fn string_array(object: &Object, member: &str) -> Vec<String> {
    match object.get(member) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn material_pairs(object: &Object) -> Vec<(String, i64)> {
    match object.get("materials") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Array(pair) => match (pair.first(), pair.get(1)) {
                    (Some(Value::Str(name)), Some(Value::Int(portion))) => {
                        Some((name.clone(), *portion))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
