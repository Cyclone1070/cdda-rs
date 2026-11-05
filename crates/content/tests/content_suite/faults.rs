//! Every fault the game defines, field by field against the reference.
//!
//! `corpus/items/faults.json` is `--dump-faults`: all 54 faults, with the name, the description, the fault type, the price modifier, the degradation modifier, the two
//! encumbrance numbers and both modifier lists. It is small enough to compare completely, which is the point — a fault's numbers reach the player through the item's
//! displayed name, its price, its repair ceiling and its melee damage.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::faults::{DamageMod, Fault, load_paths};
use cdda_json::Value;
use std::collections::BTreeMap;

/// Reads the recorded faults.
#[allow(clippy::cast_precision_loss)]
fn real(value: i64) -> f64 {
    value as f64
}

fn dump() -> Vec<Fault> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/faults.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(faults)) = root.get("faults") else {
        panic!("the dump holds faults")
    };
    faults
        .iter()
        .map(|fault| {
            let Value::Object(fault) = fault else {
                panic!("a fault")
            };
            let text = |key: &str| match fault.get(key) {
                Some(Value::Str(value)) => value.clone(),
                other => panic!("{key} is text: {other:?}"),
            };
            let number = |key: &str| match fault.get(key) {
                Some(Value::Float(value)) => *value,
                Some(Value::Int(value)) => real(*value),
                other => panic!("{key} is a number: {other:?}"),
            };
            let whole = |key: &str| match fault.get(key) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("a small number"),
                other => panic!("{key} is a whole number: {other:?}"),
            };
            let ids = |key: &str| match fault.get(key) {
                Some(Value::Array(entries)) => entries
                    .iter()
                    .map(|entry| match entry {
                        Value::Str(id) => id.clone(),
                        other => panic!("{key} holds text: {other:?}"),
                    })
                    .collect::<Vec<String>>(),
                other => panic!("{key} is a list: {other:?}"),
            };
            let mods = |key: &str| match fault.get(key) {
                Some(Value::Array(entries)) => entries
                    .iter()
                    .map(|entry| {
                        let Value::Object(entry) = entry else {
                            panic!("a modifier")
                        };
                        DamageMod {
                            add: match entry.get("add") {
                                Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
                                other => panic!("add is a whole number: {other:?}"),
                            },
                            multiply: match entry.get("multiply") {
                                Some(Value::Float(value)) => *value,
                                other => panic!("multiply is a number: {other:?}"),
                            },
                            damage_id: match entry.get("damage_id") {
                                Some(Value::Str(id)) => id.clone(),
                                other => panic!("damage_id is text: {other:?}"),
                            },
                        }
                    })
                    .collect::<Vec<DamageMod>>(),
                other => panic!("{key} is a list: {other:?}"),
            };
            Fault {
                id: text("id"),
                name: text("name"),
                message: String::new(),
            description: text("description"),
                fault_type: text("type"),
                item_prefix: text("item_prefix"),
                item_suffix: text("item_suffix"),
                price_modifier: number("price_modifier"),
                degradation_mod: whole("degradation_mod"),
                affected_by_degradation: matches!(
                    fault.get("affected_by_degradation"),
                    Some(Value::Bool(true))
                ),
                encumbrance_add: number("encumbrance_add"),
                encumbrance_mult: number("encumbrance_mult"),
                block_faults: ids("block_faults").into_iter().collect(),
                melee_damage_mod: mods("melee_damage_mod"),
                armor_mod: mods("armor_mod"),
            }
        })
        .collect()
}

fn loaded() -> BTreeMap<String, Fault> {
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the faults load")
        .into_iter()
        .map(|fault| (fault.id.clone(), fault))
        .collect()
}

#[test]
fn every_fault_matches_the_reference_number_for_number() {
    let recorded = dump();
    assert_eq!(recorded.len(), 54, "every fault was recorded");
    let faults = loaded();
    let mut differing: Vec<String> = Vec::new();
    for case in &recorded {
        let Some(fault) = faults.get(&case.id) else {
            differing.push(format!("{} is not loaded", case.id));
            continue;
        };
        if fault != case {
            differing.push(format!(
                "{}: the port says {fault:?}, the reference says {case:?}",
                case.id
            ));
        }
    }
    // The gate has to exercise the interesting half: 19 faults adjust melee damage and 9 raise the repair ceiling.
    assert!(
        recorded.iter().filter(|fault| !fault.melee_damage_mod.is_empty()).count() > 15,
        "the melee modifiers have to be exercised"
    );
    assert!(
        recorded
            .iter()
            .filter(|fault| fault.degradation_mod > 0)
            .count()
            > 5,
        "and so does the degradation modifier"
    );
    assert!(
        differing.is_empty(),
        "{} of {} faults disagree; they are {:#?}",
        differing.len(),
        recorded.len(),
        differing
    );
}

/// Reads one recorded fix, field by field.
fn recorded_fix(entry: &Value) -> cdda_content::faults::FaultFix {
    let Value::Object(entry) = entry else {
        panic!("a fix")
    };
    let text = |key: &str| match entry.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key} is text: {other:?}"),
    };
    let whole = |key: &str| match entry.get(key) {
        Some(Value::Int(value)) => *value,
        other => panic!("{key} is a whole number: {other:?}"),
    };
    let ids = |key: &str| match entry.get(key) {
        Some(Value::Array(entries)) => entries
            .iter()
            .map(|id| match id {
                Value::Str(id) => id.clone(),
                other => panic!("{key} holds text: {other:?}"),
            })
            .collect::<std::collections::BTreeSet<String>>(),
        other => panic!("{key} is a list: {other:?}"),
    };
    let skills = match entry.get("skills") {
        Some(Value::Object(skills)) => skills
            .members()
            .iter()
            .filter_map(|(skill, level)| match level {
                Value::Int(level) => Some((skill.clone(), i32::try_from(*level).ok()?)),
                _ => None,
            })
            .collect::<BTreeMap<String, i32>>(),
        other => panic!("skills is an object: {other:?}"),
    };
    cdda_content::faults::FaultFix {
        id: text("id"),
        name: text("name"),
        success_msg: text("success_msg"),
        time_seconds: whole("time_seconds"),
        #[allow(clippy::cast_possible_truncation)]
        mod_damage: whole("mod_damage") as i32,
        #[allow(clippy::cast_possible_truncation)]
        mod_degradation: whole("mod_degradation") as i32,
        faults_removed: ids("faults_removed"),
        faults_added: ids("faults_added"),
        skills,
    }
}

#[test]
fn every_fault_fix_matches_the_reference() {
    // 40 fixes, each with a time, a set of skills, the faults it moves and the damage and degradation it changes. Small enough to compare completely.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/fault-fixes.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(fixes)) = root.get("fixes") else {
        panic!("the dump holds fixes")
    };
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, cdda_content::faults::FaultFix> =
        cdda_content::faults::load_fixes(&paths)
            .expect("the fixes load")
            .into_iter()
            .map(|fix| (fix.id.clone(), fix))
            .collect();
    assert_eq!(fixes.len(), 40, "every fix was recorded");

    let mut differing: Vec<String> = Vec::new();
    let mut with_skills = 0;
    let mut adding = 0;
    for entry in fixes {
        let expected = recorded_fix(entry);
        let id = expected.id.clone();
        let Some(fix) = loaded.get(&id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        if !fix.skills.is_empty() {
            with_skills += 1;
        }
        if !fix.faults_added.is_empty() {
            adding += 1;
        }
        if fix != &expected {
            differing.push(format!("{id}: the port says {fix:?}, the reference says {expected:?}"));
        }
    }
    assert!(with_skills > 10, "the skill requirement has to be exercised: {with_skills}");
    assert!(adding > 0, "and so does the fix that adds a fault");
    assert!(
        differing.is_empty(),
        "{} of 40 fixes disagree; they are {:#?}",
        differing.len(),
        differing
    );
}
