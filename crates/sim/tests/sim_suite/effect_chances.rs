//! How likely each effect's per-turn something is, as a percentage.
//!
//! `corpus/effect-chances.json` is `--dump-effect-chances`: for the 112 effect types that state a chance or a tick, every argument they state it for, at four
//! intensities and three values, with the percentage `effect::get_percentage` (`src/effect.cpp:1324-1356`) answers.
//!
//! The rule stacks four things and each one has a shape a reading gets wrong: a value of zero with no stated chance answers **zero**; a stated bottom switches the
//! formula from `100 / top` to `100 * top / bottom`, so a bottom of zero and a top of zero mean opposite things; a tick of **zero means one**, a tick above one
//! divides, and a tick below zero means it cannot happen at all; and the intensity used is the raw one, not the effective one the amount readers use.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // The recording is the authority on the exact bits; a margin of error here would hide the very differences this gate exists to catch.
    clippy::float_cmp,
    clippy::cast_precision_loss
)]

use cdda_content::effects::EffectType;
use cdda_json::Value;
use std::collections::BTreeSet;

fn integer(object: &cdda_json::Object, key: &str) -> i64 {
    match object.get(key) {
        Some(Value::Int(value)) => *value,
        other => panic!("{key}: {other:?}"),
    }
}

fn text(object: &cdda_json::Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

fn number(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(Value::Float(value)) => *value,
        Some(Value::Int(value)) => *value as f64,
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn every_chance_matches_the_percentage_the_reference_answers() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effect-chances.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("types")
    };
    assert_eq!(types.len(), 112, "every effect type that states a chance");
    assert_eq!(integer(&root, "count"), 2_152, "every recorded percentage");

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let effects: std::collections::BTreeMap<String, EffectType> =
        cdda_content::effects::load_paths(&paths).expect("the effects load");

    let mut differing: Vec<String> = Vec::new();
    let mut seen = 0;
    let mut zero_valued = 0;
    let mut zero_percent = 0;
    let mut divided = 0;
    let mut exactly_hundred = 0;
    let mut negative = 0;
    let mut values = BTreeSet::new();
    for recorded in types {
        let Value::Object(recorded) = recorded else {
            panic!("a type")
        };
        let id = text(recorded, "id");
        let Some(kind) = effects.get(&id) else {
            differing.push(format!("{id}: the port does not load it"));
            continue;
        };
        let Some(Value::Array(chances)) = recorded.get("chances") else {
            panic!("chances")
        };
        for chance in chances {
            let Value::Object(chance) = chance else {
                panic!("a chance")
            };
            let argument = text(chance, "arg");
            let intensity = i32::try_from(integer(chance, "intensity")).expect("an intensity");
            let val = i32::try_from(integer(chance, "val")).expect("a value");
            let percentage = number(chance, "percentage");
            seen += 1;
            values.insert(val);
            if val == 0 {
                zero_valued += 1;
            }
            if percentage == 0.0 {
                zero_percent += 1;
            }
            if percentage == 100.0 {
                exactly_hundred += 1;
            }
            if percentage < 0.0 {
                negative += 1;
            }
            if percentage > 0.0 && percentage < 100.0 {
                divided += 1;
            }
            let worn = cdda_sim::effects::ActiveEffect {
                id: id.clone(),
                intensity,
                duration: 1,
                permanent: false,
            };
            let ours = worn.percentage(kind, &argument, val, false);
            // The recording writes seventeen significant digits, so anything beyond a rounding step apart is a real disagreement.
            if (ours - percentage).abs() > 1e-12 {
                differing.push(format!(
                    "{id}.{argument} at intensity {intensity} with a value of {val}: the port says {ours}, the reference says {percentage}"
                ));
            }
        }
    }
    // **Non-vacuity**: the grid has to reach the zero-value rule, the one-in-top shape, and percentages that are neither 100 nor 0.
    assert_eq!(seen, 2_152, "percentages compared: {seen}");
    // The measured shape of the grid, not a guess about it: the zero-value rule fires 233 times, one in ten comes out at exactly a hundred 27 times, most
    // percentages land between the two, and — the part a formula written from memory misses — **158 of them are negative**, because a chance or a bottom can be
    // stated below zero and nothing clamps the answer.
    assert_eq!(zero_valued, 820, "cases whose value is zero");
    assert_eq!(zero_percent, 233, "percentages that came out zero");
    assert_eq!(
        exactly_hundred, 27,
        "percentages of exactly a hundred: {exactly_hundred}"
    );
    assert_eq!(divided, 1_734, "percentages between 0 and 100: {divided}");
    assert_eq!(negative, 158, "negative percentages: {negative}");
    assert_eq!(values.len(), 3, "the values the grid uses: {values:?}");
    assert!(
        differing.is_empty(),
        "{} disagreements over {seen} percentages; the first twelve are {:#?}",
        differing.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}

/// The tick's two special cases, which a flat formula gets wrong: a tick of zero means **one** rather than no chance, and a tick above one divides the chance.
#[test]
fn the_tick_zero_means_one_and_two_halves() {
    use cdda_content::effects::{ModAction, ModTable, ModValues};
    use cdda_sim::effects::ActiveEffect;

    let mut kind = EffectType {
        id: "harness".to_owned(),
        max_intensity: 1,
        max_duration: 3600,
        dur_add_perc: 100,
        int_add_val: 0,
        int_decay_step: -1,
        int_decay_tick: 0,
        int_dur_factor: 0,
        int_decay_remove: false,
        main_parts_only: false,
        show_in_info: false,
        show_intensity: true,
        pain_sizing: false,
        hurt_sizing: false,
        harmful_cough: false,
        impairs_movement: false,
        pkill_addict_reduces: false,
        kill_chance: Vec::new(),
        red_kill_chance: Vec::new(),
        apply_ratings: Vec::new(),
        flags: BTreeSet::new(),
        resist_traits: Vec::new(),
        resist_effects: Vec::new(),
        removes_effects: Vec::new(),
        blocks_effects: Vec::new(),
        mods: ModTable::new(),
    };
    let worn = ActiveEffect {
        id: "harness".to_owned(),
        intensity: 1,
        duration: 1,
        permanent: false,
    };
    let mut cells = std::collections::BTreeMap::new();
    cells.insert(
        (ModAction::ChanceTop, 0),
        ModValues {
            base: 10.0,
            scaling: 0.0,
        },
    );
    kind.mods.insert("X".to_owned(), cells);

    // A tick of zero is one turn: one in ten, every turn.
    assert!(
        (worn.percentage(&kind, "X", 1, false) - 10.0).abs() < 1e-9,
        "one in ten"
    );
    // A tick of two halves it.
    let mut with_tick = kind.clone();
    with_tick.mods.get_mut("X").expect("the group").insert(
        (ModAction::Tick, 0),
        ModValues {
            base: 2.0,
            scaling: 0.0,
        },
    );
    assert!(
        (worn.percentage(&with_tick, "X", 1, false) - 5.0).abs() < 1e-9,
        "one in twenty"
    );
    // A value of zero with a stated chance is the same as any other value; it is only a *missing* chance that answers zero.
    assert!((worn.percentage(&kind, "X", 0, false) - 10.0).abs() < 1e-9);
    // And a missing chance with a **nonzero** value is not zero at all: the guard only covers the zero value, so the formula divides a hundred by a top of zero and
    // answers an infinity. The reference does that too — it is the caller's job to state a chance before passing a value.
    assert_eq!(
        worn.percentage(&kind, "nothing", 0, false),
        0.0,
        "no chance and no value is zero"
    );
    let no_chance = worn.percentage(&kind, "nothing", 1, false);
    assert!(
        no_chance.is_infinite() && no_chance > 0.0,
        "no chance with a value is {no_chance}"
    );
}
