//! Every `consume_drug` use action, resolved: 108 of them, each a list of consequences.
//!
//! `corpus/items/consume-drugs.json` is `--dump-consume-drugs`. The actor is `consume_drug_iuse` (`src/iuse_actor.h:247-283`), and almost every field is a
//! collection because taking a drug does several things at once: effects with durations and body parts, fields in the world, stat adjustments, vitamins as a range
//! to roll between, and the item left behind.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::{ConsumeDrug, DrugDamage, DrugEffect, UseAction};
use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Reads one recorded drug.
fn recorded_drug(entry: &Value) -> ConsumeDrug {
    let Value::Object(entry) = entry else {
        panic!("a drug")
    };
    let text = |key: &str| match entry.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key} is text: {other:?}"),
    };
    let int_map = |key: &str| match entry.get(key) {
        Some(Value::Object(map)) => map
            .members()
            .iter()
            .filter_map(|(id, value)| match value {
                Value::Int(value) => Some((id.clone(), i32::try_from(*value).ok()?)),
                _ => None,
            })
            .collect::<BTreeMap<String, i32>>(),
        other => panic!("{key} is an object: {other:?}"),
    };
    let list = |key: &str| match entry.get(key) {
        Some(Value::Array(entries)) => entries.clone(),
        other => panic!("{key} is a list: {other:?}"),
    };
    ConsumeDrug {
        action: text("action"),
        moves: match entry.get("moves") {
            Some(Value::Int(moves)) => i32::try_from(*moves).expect("a small number"),
            other => panic!("moves: {other:?}"),
        },
        activation_message: text("activation_message"),
        used_up_item: text("used_up_item"),
        fields_produced: int_map("fields_produced"),
        stat_adjustments: int_map("stat_adjustments"),
        charges_needed: int_map("charges_needed"),
        tools_needed: int_map("tools_needed"),
        vitamins: recorded_vitamins(&list("vitamins")),
        effects: recorded_effects(&list("effects")),
        damage_over_time: recorded_damage(&list("damage_over_time")),
    }
}

/// Reads the recorded vitamins.
fn recorded_vitamins(entries: &[Value]) -> BTreeMap<String, (i32, i32)> {
    entries
        .iter()
        .map(|vitamin| {
            let Value::Object(vitamin) = vitamin else {
                panic!("a vitamin")
            };
            let int = |key: &str| match vitamin.get(key) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("a small number"),
                other => panic!("{key} is a whole number: {other:?}"),
            };
            (
                match vitamin.get("id") {
                    Some(Value::Str(id)) => id.clone(),
                    other => panic!("a vitamin id: {other:?}"),
                },
                (int("lo"), int("hi")),
            )
        })
        .collect()
}

/// Reads the recorded effects.
fn recorded_effects(entries: &[Value]) -> Vec<DrugEffect> {
    entries
        .iter()
        .map(|effect| {
            let Value::Object(effect) = effect else {
                panic!("an effect")
            };
            DrugEffect {
                id: match effect.get("id") {
                    Some(Value::Str(id)) => id.clone(),
                    other => panic!("an effect id: {other:?}"),
                },
                duration_seconds: match effect.get("duration_seconds") {
                    Some(Value::Int(seconds)) => *seconds,
                    other => panic!("a duration: {other:?}"),
                },
                bp: match effect.get("bp") {
                    Some(Value::Str(bp)) => bp.clone(),
                    other => panic!("a body part: {other:?}"),
                },
                permanent: matches!(effect.get("permanent"), Some(Value::Bool(true))),
            }
        })
        .collect()
}

/// Reads the recorded damage over time.
fn recorded_damage(entries: &[Value]) -> Vec<DrugDamage> {
    entries
        .iter()
        .map(|dot| {
            let Value::Object(dot) = dot else {
                panic!("damage over time")
            };
            DrugDamage {
                damage_type: match dot.get("type") {
                    Some(Value::Str(kind)) => kind.clone(),
                    other => panic!("a damage type: {other:?}"),
                },
                duration_seconds: match dot.get("duration_seconds") {
                    Some(Value::Int(seconds)) => *seconds,
                    other => panic!("a duration: {other:?}"),
                },
                amount: match dot.get("amount") {
                    Some(Value::Float(amount)) => *amount,
                    Some(Value::Int(amount)) => {
                        #[allow(clippy::cast_precision_loss)]
                        let amount = *amount as f64;
                        amount
                    }
                    other => panic!("an amount: {other:?}"),
                },
                bps: match dot.get("bps") {
                    Some(Value::Array(parts)) => parts
                        .iter()
                        .map(|part| match part {
                            Value::Str(part) => part.clone(),
                            other => panic!("a body part: {other:?}"),
                        })
                        .collect(),
                    other => panic!("bps is a list: {other:?}"),
                },
            }
        })
        .collect()
}

/// Reads the recorded actors, keyed by the item type that declares them.
fn dump() -> BTreeMap<String, ConsumeDrug> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/consume-drugs.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    let mut out = BTreeMap::new();
    for entry in types {
        let Value::Object(object) = entry else {
            panic!("a drug")
        };
        let Some(Value::Str(id)) = object.get("id") else {
            panic!("a drug id")
        };
        out.insert(id.clone(), recorded_drug(entry));
    }
    out
}

#[test]
fn every_drug_is_read_the_way_the_reference_reads_it() {
    let recorded = dump();
    assert_eq!(recorded.len(), 108, "every drug was recorded");
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, ItemType> = load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    // What the corpus exercises: a vitamin range wider than one, a tool that costs nothing, an item left behind, a body part other than the null one.
    let mut graded_range = 0;
    let mut free_tool = 0;
    let mut left_behind = 0;
    let mut real_body_part = 0;
    let mut field_making = 0;
    for drug in recorded.values() {
        if drug.vitamins.values().any(|(lo, hi)| lo != hi) {
            graded_range += 1;
        }
        if drug.tools_needed.values().any(|count| *count < 0) {
            free_tool += 1;
        }
        if drug.used_up_item != "null" && !drug.used_up_item.is_empty() {
            left_behind += 1;
        }
        if drug.effects.iter().any(|effect| effect.bp != "bp_null") {
            real_body_part += 1;
        }
        if !drug.fields_produced.is_empty() {
            field_making += 1;
        }
    }

    let mut differing: Vec<String> = Vec::new();
    for (id, expected) in &recorded {
        let Some(item) = loaded.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let found: Vec<&ConsumeDrug> = item
            .use_actions
            .values()
            .filter_map(|action| match action {
                UseAction::ConsumeDrug(drug) => Some(drug),
                _ => None,
            })
            .collect();
        if found.first().is_none_or(|drug| *drug != expected) {
            differing.push(format!(
                "{id}: the port says {found:?}, the reference says {expected:?}"
            ));
        }
    }
    assert!(
        graded_range > 10,
        "a vitamin range has to be exercised: {graded_range}"
    );
    assert!(
        free_tool > 5,
        "and a tool that costs nothing (a negative count): {free_tool}"
    );
    assert!(left_behind > 0, "and the item left behind: {left_behind}");
    // **Declared gap, asserted rather than assumed**: every effect in this content names `bp_null`, and none is permanent. Both branches exist in the actor and the
    // port reads them, but this corpus does not exercise them — so the counts are pinned at zero, and the day the content grows one the test says so instead of
    // quietly passing.
    assert_eq!(
        real_body_part, 0,
        "no effect names a body part in this content"
    );
    assert_eq!(
        recorded
            .values()
            .flat_map(|drug| drug.effects.iter())
            .filter(|effect| effect.permanent)
            .count(),
        0,
        "and none is permanent"
    );
    assert!(field_making > 0, "and a field produced: {field_making}");
    assert!(
        differing.is_empty(),
        "{} of {} drugs disagree; the first ten are {:#?}",
        differing.len(),
        recorded.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

#[test]
fn the_ported_actor_types_are_the_ones_the_content_uses() {
    // A cheap cross-check that keeps the actor dispatch honest: every action the port recognises by **name only** must be a type the content really uses, and the
    // three ported ones must appear as their own variants rather than as `Other`.
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let items = load_paths(&paths).expect("the items load");
    let mut ported: BTreeSet<&str> = BTreeSet::new();
    let mut others: BTreeMap<String, usize> = BTreeMap::new();
    for item in &items {
        for action in item.use_actions.values() {
            match action {
                UseAction::Other { kind } => {
                    *others.entry(kind.clone()).or_default() += 1;
                }
                ported_action => {
                    ported.insert(ported_action.kind());
                }
            }
        }
    }
    assert!(
        ported.contains("transform"),
        "the port has the transform actor"
    );
    assert!(
        ported.contains("delayed_transform"),
        "and its delayed variant"
    );
    assert!(ported.contains("consume_drug"), "and the drug");
    // **`consume_drug` must never arrive as `Other`**: a regression there would silently drop every drug's consequences while the drug gate still passed.
    assert!(
        !others.contains_key("consume_drug"),
        "consume_drug is ported, so it cannot be an unported name"
    );
    assert!(
        others.len() > 20,
        "the content still uses many actors: {}",
        others.len()
    );
}
