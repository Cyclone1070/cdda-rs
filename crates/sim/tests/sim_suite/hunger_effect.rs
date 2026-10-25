//! Which hunger effect a body wears, against a recorded grid of 126 character states.
//!
//! `corpus/characters/hunger-effect.json` is `--dump-hunger-effect`: seven fillings of a 250 mL stomach from empty to full, three ages of the last bite either side of
//! the ten-minute and three-hour lines, and six body-fat levels from emaciated to obese — with the reference's own answers for the body-fat figure, the calorie
//! deficit, the two volume tests, and the effect that ended up installed.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use cdda_json::Value;
use cdda_sim::body::{
    choose_hunger_effect, has_calorie_deficit, would_be_engorged_with, would_be_full_with,
};
use std::collections::BTreeMap;

/// One recorded case.
struct Case {
    minutes_since_ate: i64,
    stored_kcal: i64,
    bmi_fat: f32,
    calorie_deficit: bool,
    engorged: bool,
    full: bool,
    contains_ml: i64,
    guts_kcal: i64,
    effect: String,
}

fn float(object: &cdda_json::Object, key: &str) -> f32 {
    match object.get(key) {
        Some(Value::Float(number)) => *number as f32,
        Some(Value::Int(number)) => *number as f32,
        other => panic!("{key}: {other:?}"),
    }
}

fn integer(object: &cdda_json::Object, key: &str) -> i64 {
    match object.get(key) {
        Some(Value::Int(number)) => *number,
        Some(Value::Float(number)) => *number as i64,
        other => panic!("{key}: {other:?}"),
    }
}

fn flag(object: &cdda_json::Object, key: &str) -> bool {
    match object.get(key) {
        Some(Value::Bool(value)) => *value,
        other => panic!("{key}: {other:?}"),
    }
}

fn recorded() -> (i64, f32, i64, Vec<Case>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/characters/hunger-effect.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let height = integer(&root, "height_cm");
    let organic = float(&root, "organic_size");
    let capacity = integer(&root, "capacity_ml");
    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("cases")
    };
    let cases = cases
        .iter()
        .map(|case| {
            let Value::Object(case) = case else { panic!("a case") };
            let effect = match case.get("effect") {
                Some(Value::Str(effect)) => effect.clone(),
                other => panic!("{other:?}"),
            };
            Case {
                minutes_since_ate: integer(case, "minutes_since_ate"),
                stored_kcal: integer(case, "stored_kcal"),
                bmi_fat: float(case, "bmi_fat"),
                calorie_deficit: flag(case, "calorie_deficit"),
                engorged: flag(case, "engorged"),
                full: flag(case, "full"),
                contains_ml: integer(case, "contains_ml"),
                guts_kcal: integer(case, "guts_kcal"),
                effect,
            }
        })
        .collect();
    (height, organic, capacity, cases)
}

#[test]
fn every_state_wears_the_hunger_effect_the_reference_chooses() {
    let (height, organic, capacity, cases) = recorded();
    assert_eq!(cases.len(), 126, "every recorded state");

    let mut differing: Vec<String> = Vec::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for case in &cases {
        let label = format!(
            "ate{}:kcal{}:ml{}",
            case.minutes_since_ate, case.stored_kcal, case.contains_ml
        );
        // The body-fat figure and everything read off it, from the port's own arithmetic.
        let bmi = cdda_sim::body::bmi_fat(case.stored_kcal, height, organic);
        if (bmi - case.bmi_fat).abs() > 1e-4 {
            differing.push(format!(
                "{label}.bmi_fat: the port says {bmi:.6}, the reference says {:.6}",
                case.bmi_fat
            ));
            continue;
        }
        let deficit = has_calorie_deficit(bmi);
        if deficit != case.calorie_deficit {
            differing.push(format!(
                "{label}.calorie_deficit: the port says {deficit}, the reference says {}",
                case.calorie_deficit
            ));
        }
        let engorged = would_be_engorged_with(case.contains_ml, 0, capacity, deficit);
        if engorged != case.engorged {
            differing.push(format!(
                "{label}.engorged: the port says {engorged}, the reference says {}",
                case.engorged
            ));
        }
        let full = would_be_full_with(case.contains_ml, 0, capacity, deficit);
        if full != case.full {
            differing.push(format!(
                "{label}.full: the port says {full}, the reference says {}",
                case.full
            ));
        }
        let effect = choose_hunger_effect(
            case.contains_ml,
            capacity,
            case.minutes_since_ate * 60,
            bmi,
            case.guts_kcal,
        );
        if effect != case.effect {
            differing.push(format!(
                "{label}.effect: the port says {effect}, the reference says {}",
                case.effect
            ));
        }
        *seen.entry(effect).or_insert(0) += 1;
    }
    // **Non-vacuity**: seven rungs of the ladder have to be reached, and two never can be. Both of those are dead code in the reference rather than a gap in the grid,
    // and a change that made them reachable would be a behaviour change worth failing on.
    assert_eq!(seen.get("hunger_engorged").copied().unwrap_or(0), 18, "{seen:?}");
    assert_eq!(seen.get("hunger_hungry").copied().unwrap_or(0), 14, "{seen:?}");
    assert!(
        seen.get("hunger_famished").copied().unwrap_or(0) > 0,
        "the famished rung is reached: {seen:?}"
    );
    assert!(
        !seen.contains_key("hunger_full") && !seen.contains_key("hunger_starving"),
        "the two unreachable rungs stayed unreachable: {seen:?}"
    );
    assert!(
        differing.is_empty(),
        "{} disagreements over {} states; the first twelve are {:#?}",
        differing.len(),
        cases.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}

/// The state machine around the ladder: exactly one hunger effect at a time, and one already worn is **not** refreshed.
///
/// Reference: `src/character_body.cpp:1083-1094` — `if( !has_effect( hunger_effect ) )` guards a block that removes all nine and adds the chosen one for 24 hours. So a
/// character already wearing the right effect keeps the duration it had, which matters when the same choice comes up turn after turn.
#[test]
fn the_effect_is_replaced_only_when_it_changes() {
    use cdda_sim::body::{Body, BodyInputs};
    use cdda_sim::digest::MetabolicRates;
    use std::collections::BTreeMap;

    let vitamins = BTreeMap::new();
    let rates = MetabolicRates {
        kcal: 2_000.0,
        hunger: 1.0,
        thirst: 1.0,
        sleepiness: 0.0,
    };
    let inputs = BodyInputs {
        five_mins: 0,
        half_hours: 0,
        kcal_per_time: 0.0,
        healthy_kcal: 118_153,
        hunger_roll: 0,
        thirst_roll: 0,
        time_since_ate: 0,
        foodless: false,
        no_thirst: false,
        mycus: false,
        height_cm: 175,
        organic_size: 1.0,
    };

    // An empty stomach at 4.0 of body fat, just eaten: the reference's ladder answers `hunger_blank` — a character with no calorie deficit and nothing in the stomach
    // is not judged hungry, it is judged blank. (At zero calories the same state would answer `hunger_hungry`, because the deficit ladder reads an empty stomach as hunger.)
    let mut body = Body::new(250);
    body.stored_calories = 118_153 * 1_000;
    body.hunger_effect = Some("hunger_engorged".to_owned());
    body.update_stomach(&rates, &inputs, &vitamins);
    assert_eq!(body.hunger_effect.as_deref(), Some("hunger_blank"), "the old effect is gone");

    // Running the same state again leaves the effect alone rather than refreshing it.
    let before = body.hunger_effect.clone();
    body.update_stomach(&rates, &inputs, &vitamins);
    assert_eq!(body.hunger_effect, before, "an effect already worn is not re-added");
}
