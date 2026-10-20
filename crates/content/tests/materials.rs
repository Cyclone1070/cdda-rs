//! Materials against the reference's own resolve.
//!
//! The oracle is `corpus/prototypes/materials.json`: every material after the
//! reference loaded its files, with the numeric fields a port can get wrong
//! silently - density, chip resistance, the breathability rating, and the three
//! heat constants.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::materials;
use cdda_json::{Value, parse};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn a_material_takes_its_numbers_and_its_breathability_rating() {
    let dir = std::env::temp_dir().join("cdda-material-fixture");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("materials.json"),
        r#"[
        {
          "type": "material",
          "id": "test_cloth",
          "name": "cloth",
          "density": 3,
          "chip_resist": 4,
          "breathability": "good",
          "specific_heat_liquid": 1.5,
          "specific_heat_solid": 1.25,
          "latent_heat": 200
        }
        ]"#,
    )
    .expect("fixture");
    let loaded = materials::load_paths(&[&dir]).expect("loads");
    let cloth = loaded
        .iter()
        .find(|entry| entry.id == "test_cloth")
        .expect("prototype");
    assert_eq!(cloth.name, "cloth");
    assert!((cloth.density - 3.0).abs() < f64::EPSILON);
    assert_eq!(cloth.chip_resist, 4);
    // "good" is rated 80, not the enum ordinal 3.
    assert_eq!(cloth.breathability, 80);
    assert!((cloth.specific_heat_liquid - 1.5).abs() < f64::EPSILON);
    assert!((cloth.latent_heat - 200.0).abs() < f64::EPSILON);
}

#[test]
fn every_material_matches_the_reference() {
    let text =
        std::fs::read_to_string(root().join("corpus/prototypes/materials.json")).expect("artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let Value::Object(root_object) = value else {
        panic!("root")
    };
    let Some(Value::Array(entries)) = root_object.get("prototypes") else {
        panic!("prototypes")
    };
    let loaded = materials::load_paths(&[
        &root().join("references/Cataclysm-DDA/data/core"),
        &root().join("references/Cataclysm-DDA/data/json"),
    ])
    .expect("loads");
    println!(
        "loaded {} materials, reference has {}",
        loaded.len(),
        entries.len()
    );
    let mut mismatches = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        let text_of = |member: &str| match object.get(member) {
            Some(Value::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let number_of = |member: &str| match object.get(member) {
            Some(Value::Float(value)) => *value,
            #[allow(clippy::cast_precision_loss)]
            Some(Value::Int(value)) => *value as f64,
            _ => 0.0,
        };
        let id = text_of("id");
        let Some(ours) = loaded.iter().find(|material| material.id == id) else {
            mismatches.push(format!("{id}: missing"));
            continue;
        };
        if ours.name != text_of("name") {
            mismatches.push(format!(
                "{id}: name {:?} != {:?}",
                ours.name,
                text_of("name")
            ));
        }
        for (field, value) in [
            ("density", ours.density),
            ("specific_heat_liquid", ours.specific_heat_liquid),
            ("specific_heat_solid", ours.specific_heat_solid),
            ("latent_heat", ours.latent_heat),
        ] {
            if (value - number_of(field)).abs() > 1e-4 {
                mismatches.push(format!("{id}: {field} {value} != {}", number_of(field)));
            }
        }
        let integer_of = |member: &str| match object.get(member) {
            Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(i32::MAX),
            _ => 0,
        };
        if ours.chip_resist != integer_of("chip_resist") {
            mismatches.push(format!(
                "{id}: chip_resist {} != {}",
                ours.chip_resist,
                integer_of("chip_resist")
            ));
        }
        if ours.breathability != integer_of("breathability") {
            mismatches.push(format!(
                "{id}: breathability {} != {}",
                ours.breathability,
                integer_of("breathability")
            ));
        }
    }
    println!("{} mismatches", mismatches.len());
    for line in mismatches.iter().take(15) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

fn load() -> Vec<materials::Material> {
    materials::load_paths(&[
        &root().join("references/Cataclysm-DDA/data/core"),
        &root().join("references/Cataclysm-DDA/data/json"),
    ])
    .expect("loads")
}

#[test]
fn materials_carry_what_a_fire_needs_at_each_intensity() {
    // Every material describes three intensities of burning, and the numbers climb with intensity
    // for anything that actually burns - which is the shape the fire processor relies on.
    let all = load();
    assert_eq!(all.len(), 193, "materials");
    // 109 of 193 materials carry `burn_data` in the data; the rest are filled with defaults by the
    // reference at load time, which the port does not reproduce yet - recorded in the ledger.
    let with_burn = all
        .iter()
        .filter(|material| !material.burn.is_empty())
        .count();
    assert_eq!(with_burn, 109, "materials that describe burning");
    for material in all.iter().filter(|material| !material.burn.is_empty()) {
        assert_eq!(
            material.burn.len(),
            3,
            "{} must describe three intensities",
            material.id
        );
        assert!(
            material
                .burn
                .iter()
                .all(|burn| (1..=3).contains(&burn.intensity)),
            "{}: intensities are 1, 2, 3",
            material.id
        );
    }
    let wood = all.iter().find(|m| m.id == "wood").expect("wood exists");
    let fuel: Vec<f64> = wood.burn.iter().map(|burn| burn.fuel).collect();
    assert_eq!(fuel, vec![1.0, 2.0, 3.0], "wood fuels harder fires");
    assert!(
        wood.burn.iter().all(|burn| !burn.immune),
        "and always burns"
    );
}

#[test]
fn exactly_one_material_refuses_to_burn_at_all() {
    let all = load();
    let immune: Vec<&str> = all
        .iter()
        .filter(|m| !m.burn.is_empty() && m.burn.iter().all(|burn| burn.immune))
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(immune, vec!["nomex"], "only nomex is fireproof throughout");
}
