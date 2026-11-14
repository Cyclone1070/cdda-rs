//! What each material carries, against the reference's own copy.
//!
//! `corpus/items/material-vitamins.json` is `--dump-material-vitamins`: the nonzero vitamins of every one of the 193 materials, read through
//! `material_type::vitamin` (`src/material.h:163-166`). It exists because a food that states no vitamins derives them from its materials
//! (`src/item_factory.cpp:762-776`), so a disagreement about a food has to be chased back to the material data before it can be called a derivation bug — which is
//! exactly what happened on the first run of the comestible gate.

#![allow(clippy::panic, clippy::expect_used, clippy::cast_precision_loss)]

use cdda_content::materials::load_paths;
use cdda_json::Value;
use std::collections::BTreeMap;

#[test]
fn every_material_carries_the_vitamins_the_reference_says() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/material-vitamins.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(materials)) = root.get("materials") else {
        panic!("the dump holds materials")
    };
    assert_eq!(materials.len(), 193, "every material prototype");
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, _> = load_paths(&paths)
        .expect("the materials load")
        .into_iter()
        .map(|material| (material.id.clone(), material))
        .collect();

    let mut differing: Vec<String> = Vec::new();
    let mut carrying = 0;
    for entry in materials {
        let Value::Object(entry) = entry else {
            panic!("a material")
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("an id")
        };
        let Some(Value::Object(theirs)) = entry.get("vitamins") else {
            panic!("a vitamin map")
        };
        let Some(material) = loaded.get(id) else {
            differing.push(format!("{id}: not loaded"));
            continue;
        };
        if !theirs.members().is_empty() {
            carrying += 1;
        }
        let ours: BTreeMap<&str, f64> = material
            .vitamins
            .iter()
            .map(|(name, amount)| (name.as_str(), *amount))
            .collect();
        let theirs: BTreeMap<&str, f64> = theirs
            .members()
            .iter()
            .map(|(name, value)| match value {
                Value::Float(amount) => (name.as_str(), *amount),
                Value::Int(amount) => (name.as_str(), *amount as f64),
                other => panic!("{name}: {other:?}"),
            })
            .collect();
        if ours != theirs {
            differing.push(format!(
                "{id}: the port says {ours:?}, the reference says {theirs:?}"
            ));
        }
    }
    // Only a handful of materials carry vitamins at all, so a loader that dropped them everywhere would otherwise pass on an empty comparison.
    assert!(
        carrying >= 4,
        "some materials do carry vitamins: {carrying}"
    );
    assert!(
        differing.is_empty(),
        "{} of {} materials disagree: {:#?}",
        differing.len(),
        materials.len(),
        differing
    );
}
