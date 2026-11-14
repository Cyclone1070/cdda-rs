//! Whether an effect's per-turn something fires this turn: the chance, the clock and the die.
//!
//! `corpus/effect-activated.json` is `--dump-effect-activated`: 112 effect types × their chancy arguments × four intensities × three values × two modifiers × three
//! turns, with the die the reference rolled, whether it rolled at all, and the answer it gave. The port replays the **same die**, so a disagreement is a rule that
//! differs rather than two independent draws that happened to land differently.
//!
//! The rule is `effect::activated` (`src/effect.cpp:1360-1394`): a zero value with no stated chance never fires, a negative tick never fires, only the turns the tick
//! divides can fire, and then one uniform float is compared against `mod / top` or `top * mod / bot`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use cdda_content::effects::EffectType;
use cdda_core_types::rng::{Draw, Recording, Rolls};
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
        Some(Value::Int(value)) => {
            #[allow(clippy::cast_precision_loss)]
            let value = *value as f64;
            value
        }
        other => panic!("{key}: {other:?}"),
    }
}

fn boolean(object: &cdda_json::Object, key: &str) -> bool {
    match object.get(key) {
        Some(Value::Bool(value)) => *value,
        other => panic!("{key}: {other:?}"),
    }
}

/// Replays one recorded uniform float through the port's own dice, and reports whether it was taken.
///
/// The reference's `x_in_y` is `rng_float( 0, 1 ) <= x / y`, so one float decides a case and a case that never reaches the comparison takes none. The count of draws
/// left afterwards is therefore a second assertion the recording carries: a rule that rolls when it should not is caught even if the answer happens to match.
fn replay(roll: f64) -> (Rolls, Recording) {
    let recording = Recording {
        draws: vec![Draw::Real {
            low: 0.0,
            high: 1.0,
            drawn: roll,
        }],
        position: 0,
        engine_draws: Vec::new(),
    };
    (Rolls::replaying(recording.clone()), recording)
}

#[test]
fn every_activation_matches_the_reference_with_the_same_die() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effect-activated.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("cases")
    };
    assert_eq!(cases.len(), 14_760, "every recorded case");
    assert_eq!(
        integer(&root, "fired"),
        1_797,
        "the reference fired on this many"
    );
    assert_eq!(
        integer(&root, "rolled"),
        9_124,
        "and rolled a die on this many"
    );

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let effects: std::collections::BTreeMap<String, EffectType> =
        cdda_content::effects::load_paths(&paths).expect("the effects load");

    let mut differing: Vec<String> = Vec::new();
    let mut fired = 0;
    let mut rolled = 0;
    let mut turns = BTreeSet::new();
    let mut modifiers = BTreeSet::new();
    for case in cases {
        let Value::Object(case) = case else {
            panic!("a case")
        };
        let id = text(case, "id");
        let Some(kind) = effects.get(&id) else {
            differing.push(format!("{id}: the port does not load it"));
            continue;
        };
        let argument = text(case, "arg");
        let intensity = i32::try_from(integer(case, "intensity")).expect("an intensity");
        let value = i32::try_from(integer(case, "val")).expect("a value");
        let modifier = number(case, "mod");
        let turn = integer(case, "turn");
        let roll = number(case, "roll");
        let dice = integer(case, "dice");
        let answer = boolean(case, "answer");
        turns.insert(turn);
        modifiers.insert(format!("{modifier}"));
        let worn = cdda_sim::effects::ActiveEffect {
            id: id.clone(),
            intensity,
            duration: 1,
            permanent: false,
        };
        let (mut rolls, _recording) = replay(roll);
        let ours = worn.activated(kind, &mut rolls, turn, &argument, value, false, modifier);
        let took_a_die = rolls.recording_remaining() == Some(0);
        if ours {
            fired += 1;
        }
        if took_a_die {
            rolled += 1;
        }
        if ours != answer {
            differing.push(format!(
                "{id}.{argument} at intensity {intensity}, value {value}, mod {modifier}, turn {turn}, roll {roll}: the port says {ours}, the reference says {answer}"
            ));
        } else if took_a_die != (dice == 1) {
            differing.push(format!(
                "{id}.{argument} at intensity {intensity}, value {value}, mod {modifier}, turn {turn}: the port {} a die and the reference {} one",
                if took_a_die { "took" } else { "took no" },
                if dice == 1 { "took" } else { "took no" }
            ));
        }
    }
    // **Non-vacuity**: the grid has to fire, to roll, and to reach more than one turn and more than one modifier.
    assert_eq!(fired, 1_797, "cases the port fired on: {fired}");
    assert_eq!(rolled, 9_124, "cases the port rolled on: {rolled}");
    assert_eq!(turns.len(), 3, "turns the grid uses: {turns:?}");
    assert_eq!(modifiers.len(), 2, "modifiers the grid uses: {modifiers:?}");
    assert!(
        differing.is_empty(),
        "{} disagreements over 14760 cases; the first twelve are {:#?}",
        differing.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}
