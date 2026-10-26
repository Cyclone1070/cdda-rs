//! Every effect type, field by field, against the reference's own registry.
//!
//! `corpus/effects.json` is `--dump-effects`: for all 564 effect types the reference holds, the twenty-odd members a rule can read. The family is a plain
//! `std::map` rather than a `generic_factory`, so it is the one content family the prototype digest could not see on its own — the harness now appends it by hand.
//!
//! Two of the 564 do not come from data at all: `register_ma_buff_effect` (`src/effect.cpp:1674-1680`) mints an effect for every martial-art buff, so the port's
//! loader — which reads JSON — must produce exactly the rest and those must be exactly the ones it lacks.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use cdda_content::effects::{EffectType, Rating};
use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn read() -> (BTreeMap<String, Value>, usize) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effects.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("types")
    };
    let mut out = BTreeMap::new();
    for entry in types {
        let Value::Object(object) = entry else { panic!("a type") };
        let Some(Value::Str(id)) = object.get("id") else {
            panic!("an id")
        };
        out.insert(id.clone(), entry.clone());
    }
    (out, types.len())
}

fn integer(object: &cdda_json::Object, key: &str) -> i64 {
    match object.get(key) {
        Some(Value::Int(number)) => *number,
        other => panic!("{key}: {other:?}"),
    }
}

fn boolean(object: &cdda_json::Object, key: &str) -> bool {
    match object.get(key) {
        Some(Value::Bool(value)) => *value,
        other => panic!("{key}: {other:?}"),
    }
}

fn list(object: &cdda_json::Object, key: &str) -> Vec<String> {
    match object.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::Str(text) => text.clone(),
                other => panic!("{key}: {other:?}"),
            })
            .collect(),
        other => panic!("{key}: {other:?}"),
    }
}

/// Compares one field and records the disagreement rather than panicking, so one bad type does not hide the rest.
#[allow(clippy::needless_pass_by_value)]
fn check<T: PartialEq + std::fmt::Debug>(
    differing: &mut Vec<String>,
    id: &str,
    field: &str,
    ours: T,
    theirs: T,
) {
    if ours != theirs {
        differing.push(format!("{id}.{field}: the port says {ours:?}, the reference says {theirs:?}"));
    }
}

#[test]
fn every_effect_type_matches_the_reference_field_by_field() {
    let (recorded, total) = read();
    assert_eq!(total, 564, "every effect type the reference holds");

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let effects: BTreeMap<String, EffectType> =
        cdda_content::effects::load_paths(&paths).expect("the effects load");
    // 451, not the 453 objects the core data states: two ids are stated **twice**, and the reference's map keeps the last one it read. (Across the whole data tree,
    // mods included, the same count is 1,428 objects for 1,350 ids.)
    assert_eq!(effects.len(), 451, "every effect type the core data states");

    // The ones the reference holds and the data does not: one per martial-art buff, minted rather than loaded.
    let generated: BTreeSet<&String> = recorded
        .keys()
        .filter(|id| !effects.contains_key(*id))
        .collect();
    assert_eq!(
        generated.len(),
        113,
        "the reference's registry holds {} more than the data states",
        generated.len()
    );
    // And they are all of one kind: a martial-art buff's effect is minted by `register_ma_buff_effect`, not loaded, and its id says so.
    assert!(
        generated.iter().all(|id| id.starts_with("mabuff:")),
        "the generated effects are all martial-art buffs: {:?}",
        generated.iter().take(5).collect::<Vec<_>>()
    );

    let mut differing: Vec<String> = Vec::new();
    let mut with_flags = 0;
    let mut with_resists = 0;
    let mut rated = BTreeSet::new();
    let mut decaying = 0;
    for (id, reference) in &recorded {
        let Value::Object(reference) = reference else {
            panic!("a type")
        };
        let Some(ours) = effects.get(id) else {
            // Declared: a martial-art buff's effect. Checked below for its shape rather than its fields.
            continue;
        };
        check(
            &mut differing,
            id,
            "max_intensity",
            i64::from(ours.max_intensity),
            integer(reference, "max_intensity"),
        );
        check(&mut differing, id, "max_duration", ours.max_duration, integer(reference, "max_duration"));
        check(
            &mut differing,
            id,
            "dur_add_perc",
            i64::from(ours.dur_add_perc),
            integer(reference, "dur_add_perc"),
        );
        check(
            &mut differing,
            id,
            "int_add_val",
            i64::from(ours.int_add_val),
            integer(reference, "int_add_val"),
        );
        check(
            &mut differing,
            id,
            "int_decay_step",
            i64::from(ours.int_decay_step),
            integer(reference, "int_decay_step"),
        );
        check(
            &mut differing,
            id,
            "int_decay_tick",
            i64::from(ours.int_decay_tick),
            integer(reference, "int_decay_tick"),
        );
        check(&mut differing, id, "int_dur_factor", ours.int_dur_factor, integer(reference, "int_dur_factor"));
        check(
            &mut differing,
            id,
            "int_decay_remove",
            ours.int_decay_remove,
            boolean(reference, "int_decay_remove"),
        );
        check(
            &mut differing,
            id,
            "main_parts_only",
            ours.main_parts_only,
            boolean(reference, "main_parts_only"),
        );
        check(&mut differing, id, "show_in_info", ours.show_in_info, boolean(reference, "show_in_info"));
        check(
            &mut differing,
            id,
            "show_intensity",
            ours.show_intensity,
            boolean(reference, "show_intensity"),
        );
        check(&mut differing, id, "pain_sizing", ours.pain_sizing, boolean(reference, "pain_sizing"));
        check(&mut differing, id, "hurt_sizing", ours.hurt_sizing, boolean(reference, "hurt_sizing"));
        check(
            &mut differing,
            id,
            "harmful_cough",
            ours.harmful_cough,
            boolean(reference, "harmful_cough"),
        );
        check(
            &mut differing,
            id,
            "impairs_movement",
            ours.impairs_movement,
            boolean(reference, "impairs_movement"),
        );
        check(
            &mut differing,
            id,
            "pkill_addict_reduces",
            ours.pkill_addict_reduces,
            boolean(reference, "pkill_addict_reduces"),
        );
        // The rating at intensity one, which is what the reference's `get_rating()` defaults to.
        check(
            &mut differing,
            id,
            "rating",
            ours.rating(1).value(),
            i32::try_from(integer(reference, "rating")).expect("a rating"),
        );
        check(
            &mut differing,
            id,
            "flags",
            ours.flags.iter().cloned().collect::<Vec<String>>(),
            {
                let mut theirs = list(reference, "flags");
                theirs.sort();
                theirs
            },
        );
        check(&mut differing, id, "resist_traits", ours.resist_traits.clone(), list(reference, "resist_traits"));
        check(
            &mut differing,
            id,
            "resist_effects",
            ours.resist_effects.clone(),
            list(reference, "resist_effects"),
        );
        check(
            &mut differing,
            id,
            "removes_effects",
            ours.removes_effects.clone(),
            list(reference, "removes_effects"),
        );
        check(
            &mut differing,
            id,
            "blocks_effects",
            ours.blocks_effects.clone(),
            list(reference, "blocks_effects"),
        );

        if !ours.flags.is_empty() {
            with_flags += 1;
        }
        if !ours.resist_traits.is_empty() || !ours.resist_effects.is_empty() {
            with_resists += 1;
        }
        rated.insert(ours.rating(1).value());
        if ours.int_dur_factor != 0 {
            decaying += 1;
        }
    }
    // **Non-vacuity**: the fields that matter have to be *stated* by a real share of the content, or the comparison is over defaults.
    assert!(with_flags > 60, "types with flags: {with_flags}");
    assert!(with_resists > 50, "types something resists: {with_resists}");
    assert!(rated.len() >= 4, "distinct ratings: {rated:?}");
    assert!(decaying > 30, "types whose intensity has a duration: {decaying}");
    assert!(
        Rating::Good.value() != Rating::Neutral.value() && Rating::Bad.value() != Rating::Neutral.value(),
        "the ratings are distinct values"
    );
    assert!(
        differing.is_empty(),
        "{} disagreements over {} types; the first twelve are {:#?}",
        differing.len(),
        effects.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}
