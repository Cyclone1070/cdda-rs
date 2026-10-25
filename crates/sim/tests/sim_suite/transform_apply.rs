//! What a `transform` action does to its item, over all 459 of them.
//!
//! `corpus/items/transform-applies.json` is `--dump-transform-applies`: for each type that carries a `transform`, the reference builds the item, hands it the
//! actor's `do_transform`, and reports what it became. That is the item-level half of the action, and it is the half with the surprises — a liquid result turns the
//! item **active** because liquids have a temperature, a result whose type has a `countdown_interval` arms a timer the actor never mentions, and the charges survive
//! only while the count-by-charges-ness does.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::UseAction;
use cdda_json::Value;
use cdda_sim::transform;
use std::collections::BTreeMap;

/// The countdown the reference reports for "no countdown": it subtracts from an unset point and saturates.
const NO_COUNTDOWN: i64 = 2_147_483_647;

fn recorded() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/transform-applies.json");
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

fn int(entry: &cdda_json::Object, key: &str) -> i64 {
    match entry.get(key) {
        Some(Value::Int(number)) => *number,
        other => panic!("{key}: {other:?}"),
    }
}

fn boolean(entry: &cdda_json::Object, key: &str) -> bool {
    match entry.get(key) {
        Some(Value::Bool(flag)) => *flag,
        other => panic!("{key}: {other:?}"),
    }
}

fn text(entry: &cdda_json::Object, key: &str) -> String {
    match entry.get(key) {
        Some(Value::Str(text)) => text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

/// Whether the reference considers a type to have a temperature, which is what turns a liquid result **active**.
///
/// Reference: `item::has_temperature` (`src/item.cpp:10091-10094`): `( is_comestible() && !has_flag( NO_TEMP ) ) || is_corpse()`. The corpse half is **declared**:
/// the port has no corpse type yet, and no transform in the content produces one, so the gate would be claiming something it never exercises if it pretended to
/// check it.
fn has_temperature(target: &ItemType) -> bool {
    !target.comestible_type.is_empty() && !target.flags.contains("NO_TEMP")
}

#[test]
fn every_transform_does_to_its_item_what_the_reference_does() {
    let Value::Object(root) = recorded() else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    assert_eq!(types.len(), 459, "every transform was recorded");
    let items = loaded();

    let mut differing: Vec<String> = Vec::new();
    let mut active = 0;
    let mut timers = 0;
    let mut declared = 0;
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let id = text(entry, "id");
        let action = text(entry, "action");
        let Some(item) = items.get(&id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let Some(UseAction::Transform(actor)) = item.use_actions.get(&action) else {
            differing.push(format!("{id}: the port has no {action}"));
            continue;
        };
        let result_kind = text(entry, "result");
        let Some(target) = items.get(&result_kind) else {
            differing.push(format!("{id}: the reference's result {result_kind} is not loaded"));
            continue;
        };
        let before = transform::Before {
            kind: id.clone(),
            charges: i32::try_from(int(entry, "charges_before")).expect("a charge count"),
            count_by_charges: item.counts_by_charge(),
        };
        let facts = transform::Target {
            kind: result_kind.clone(),
            count_by_charges: target.counts_by_charge(),
            default_charges: target.default_charges,
            countdown_interval_seconds: target.countdown_interval_seconds,
            has_temperature: has_temperature(target),
            phase: target.phase.clone(),
        };
        // The key is the **action's** name, because a type may carry two transforms and they are different behaviours.
        let Ok(after) = transform::apply(actor, &before, &facts, 0) else {
            declared += 1;
            continue;
        };
        if after.active {
            active += 1;
        }
        if after.countdown_at.is_some() {
            timers += 1;
        }
        let expected_countdown = match int(entry, "countdown_after") {
            NO_COUNTDOWN => None,
            turns => Some(turns),
        };
        let mismatches: Vec<String> = [
            (after.kind != result_kind, format!("kind {} != {result_kind}", after.kind)),
            (
                i64::from(after.charges) != int(entry, "charges_after"),
                format!("charges {} != {}", after.charges, int(entry, "charges_after")),
            ),
            (
                after.count_by_charges != boolean(entry, "cbc_after"),
                format!("count_by_charges {} != {}", after.count_by_charges, boolean(entry, "cbc_after")),
            ),
            (
                after.active != boolean(entry, "active_after"),
                format!("active {} != {}", after.active, boolean(entry, "active_after")),
            ),
            (
                after.countdown_at != expected_countdown,
                format!("countdown {:?} != {expected_countdown:?}", after.countdown_at),
            ),
            (
                i64::from(after.item_counter) != int(entry, "item_counter_after"),
                format!("item_counter {} != {}", after.item_counter, int(entry, "item_counter_after")),
            ),
        ]
        .into_iter()
        .filter_map(|(wrong, message)| wrong.then_some(message))
        .collect();
        if !mismatches.is_empty() {
            differing.push(format!("{id}: {}", mismatches.join("; ")));
        }
    }
    // **Non-vacuity.** A rule that always answered "inactive, no timer" would pass a gate whose fixtures were all inactive, so the corpus has to reach both.
    assert!(active > 100, "most transforms end active for one of three reasons: {active}");
    assert!(timers > 40, "and a fair number start a timer: {timers}");
    // **The declared count is asserted, not just printed.** Four transforms put their result inside a container, which is a shape the port does not perform yet; if
    // that number ever grows the gate has stopped covering something without saying so.
    assert_eq!(declared, 4, "four transforms are the container shape the port declares");
    assert!(
        differing.is_empty(),
        "{} of 459 transforms disagree; the first ten are {:#?}",
        differing.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}
