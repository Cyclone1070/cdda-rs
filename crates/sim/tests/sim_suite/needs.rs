//! The arithmetic behind the metabolic rates, against a recorded grid of character states.
//!
//! `corpus/characters/needs.json` is `--dump-needs`: 72 character states — six hunger figures from engorged to past famished, three calorie stores from full to half, and
//! four activity levels — with every function called separately and its own answer written down, so a disagreement says **which** of the four is wrong instead of only
//! that the rates came out different.

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
use cdda_sim::needs::{Point, base_bmr, bmr, metabolic_rate, multi_lerp, needs_rates, starvation};

/// One recorded case.
struct Case {
    hunger: i32,
    kcal_percent: f32,
    bodyweight_grams: i32,
    metabolism_base: f32,
    starvation: i32,
    speed: i32,
    reference_metabolic_rate: f32,
    reference_base_bmr: i32,
    average_activity: f32,
    max_exertion: f32,
    reference_bmr: i32,
    rate_kcal: f32,
    rate_hunger: f32,
    rate_thirst: f32,
    rate_sleepiness: f32,
    rate_recovery: f32,
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

fn recorded() -> (i32, i32, Vec<Case>) {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/characters/needs.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let height = i32::try_from(integer(&root, "height_cm")).expect("a height");
    let age = i32::try_from(integer(&root, "age")).expect("an age");
    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("cases")
    };
    let cases = cases
        .iter()
        .map(|case| {
            let Value::Object(case) = case else {
                panic!("a case")
            };
            let rates = match case.get("rates") {
                Some(Value::Object(rates)) => rates,
                other => panic!("rates: {other:?}"),
            };
            Case {
                hunger: i32::try_from(integer(case, "hunger")).expect("hunger"),
                kcal_percent: float(case, "kcal_percent"),
                bodyweight_grams: i32::try_from(integer(case, "bodyweight_grams")).expect("grams"),
                metabolism_base: float(case, "metabolism_base"),
                starvation: i32::try_from(integer(case, "starvation")).expect("starvation"),
                speed: i32::try_from(integer(case, "speed")).expect("speed"),
                reference_metabolic_rate: float(case, "metabolic_rate"),
                reference_base_bmr: i32::try_from(integer(case, "base_bmr")).expect("bmr"),
                average_activity: float(case, "average_activity"),
                max_exertion: float(case, "max_exertion"),
                reference_bmr: i32::try_from(integer(case, "bmr")).expect("bmr"),
                rate_kcal: float(rates, "kcal"),
                rate_hunger: float(rates, "hunger"),
                rate_thirst: float(rates, "thirst"),
                rate_sleepiness: float(rates, "sleepiness"),
                rate_recovery: float(rates, "recovery"),
            }
        })
        .collect();
    (height, age, cases)
}

#[test]
fn the_metabolic_rates_follow_the_reference_arithmetic() {
    let (height, age, cases) = recorded();
    assert_eq!(cases.len(), 72, "every recorded state");

    let mut differing: Vec<String> = Vec::new();
    let mut starved = 0;
    let mut hungry_multiplier = 0;
    let mut activity_ceilings = 0;
    for case in &cases {
        // The dump writes the hunger it **asked** for; the reference caps hunger at 300 the moment it is set, so a case that asked for 600 is a character at 300 and the
        // multiplier is exactly 1. Reading the requested figure straight through was this gate's first failure.
        let hunger = case.hunger.min(cdda_sim::needs::MAX_HUNGER);
        let label = format!(
            "h{}:pct{:.2}:w{}",
            case.hunger, case.kcal_percent, case.bodyweight_grams
        );

        let ours_starvation = starvation(case.kcal_percent);
        if ours_starvation != case.starvation {
            differing.push(format!(
                "{label}.starvation: the port says {ours_starvation}, the reference says {}",
                case.starvation
            ));
        }
        // The metabolic multiplier is computed from the port's own starvation figure, so a wrong curve shows up here as well as above.
        let ours_rate = metabolic_rate(hunger, ours_starvation, case.speed, case.metabolism_base);
        if (ours_rate - case.reference_metabolic_rate).abs() > 1e-6 {
            differing.push(format!(
                "{label}.metabolic_rate: the port says {ours_rate:.9}, the reference says {:.9}",
                case.reference_metabolic_rate
            ));
        }
        let ours_base = base_bmr(case.bodyweight_grams, height, age, case.metabolism_base);
        if ours_base != case.reference_base_bmr {
            differing.push(format!(
                "{label}.base_bmr: the port says {ours_base}, the reference says {}",
                case.reference_base_bmr
            ));
        }
        let ours_bmr = bmr(ours_base, case.average_activity, case.max_exertion);
        if ours_bmr != case.reference_bmr {
            differing.push(format!(
                "{label}.bmr: the port says {ours_bmr}, the reference says {}",
                case.reference_bmr
            ));
        }
        // The two option-driven rates are one for an awake character with no traits, which is the only state this grid covers.
        if case.rate_thirst != 1.0 || case.rate_sleepiness != 1.0 || case.rate_recovery != 0.0 {
            differing.push(format!(
                "{label}.rates: an awake character with no traits should have thirst 1, sleepiness 1 and recovery 0, but the reference says \
                 {} / {} / {}",
                case.rate_thirst, case.rate_sleepiness, case.rate_recovery
            ));
        }
        let ours_rates = needs_rates(ours_bmr, ours_rate, case.rate_thirst, case.rate_sleepiness);
        if ours_rates.kcal != case.rate_kcal || ours_rates.hunger != case.rate_hunger {
            differing.push(format!(
                "{label}.rates: the port says kcal {} hunger {}, the reference says kcal {} hunger {}",
                ours_rates.kcal, ours_rates.hunger, case.rate_kcal, case.rate_hunger
            ));
        }

        if ours_starvation > 0 {
            starved += 1;
        }
        if (ours_rate - case.metabolism_base).abs() > 1e-6 {
            hungry_multiplier += 1;
        }
        if ours_bmr != ours_base {
            activity_ceilings += 1;
        }
    }
    // **Non-vacuity**: a grid where nothing moves would pass while proving nothing.
    assert!(starved > 20, "states that were starving: {starved}");
    assert!(
        hungry_multiplier > 20,
        "states whose multiplier moved: {hungry_multiplier}"
    );
    assert!(
        activity_ceilings > 50,
        "states whose activity rounded up: {activity_ceilings}"
    );
    assert!(
        differing.is_empty(),
        "{} disagreements over {} states; the first twelve are {:#?}",
        differing.len(),
        cases.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}

/// `multi_lerp` on its own, including the two ends that are not interpolations at all.
///
/// Reference: `src/cata_utility.cpp:267-288`. Below the first point the answer is the first point's value, above the last it is the last's, a value **on** a point
/// answers that point, and between two points it is a straight line.
#[test]
fn multi_lerp_holds_the_ends_and_walks_the_line() {
    let points: [Point; 3] = [(0.0, 10.0), (2.0, 20.0), (10.0, 0.0)];
    assert_eq!(multi_lerp(&points, -5.0), 10.0, "below the first point");
    assert_eq!(multi_lerp(&points, 0.0), 10.0, "on the first point");
    assert_eq!(multi_lerp(&points, 2.0), 20.0, "on a middle point");
    assert_eq!(multi_lerp(&points, 6.0), 10.0, "halfway between 20 and 0");
    assert_eq!(multi_lerp(&points, 10.0), 0.0, "on the last point");
    assert_eq!(multi_lerp(&points, 99.0), 0.0, "above the last point");
}
