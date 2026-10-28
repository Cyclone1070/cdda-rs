//! Applying an effect again: what happens to its intensity and its duration.
//!
//! `corpus/effect-merge.json` is `--dump-effect-merge`: six effect profiles (a plain one, a three-step one, a fifty-percent one, a two-hundred-percent one, a
//! duration-sized one and one that can reach zero) each applied three times in four different sequences, recording after every application what the intensity and
//! duration had become. The profiles are part of the recording, so the replay cannot drift from what it was run against. One sequence deliberately uses durations that
//! are not a whole number of the ten-minute factor, because that is where `ceil` and truncation part company.
//!
//! The rules under test are `Creature::add_effect`'s two halves (`src/creature.cpp:1782-1856`): a creature that does not have the effect gets it as asked, capped;
//! a creature that has it has the duration grown by a percentage of the new one and the intensity jumped, stepped or left alone.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::effects::EffectType;
use cdda_json::Value;
use std::collections::BTreeSet;

/// One profile: the effect type a run was applied against.
struct Profile {
    max_intensity: i32,
    max_duration: i64,
    dur_add_perc: i32,
    int_add_val: i32,
    int_dur_factor: i64,
    int_decay_remove: bool,
}

impl Profile {
    /// The type as the loader would have produced it. The two members that do not matter to these rules — the flags and the lists — are empty, which is what a
    /// synthesised effect type has.
    fn kind(&self) -> EffectType {
        EffectType {
            id: "harness".to_owned(),
            max_intensity: self.max_intensity,
            max_duration: self.max_duration,
            dur_add_perc: self.dur_add_perc,
            int_add_val: self.int_add_val,
            int_decay_step: -1,
            int_decay_tick: 0,
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
            mods: cdda_content::effects::ModTable::new(),
        }
    }
}

/// One application of a run.
struct Step {
    minutes: i64,
    intensity: i32,
    after_intensity: i32,
    after_turns: i64,
}

/// One run: a profile, a sequence, and what each application left behind.
struct Run {
    profile: usize,
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

/// Turns from a duration the recording spells the reference's way: `"6 hours"`, `"30 minutes"`, `"0 turns"`.
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

fn recorded() -> (Vec<Profile>, Vec<Run>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effect-merge.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let (Some(Value::Array(profiles)), Some(Value::Array(runs))) = (root.get("profiles"), root.get("runs"))
    else {
        panic!("profiles and runs")
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
                dur_add_perc: i32::try_from(integer(profile, "dur_add_perc")).expect("a percentage"),
                int_add_val: i32::try_from(integer(profile, "int_add_val")).expect("a step"),
                int_dur_factor: turns(profile, "int_dur_factor"),
                int_decay_remove: boolean(profile, "int_decay_remove"),
            }
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
                steps: steps
                    .iter()
                    .map(|step| {
                        let Value::Object(step) = step else { panic!("a step") };
                        Step {
                            minutes: integer(step, "minutes"),
                            intensity: i32::try_from(integer(step, "intensity")).expect("an intensity"),
                            after_intensity: i32::try_from(integer(step, "after_intensity"))
                                .expect("an intensity"),
                            after_turns: integer(step, "after_turns"),
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    (profiles, runs)
}

#[test]
fn every_application_leaves_the_intensity_and_duration_the_reference_leaves() {
    let (profiles, runs) = recorded();
    assert_eq!(profiles.len(), 6, "every recorded profile");
    assert_eq!(runs.len(), 30, "every recorded run");

    let mut differing: Vec<String> = Vec::new();
    let mut created = 0;
    let mut merged = 0;
    let mut jumped = 0;
    let mut stepped = 0;
    let mut capped = 0;
    let mut zero_kept = 0;
    for run in &runs {
        let kind = profiles[run.profile].kind();
        let mut active: Option<cdda_sim::effects::ActiveEffect> = None;
        for (index, step) in run.steps.iter().enumerate() {
            let before = active.as_ref().map(|effect| (effect.intensity, effect.duration));
            match &mut active {
                None => {
                    active = Some(cdda_sim::effects::ActiveEffect::new(
                        &kind,
                        step.minutes * 60,
                        step.intensity,
                    ));
                    created += 1;
                }
                Some(effect) => {
                    effect.merge(&kind, step.minutes * 60, step.intensity);
                    merged += 1;
                }
            }
            let effect = active.as_ref().expect("an effect");
            let label = format!("profile{} step{index}", run.profile);
            if (effect.intensity, effect.duration) != (step.after_intensity, step.after_turns) {
                differing.push(format!(
                    "{label}: the port says intensity {} for {} turns, the reference says {} for {}",
                    effect.intensity, effect.duration, step.after_intensity, step.after_turns
                ));
            }
            // What the step actually exercised, so the counts below are honest about the grid rather than about the code.
            if let Some((intensity, duration)) = before {
                if step.intensity > 0 && effect.intensity == step.intensity && effect.intensity != intensity
                {
                    jumped += 1;
                }
                if step.intensity == 0
                    && kind.int_add_val != 0
                    && kind.int_dur_factor == 0
                    && effect.intensity != intensity
                {
                    stepped += 1;
                }
                if effect.duration == kind.max_duration && duration != kind.max_duration {
                    capped += 1;
                }
            }
            if effect.intensity == 1 && kind.int_decay_remove && step.intensity == 0 && index > 0 {
                zero_kept += 1;
            }
        }
    }
    // **Non-vacuity**: each branch of the rule has to be taken by a real run, or the grid is testing one path six ways.
    assert!(created >= 30, "effects created: {created}");
    assert!(merged >= 60, "effects merged into: {merged}");
    assert!(jumped > 0, "intensities jumped to the one asked for: {jumped}");
    assert!(stepped > 0, "intensities stepped by the type's own size: {stepped}");
    assert!(capped > 0, "durations capped by the ceiling: {capped}");
    assert!(zero_kept > 0, "zero intensities lifted back to one: {zero_kept}");
    assert!(
        differing.is_empty(),
        "{} disagreements over {} applications; the first twelve are {:#?}",
        differing.len(),
        created + merged,
        differing.get(..12).unwrap_or(&differing)
    );
}

/// The integer division the reference does on the duration: `dur * dur_add_perc / 100` truncates, so half of an odd duration loses a turn.
#[test]
fn the_added_duration_truncates_like_the_reference() {
    let mut kind = Profile {
        max_intensity: 9,
        max_duration: 100_000,
        dur_add_perc: 50,
        int_add_val: 0,
        int_dur_factor: 0,
        int_decay_remove: false,
    }
    .kind();
    let mut effect = cdda_sim::effects::ActiveEffect::new(&kind, 3, 1);
    assert_eq!(effect.duration, 3);
    // Three turns at fifty percent is 1.5, and the reference keeps one.
    effect.merge(&kind, 3, 0);
    assert_eq!(effect.duration, 4, "half of three turned into one, not two");

    // And with the percentage at one hundred the same three turns add three.
    kind.dur_add_perc = 100;
    let mut effect = cdda_sim::effects::ActiveEffect::new(&kind, 3, 1);
    effect.merge(&kind, 3, 0);
    assert_eq!(effect.duration, 6);
}
