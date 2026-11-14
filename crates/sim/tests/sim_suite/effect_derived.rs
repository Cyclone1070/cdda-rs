//! The small rules an effect answers about itself: its average modifier, its kill chance, and the addiction multiplier.
//!
//! `corpus/effect-derived.json` is `--dump-effect-derived`: for every effect type that states a range or a kill chance, the value `effect::get_avg_mod`
//! (`src/effect.cpp:1284-1296`) answers at four intensities and two resistance levels, and the `kill_roll` (`src/effect.cpp:1634-1645`) answer with the die the
//! reference rolled.
//!
//! Three shapes a reading gets wrong. The average is taken **only when the stated maximum is not zero** - `int( max ) != 0`, so a maximum of 0.5 states no range and
//! the minimum comes back alone - and the mean is truncated toward zero, not rounded. The kill chance is indexed by **intensity minus one**, capped so an effect past
//! the end of its list keeps using the last entry, which is the opposite of clamping to the first. And the addiction multiplier is one **unless** the effect says a
//! painkiller addict takes less of it, which no effect in the data currently does.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    // The recording is the authority on the exact bits, and `int( max ) != 0` is the reference's own cast, so the truncations here are the rule being tested.
    clippy::cast_possible_truncation
)]

use cdda_content::effects::EffectType;
use cdda_json::Value;

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

fn real(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(Value::Float(value)) => *value,
        Some(Value::Int(value)) => *value as f64,
        other => panic!("{key}: {other:?}"),
    }
}

fn boolean(object: &cdda_json::Object, key: &str) -> bool {
    match object.get(key) {
        Some(Value::Bool(value)) => *value,
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn every_types_average_modifier_matches_the_reference() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effect-derived.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("types")
    };
    assert_eq!(integer(&root, "count"), 5_552, "every recorded case");
    assert_eq!(
        integer(&root, "fired"),
        0,
        "and the reference's kill rolls fired none of them"
    );

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let effects: std::collections::BTreeMap<String, EffectType> =
        cdda_content::effects::load_paths(&paths).expect("the effects load");

    let mut differing: Vec<String> = Vec::new();
    let mut compared = 0;
    let mut averaged = 0;
    let mut minimum_only = 0;
    let mut resistant = 0;
    let mut zero = 0;
    for recorded in types {
        let Value::Object(recorded) = recorded else {
            panic!("a type")
        };
        let id = text(recorded, "id");
        let Some(kind) = effects.get(&id) else {
            differing.push(format!("{id}: the port does not load it"));
            continue;
        };
        let Some(Value::Array(cases)) = recorded.get("avg") else {
            panic!("avg")
        };
        for case in cases {
            let Value::Object(case) = case else {
                panic!("an average")
            };
            let argument = text(case, "arg");
            let intensity = i32::try_from(integer(case, "intensity")).expect("an intensity");
            let level = u8::try_from(integer(case, "level")).expect("a level");
            let expected = i32::try_from(integer(case, "average")).expect("an average");
            let actual = cdda_sim::effects::average_mod(kind, argument.as_str(), level, intensity);
            compared += 1;
            if level != 0 {
                resistant += 1;
            }
            let low = kind.mod_value(
                argument.as_str(),
                cdda_content::effects::ModAction::Min,
                level,
                intensity,
            );
            let high = kind.mod_value(
                argument.as_str(),
                cdda_content::effects::ModAction::Max,
                level,
                intensity,
            );
            if high as i32 != 0 {
                averaged += 1;
            } else {
                minimum_only += 1;
            }
            if expected == 0 {
                zero += 1;
            }
            if actual != expected {
                differing.push(format!(
                    "{id} {argument} at intensity {intensity} level {level}: the port says {actual}, the reference {expected} (min {low}, max {high})"
                ));
            }
        }
    }
    assert_eq!(compared, 4_160, "every recorded average");
    assert!(
        averaged > 0 && minimum_only > 0,
        "both branches of the stated-maximum gate are reached: {averaged} averaged, {minimum_only} minimum only"
    );
    assert!(
        resistant > 0 && zero > 0,
        "and both levels, with zeros among the answers: {resistant} resistant, {zero} zero"
    );
    assert!(
        differing.is_empty(),
        "{} differences:\n{}",
        differing.len(),
        differing.join("\n")
    );
}

#[test]
fn a_stated_maximum_below_one_does_not_count_as_a_range() {
    // The reference reads the maximum as an **integer** before deciding whether a range was stated (`static_cast<int>( max ) != 0`), so a maximum of half a point is
    // a maximum of nothing and the minimum comes back alone. The data has no such case - the corpus's both-branch count shows every stated maximum is whole - so the
    // rule is pinned here instead, on a type assembled for it: without this test, `max != 0.0` passes the whole corpus.
    let mut object = cdda_json::Object::new();
    object.insert(
        "id".to_string(),
        Value::Str("harness_fractional_max".to_string()),
    );
    object.insert(
        "name".to_string(),
        Value::Str("a fractional maximum".to_string()),
    );
    let mut per = cdda_json::Object::new();
    // The loader keys the table by the data's own names (`per_min` reads as `Min` under `PER`), so the range is stated with those keys, not as a nested object.
    // PAIN is the group with both a minimum key and a maximum key; the stats (`per_mod`) only have a minimum, so a range cannot be stated for them at all.
    per.insert("pain_min".to_string(), Value::Array(vec![Value::Int(2)]));
    per.insert(
        "pain_max".to_string(),
        Value::Array(vec![Value::Float(0.5)]),
    );
    object.insert("base_mods".to_string(), Value::Object(per));
    let kind = cdda_content::effects::read_one("harness_fractional_max", &object);

    assert_eq!(
        cdda_sim::effects::average_mod(&kind, "PAIN", 0, 1),
        2,
        "the minimum alone, because the maximum is not a whole number"
    );
    assert_eq!(
        kind.mod_value("PAIN", cdda_content::effects::ModAction::Max, 0, 1),
        0.5,
        "and the maximum really is half a point rather than zero"
    );
}

#[test]
fn a_kill_chance_is_chosen_by_intensity_and_capped_at_the_lists_end() {
    // The reference indexes the list by `intensity - 1` and caps the index at the last entry (`cap_to_size`, `src/effect.cpp:1107-1116`), so a list of two answers at
    // intensity four, five or a hundred all read the **second** entry - a reading that clamped to the first would kill far less often. The recording holds the one effect
    // in the data with a kill chance (`star_vampire_blood_drink_feral`, one in twenty), whose rolls the reference made; the port replays each one.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effect-derived.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("types")
    };
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let effects: std::collections::BTreeMap<String, EffectType> =
        cdda_content::effects::load_paths(&paths).expect("the effects load");

    let mut replayed = 0;
    let mut true_answers = 0;
    let mut differing: Vec<String> = Vec::new();
    for recorded in types {
        let Value::Object(recorded) = recorded else {
            panic!("a type")
        };
        let id = text(recorded, "id");
        let Some(kind) = effects.get(&id) else {
            continue;
        };
        let Some(Value::Array(cases)) = recorded.get("kill") else {
            panic!("kill")
        };
        for case in cases {
            let Value::Object(case) = case else {
                panic!("a kill case")
            };
            let intensity = i32::try_from(integer(case, "intensity")).expect("an intensity");
            let level = u8::try_from(integer(case, "level")).expect("a level");
            let roll = real(case, "roll");
            let expected = boolean(case, "answer");
            // The reference rolled one real in `[0, 1)`; the port is given that same number, so both decide on the same die.
            let mut rolls = cdda_core_types::rng::Rolls::replaying(
                cdda_core_types::rng::Recording::parse(&format!("real 0 1 {roll}\n"))
                    .expect("a recording"),
            );
            let actual = cdda_sim::effects::kill_roll(kind, level, intensity, &mut rolls);
            replayed += 1;
            if expected {
                true_answers += 1;
            }
            if actual != expected {
                differing.push(format!(
                    "{id} at intensity {intensity} level {level} on roll {roll}: the port says {actual}, the reference {expected} (chances {:?})",
                    kind.kill_chance
                ));
            }
        }
    }
    assert_eq!(
        true_answers, 0,
        "and the reference's rolls fired none of them, one in twenty being one in twenty"
    );
    assert_eq!(
        replayed, 1_392,
        "every recorded kill roll is replayed: 174 types, four intensities, two levels"
    );
    assert!(
        differing.is_empty(),
        "{} differences:\n{}",
        differing.len(),
        differing.join("\n")
    );

    // And the cap itself, on a list assembled for it: intensity three of a two-entry list reads the second entry, not the first.
    let star = effects
        .get("star_vampire_blood_drink_feral")
        .expect("the one effect with a kill chance");
    assert_eq!(
        star.kill_chance,
        vec![(1, 20)],
        "one in twenty, as the data states it"
    );
    assert_eq!(
        cdda_sim::effects::cap_to_size(2, 1),
        0,
        "intensity one reads the first entry"
    );
    assert_eq!(
        cdda_sim::effects::cap_to_size(2, 2),
        1,
        "intensity two the second"
    );
    assert_eq!(
        cdda_sim::effects::cap_to_size(2, 3),
        1,
        "and three keeps the second"
    );
    assert_eq!(
        cdda_sim::effects::cap_to_size(2, 100),
        1,
        "however far past the end it goes"
    );
    assert_eq!(
        cdda_sim::effects::cap_to_size(2, 0),
        1,
        "and an intensity of zero underflows to the last entry, as the reference does"
    );
}
