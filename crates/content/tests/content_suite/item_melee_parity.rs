//! The loader's melee numbers and the reference's own dump agree, for every item type.
//!
//! The registry gate (`sim_suite::melee_rules`) proves the registry **carries** what it is given; this one proves the **loader** gives it the right thing. The two are different
//! mistakes: round 1096 found the loader reading an object `to_hit` as the -2 default while every registry test passed, because the registry's fixture was built from the dump.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::{Value, parse};
use std::collections::BTreeMap;

/// What the reference says about one item type.
struct Dumped {
    to_hit: i64,
    bash: f64,
    cut: f64,
    stab: f64,
}

fn dumped() -> BTreeMap<String, Dumped> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/thrown-damage.json");
    let bytes = std::fs::read(path).expect("the item dump exists");
    let root = parse(&bytes).expect("the dump is valid JSON");
    let Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump has no types");
    };
    let number = |value: Option<&Value>| -> f64 {
        match value {
            Some(Value::Float(value)) => *value,
            Some(Value::Int(value)) => {
                #[allow(clippy::cast_precision_loss)]
                {
                    *value as f64
                }
            }
            _ => 0.0,
        }
    };
    types
        .iter()
        .filter_map(|one| {
            let Value::Object(fields) = one else {
                return None;
            };
            let Some(Value::Str(id)) = fields.get("id") else {
                return None;
            };
            #[allow(clippy::cast_possible_truncation)]
            let to_hit = number(fields.get("to_hit")) as i64;
            Some((
                id.clone(),
                Dumped {
                    to_hit,
                    bash: number(fields.get("melee_bash")),
                    cut: number(fields.get("melee_cut")),
                    stab: number(fields.get("melee_stab")),
                },
            ))
        })
        .collect()
}

/// Every item the game ships is loaded with the melee numbers the reference gives it.
#[test]
fn every_items_melee_numbers_are_the_references() {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::items::load_paths(&paths).expect("the game's items load");
    let by_id: BTreeMap<&str, &cdda_content::items::ItemType> =
        loaded.iter().map(|item| (item.id.as_str(), item)).collect();
    let expected = dumped();
    assert_eq!(expected.len(), 9_912, "every type in the dump");

    let mut wrong: Vec<String> = Vec::new();
    let mut compared = 0_usize;
    for (id, want) in &expected {
        let Some(item) = by_id.get(id.as_str()) else {
            wrong.push(format!("{id}: not loaded"));
            continue;
        };
        compared += 1;
        let bash = item.melee.get("bash").copied().unwrap_or(0.0);
        let cut = item.melee.get("cut").copied().unwrap_or(0.0);
        let stab = item.melee.get("stab").copied().unwrap_or(0.0);
        if i64::from(item.to_hit) != want.to_hit
            || (bash - want.bash).abs() > f64::EPSILON
            || (cut - want.cut).abs() > f64::EPSILON
            || (stab - want.stab).abs() > f64::EPSILON
        {
            wrong.push(format!(
                "{id}: to_hit {} vs {}, melee ({bash}, {cut}, {stab}) vs ({}, {}, {})",
                item.to_hit, want.to_hit, want.bash, want.cut, want.stab
            ));
        }
    }
    let first_few: Vec<&String> = wrong.iter().take(5).collect();
    assert!(
        wrong.is_empty(),
        "{} of {compared} wrong: {first_few:?}",
        wrong.len()
    );
    assert!(compared > 9_000, "and most of them were compared: {compared}");
}
