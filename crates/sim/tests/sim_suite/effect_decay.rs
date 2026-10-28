//! Wearing an effect down: what each turn takes off it.
//!
//! `corpus/effect-decay.json` is `--dump-effect-decay`: eight effect profiles — no tick, a tick every turn, a tick every third turn, one that may reach zero, positive
//! steps, and a duration-sized one — each worn for eight turns from three starting positions, recording the intensity, the duration left and whether the turn was the
//! one the effect should come off.
//!
//! The rule is `effect::decay` (`src/effect.cpp:1042-1068`) and it does two things in one pass, which is why the guards matter: the intensity only steps while the effect
//! is **short of its ceiling**, the tick divides the **absolute** turn rather than the effect's age, a **positive** step is skipped when another effect blocks this one,
//! and the duration always falls by one unless the effect is paused.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::effects::EffectType;
use cdda_json::Value;
use std::collections::BTreeSet;

struct Profile {
    max_intensity: i32,
    max_duration: i64,
    int_decay_step: i32,
    int_decay_tick: i32,
    int_dur_factor: i64,
    int_decay_remove: bool,
}

impl Profile {
    fn kind(&self) -> EffectType {
        EffectType {
            id: "harness".to_owned(),
            max_intensity: self.max_intensity,
            max_duration: self.max_duration,
            dur_add_perc: 100,
            int_add_val: 0,
            int_decay_step: self.int_decay_step,
            int_decay_tick: self.int_decay_tick,
            int_dur_factor: self.int_dur_factor,
            int_decay_remove: self.int_decay_remove,
            main_parts_only: false,
            show_in_info: false,
            show_intensity: true,
            pain_sizing: false,
            hurt_sizing: false,
            harmful_cough: false,
            impairs_movement: false,
            pkill_addict_reduces: false,
            apply_ratings: Vec::new(),
            flags: BTreeSet::new(),
            resist_traits: Vec::new(),
            resist_effects: Vec::new(),
            removes_effects: Vec::new(),
            blocks_effects: Vec::new(),
        }
    }
}

/// One turn of wearing an effect.
struct Step {
    turn: i64,
    intensity: i32,
    turns_left: i64,
    removed: bool,
}

struct Run {
    profile: usize,
    start: usize,
    steps: Vec<Step>,
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

fn text(object: &cdda_json::Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

fn turns(object: &cdda_json::Object, key: &str) -> i64 {
    let raw = text(object, key);
    let (amount, unit) = raw.split_once(' ').unwrap_or_else(|| panic!("{raw}"));
    let amount: i64 = amount.parse().unwrap_or_else(|_| panic!("{raw}"));
    match unit {
        "turns" => amount,
        "minutes" => amount * 60,
        "hours" => amount * 60 * 60,
        other => panic!("unit {other}"),
    }
}

fn recorded() -> (Vec<Profile>, Vec<(i32, i64)>, Vec<Run>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effect-decay.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let (Some(Value::Array(profiles)), Some(Value::Array(starts)), Some(Value::Array(runs))) =
        (root.get("profiles"), root.get("starts"), root.get("runs"))
    else {
        panic!("profiles, starts and runs")
    };
    let profiles = profiles
        .iter()
        .map(|profile| {
            let Value::Object(profile) = profile else {
                panic!("a profile")
            };
            Profile {
                max_intensity: i32::try_from(integer(profile, "max_intensity")).expect("an intensity"),
                max_duration: turns(profile, "max_duration"),
                int_decay_step: i32::try_from(integer(profile, "int_decay_step")).expect("a step"),
                int_decay_tick: i32::try_from(integer(profile, "int_decay_tick")).expect("a tick"),
                int_dur_factor: turns(profile, "int_dur_factor"),
                int_decay_remove: boolean(profile, "int_decay_remove"),
            }
        })
        .collect();
    let starts = starts
        .iter()
        .map(|start| {
            let Value::Object(start) = start else {
                panic!("a start")
            };
            (
                i32::try_from(integer(start, "intensity")).expect("an intensity"),
                integer(start, "minutes") * 60,
            )
        })
        .collect();
    let runs = runs
        .iter()
        .map(|run| {
            let Value::Object(run) = run else { panic!("a run") };
            let Some(Value::Array(steps)) = run.get("steps") else {
                panic!("steps")
            };
            Run {
                profile: usize::try_from(integer(run, "profile")).expect("a profile index"),
                start: usize::try_from(integer(run, "start")).expect("a start index"),
                steps: steps
                    .iter()
                    .map(|step| {
                        let Value::Object(step) = step else { panic!("a step") };
                        Step {
                            turn: integer(step, "turn"),
                            intensity: i32::try_from(integer(step, "intensity")).expect("an intensity"),
                            turns_left: integer(step, "turns_left"),
                            removed: boolean(step, "removed"),
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    (profiles, starts, runs)
}

#[test]
fn every_turn_takes_off_what_the_reference_takes_off() {
    let (profiles, starts, runs) = recorded();
    assert_eq!(profiles.len(), 8, "every recorded profile");
    assert_eq!(runs.len(), 24, "every recorded run");

    let mut differing: Vec<String> = Vec::new();
    let mut stepped_down = 0;
    let mut stepped_up = 0;
    let mut removed = 0;
    let mut derived = 0;
    let mut duration_fell = 0;
    for run in &runs {
        let kind = profiles[run.profile].kind();
        let (intensity, duration) = starts[run.start];
        let mut worn = cdda_sim::effects::ActiveEffect::new(&kind, duration, intensity);
        for step in &run.steps {
            let before = (worn.intensity, worn.duration);
            let removed_now = worn.decay(&kind, step.turn, false);
            let label = format!("profile{} start{} turn{}", run.profile, run.start, step.turn);
            if (worn.intensity, worn.duration, removed_now)
                != (step.intensity, step.turns_left, step.removed)
            {
                differing.push(format!(
                    "{label}: the port says intensity {} for {} turns (removed {removed_now}), the reference says {} for {} (removed {})",
                    worn.intensity, worn.duration, step.intensity, step.turns_left, step.removed
                ));
            }
            if worn.intensity < before.0 {
                stepped_down += 1;
            }
            if worn.intensity > before.0 {
                stepped_up += 1;
            }
            if removed_now {
                removed += 1;
            }
            if kind.int_dur_factor != 0 && worn.intensity != before.0 {
                derived += 1;
            }
            if worn.duration < before.1 {
                duration_fell += 1;
            }
        }
    }
    // **Non-vacuity**: every branch of the rule has to fire somewhere in the grid.
    assert!(stepped_down > 0, "intensities stepped down: {stepped_down}");
    assert!(stepped_up > 0, "intensities stepped up: {stepped_up}");
    assert!(removed > 0, "effects taken off: {removed}");
    // Declared: eight turns is too short for the ten-minute profile to cross a boundary, so the recompute is not in this grid — the `--dump-effect-merge` recording
    // is where `set_duration`'s recompute is pinned (1,260 turns against a 600-turn factor answers 3, not 2), and the unit test below pins the fall.
    assert_eq!(derived, 0, "the grid does not reach a boundary crossing: {derived}");
    assert!(duration_fell > 100, "durations that fell: {duration_fell}");
    assert!(
        differing.is_empty(),
        "{} disagreements over {} turns; the first twelve are {:#?}",
        differing.len(),
        runs.len() * 8,
        differing.get(..12).unwrap_or(&differing)
    );
}

/// The guards that are easy to miss, pinned one at a time against the same rule.
#[test]
fn the_guards_that_are_easy_to_miss() {
    let kind = Profile {
        max_intensity: 5,
        max_duration: 3600,
        int_decay_step: -1,
        int_decay_tick: 1,
        int_dur_factor: 0,
        int_decay_remove: false,
    }
    .kind();
    // An effect at its ceiling does **not** lose intensity: it has only just been applied, and the reference wants the full span first.
    let mut at_ceiling = cdda_sim::effects::ActiveEffect::new(&kind, kind.max_duration, 3);
    at_ceiling.decay(&kind, 0, false);
    assert!(
        at_ceiling.intensity >= 2,
        "nothing steps at the ceiling: {}",
        at_ceiling.intensity
    );
    // One turn short of the ceiling and it does.
    let mut below = cdda_sim::effects::ActiveEffect::new(&kind, kind.max_duration - 1, 3);
    below.decay(&kind, 0, false);
    assert_eq!(below.intensity, 2);

    // A **positive** step is suppressed by a blocker; a negative one is not.
    let rising = Profile {
        max_intensity: 5,
        max_duration: 3600,
        int_decay_step: 1,
        int_decay_tick: 1,
        int_dur_factor: 0,
        int_decay_remove: false,
    }
    .kind();
    let mut effect = cdda_sim::effects::ActiveEffect::new(&rising, 100, 2);
    effect.decay(&rising, 0, true);
    assert_eq!(effect.intensity, 2, "a blocked effect does not rise");
    effect.decay(&rising, 0, false);
    assert_eq!(effect.intensity, 3, "unblocked, it rises");

    let mut falling = cdda_sim::effects::ActiveEffect::new(&kind, 100, 2);
    falling.decay(&kind, 0, true);
    assert_eq!(falling.intensity, 1, "a blocked fall still falls");

    // A paused effect keeps its duration.
    let mut paused = cdda_sim::effects::ActiveEffect::new(&kind, 100, 1);
    paused.permanent = true;
    paused.decay(&kind, 0, false);
    assert_eq!(paused.duration, 100, "a paused effect does not age");

    // A duration-sized effect recomputes its intensity as the duration falls: six turns above a ten-minute boundary is a barely-live effect, and one turn later it
    // drops a rung. The arithmetic is the reference's `ceil( duration / int_dur_factor )` — for six turns, `ceil( 0.6 )` is one.
    let sized = Profile {
        max_intensity: 30,
        max_duration: 21_600,
        int_decay_step: 0,
        int_decay_tick: 0,
        int_dur_factor: 600,
        int_decay_remove: false,
    }
    .kind();
    let mut crossing = cdda_sim::effects::ActiveEffect::new(&sized, 606, 1);
    assert_eq!(crossing.intensity, 2, "606 turns is two rungs");
    crossing.decay(&sized, 0, false);
    assert_eq!(crossing.duration, 605);
    assert_eq!(crossing.intensity, 2, "605 turns is still two rungs");
    for _ in 0..5 {
        crossing.decay(&sized, 0, false);
    }
    assert_eq!(crossing.duration, 600);
    assert_eq!(crossing.intensity, 1, "600 turns is one rung");
}
