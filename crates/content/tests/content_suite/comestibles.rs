//! The comestible slot, against the reference's own resolved copy.
//!
//! `corpus/items/comestibles.json` is `--dump-comestibles`: every item type with a comestible slot, with the slot read out of the reference after finalisation. Two
//! things in it cannot be recovered from the JSON, which is why the recording exists:
//!
//! * `calories` is stored **multiplied by 1,000** (`src/item_factory.cpp:460`), so the data's `"calories": 26` is 26,000 here;
//! * `nutr` is **derived**, `kcal / ( 2500 / 288 )` in `float` (`src/itype.h:231-239`), so a port that divided in `f64` would disagree on some values.
//!
//! And one rule about inheritance the recording pins: the slot **merges** on `copy-from` rather than replacing, because `load_slot` reuses the parent's object
//! (`src/item_factory.cpp:2859-2868`).

#![allow(clippy::panic, clippy::expect_used, clippy::cast_possible_truncation)]

use cdda_content::items::load_paths;
use cdda_json::Value;
use std::collections::BTreeMap;

fn recorded() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/comestibles.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

#[test]
fn every_comestible_has_the_slot_the_reference_has() {
    let Value::Object(root) = recorded() else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    assert_eq!(types.len(), 1_686, "every comestible prototype");
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, _> = load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    let number = |entry: &cdda_json::Object, key: &str| match entry.get(key) {
        Some(Value::Int(value)) => *value,
        Some(Value::Float(value)) => value.trunc() as i64,
        other => panic!("{key}: {other:?}"),
    };
    let text = |entry: &cdda_json::Object, key: &str| match entry.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key}: {other:?}"),
    };

    let mut differing: Vec<String> = Vec::new();
    let mut per_field: BTreeMap<String, usize> = BTreeMap::new();
    let mut with_calories = 0;
    let mut with_spoilage = 0;
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let id = text(entry, "id");
        let Some(item) = loaded.get(&id) else {
            differing.push(format!("{id}: not loaded"));
            continue;
        };
        let Some(slot) = &item.comestible else {
            differing.push(format!("{id}: the port has no comestible slot"));
            continue;
        };
        if slot.calories() > 0 {
            with_calories += 1;
        }
        if slot.spoils_in > 0 {
            with_spoilage += 1;
        }
        let checks: [(&str, String, String); 14] = [
            ("comestible_type", slot.comestible_type.clone(), text(entry, "comestible_type")),
            ("tool", slot.tool.clone(), text(entry, "tool")),
            ("cooks_like", slot.cooks_like.clone(), text(entry, "cooks_like")),
            ("smoking_result", slot.smoking_result.clone(), text(entry, "smoking_result")),
            ("charges", slot.charges.to_string(), number(entry, "charges").to_string()),
            ("stack_size", slot.stack_size.to_string(), number(entry, "stack_size").to_string()),
            ("quench", slot.quench.to_string(), number(entry, "quench").to_string()),
            ("fun", slot.fun.to_string(), number(entry, "fun").to_string()),
            ("stim", slot.stim.to_string(), number(entry, "stim").to_string()),
            ("sleepiness_mod", slot.sleepiness_mod.to_string(), number(entry, "sleepiness_mod").to_string()),
            ("healthy", slot.healthy.to_string(), number(entry, "healthy").to_string()),
            ("parasites", slot.parasites.to_string(), number(entry, "parasites").to_string()),
            ("spoils_in", slot.spoils_in.to_string(), number(entry, "spoils_in").to_string()),
            ("calories", slot.calories().to_string(), number(entry, "calories").to_string()),
        ];
        for (name, ours, theirs) in checks {
            if ours != theirs {
                *per_field.entry(name.to_owned()).or_default() += 1;
                differing.push(format!("{id}.{name}: the port says {ours}, the reference says {theirs}"));
            }
        }
        let ours = slot.nutr().to_string();
        let theirs = number(entry, "nutr").to_string();
        if ours != theirs {
            *per_field.entry("nutr".to_owned()).or_default() += 1;
            differing.push(format!("{id}.nutr: the port says {ours}, the reference says {theirs}"));
        }
    }
    // **Non-vacuity**: a loader that left every field at its default would still have 1,686 slots and would pass an empty corpus check.
    assert!(with_calories > 700, "the corpus carries real nutrition: {with_calories}");
    assert!(with_spoilage > 500, "and real spoilage times: {with_spoilage}");
    for (field, count) in &per_field {
        let examples: Vec<&String> = differing.iter().filter(|line| line.split('.').nth(1).is_some_and(|rest| rest.starts_with(&format!("{field}:")))).take(4).collect();
        eprintln!("{field}: {count} differ, e.g. {examples:#?}");
    }
    assert!(
        differing.is_empty(),
        "{} of {} comestibles disagree, by field {per_field:?}; the first twelve are {:#?}",
        differing.len(),
        types.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}
