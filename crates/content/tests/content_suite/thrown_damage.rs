//! What every item does when thrown, against the reference's own numbers.
//!
//! `corpus/items/thrown-damage.json` is `--dump-thrown-damage`: 9,912 types and the value `item::base_damage_thrown` answers for each
//! (`item.cpp:7654-7658`). The value is **computed at load** - `item_factory.cpp:324-326` fills it with `melee.damage_map[bash] * 0.75` when the data names no
//! `thrown_damage` - so a port that only read a stated member would give zero to almost every thrown thing, and a rock would stop hurting.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_json::Value;
use std::collections::BTreeMap;

fn recorded() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/thrown-damage.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

fn loaded() -> BTreeMap<String, ItemType> {
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect()
}

#[test]
fn every_item_throws_for_what_the_reference_says() {
    let Value::Object(root) = recorded() else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    assert_eq!(types.len(), 9_912, "every item type was recorded");
    let items = loaded();
    let mut compared = 0;
    let mut wrong = Vec::new();
    for entry in types {
        let Value::Object(entry) = entry else {
            continue;
        };
        let (Some(Value::Str(id)), Some(total)) = (entry.get("id"), entry.get("total")) else {
            continue;
        };
        let recorded = match total {
            Value::Float(number) => *number,
            #[allow(clippy::cast_precision_loss)]
            Value::Int(number) => *number as f64,
            _ => continue,
        };
        let Some(item) = items.get(id) else {
            continue;
        };
        compared += 1;
        if (item.thrown_damage - recorded).abs() > 1e-6 {
            wrong.push(format!("{id}: port {} versus reference {recorded}", item.thrown_damage));
        }
    }
    // Every one of the 9,912 types now throws for exactly what the reference says. Three rules got it there, and the corpus named each one:
    //   * a melee damage amount is a **whole number** - `mace_fake`'s `40 * 0.38` is 15, not 15.2;
    //   * a stated `melee_damage` **replaces** what a record copies (`pressin`);
    //   * and a stated `thrown_damage` **inherits** its emptiness, so `bone_dart` and `pulverizer` throw for their copied number rather than for their own bash.
    assert!(
        wrong.is_empty(),
        "{} of {compared} types throw for a different number: {:?}",
        wrong.len(),
        wrong.iter().take(8).collect::<Vec<_>>()
    );
    assert_eq!(compared, 9_912, "and every recorded type was compared");
    // A rock is the case the throwing rules are tested with, so the number a rock answers is asserted by name.
    let rock = items.get("rock").expect("the data has a rock");
    assert!(
        (rock.thrown_damage - 7.0).abs() < 1e-6,
        "a rock throws for its bash damage less a quarter: {}",
        rock.thrown_damage
    );
}
