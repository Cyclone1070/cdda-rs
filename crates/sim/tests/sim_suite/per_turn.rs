//! What one effect does to a body in one turn.
//!
//! `corpus/per-turn.json` is `--dump-per-turn`: seven synthetic effect shapes run through the reference's real `process_one_effect` on a real avatar, across three
//! hunger levels, two thirst levels, arrival and non-arrival, and three turns — 252 cases, 123 of which move something. The effects are synthesised so the twenty-odd
//! other branches of the function see zero and do nothing.
//!
//! Four rules hide between the three lines of each branch: the amount is read from `get_amount` on the turn the effect **arrives** and from `get_mod` afterwards; a
//! stated maximum turns the amount into a **roll**; the chance is **skipped entirely** for a brand-new effect; and the movement is clamped against the type's own
//! ceiling and floor before it is applied.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::effects::EffectType;
use cdda_core_types::rng::Rolls;
use cdda_json::Value;

fn integer(object: &cdda_json::Object, key: &str) -> i64 {
    match object.get(key) {
        Some(Value::Int(value)) => *value,
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
fn every_per_turn_change_matches_the_reference() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/per-turn.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("cases")
    };
    assert_eq!(cases.len(), 252, "every recorded case");
    assert_eq!(
        integer(&root, "moved"),
        99,
        "the reference moved a body on this many"
    );

    // The seven shapes the harness synthesised, read by the port's own reader from the recording itself.
    let Some(Value::Array(profiles)) = root.get("profiles") else {
        panic!("profiles")
    };
    assert_eq!(profiles.len(), 7, "the shapes the reference ran");
    let mut effects: std::collections::BTreeMap<String, EffectType> =
        std::collections::BTreeMap::new();
    for (index, profile) in profiles.iter().enumerate() {
        let Value::Object(inner) = profile else {
            panic!("a profile")
        };
        // The recording holds the *contents* of `base_mods`, which is what the harness wrote into its JSON; the reader wants the member that holds them.
        let mut object = cdda_json::Object::new();
        assert!(object.insert("base_mods".to_string(), Value::Object(inner.clone())));
        effects.insert(
            format!("harness_turn_{index}"),
            cdda_content::effects::read_one(&format!("harness_turn_{index}"), &object),
        );
    }

    let mut differing: Vec<String> = Vec::new();
    let mut moved = 0;
    let mut by_target: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for case in cases {
        let Value::Object(case) = case else {
            panic!("a case")
        };
        let profile = integer(case, "profile");
        let id = format!("harness_turn_{profile}");
        let Some(kind) = effects.get(&id) else {
            differing.push(format!("{id}: the port does not load it"));
            continue;
        };
        let hunger = i32::try_from(integer(case, "hunger")).expect("hunger");
        let thirst = i32::try_from(integer(case, "thirst")).expect("thirst");
        let is_new = boolean(case, "is_new");
        let turn = integer(case, "turn");
        let seed = u32::try_from(integer(case, "seed")).expect("a seed");
        let after_hunger = i32::try_from(integer(case, "after_hunger")).expect("hunger");
        let after_thirst = i32::try_from(integer(case, "after_thirst")).expect("thirst");
        let mut body = cdda_sim::body::Body::default();
        body.set_hunger(hunger);
        body.set_thirst(thirst);
        let worn = cdda_sim::effects::ActiveEffect {
            id: id.clone(),
            intensity: 1,
            duration: 1,
            permanent: false,
        };
        // The reference's engine and the port's are the same generator, gated seed for seed in `core_types_suite::rng`, so seeding the port with the case's seed
        // replays the reference's dice.
        let mut rolls = Rolls::new(seed);
        worn.apply_to_body(&mut body, kind, &mut rolls, turn, is_new);
        if body.hunger != hunger || body.thirst != thirst {
            moved += 1;
        }
        if body.hunger != hunger {
            *by_target.entry("hunger".to_string()).or_default() += 1;
        }
        if body.thirst != thirst {
            *by_target.entry("thirst".to_string()).or_default() += 1;
        }
        if body.hunger != after_hunger || body.thirst != after_thirst {
            differing.push(format!(
                "{id} at hunger {hunger}, thirst {thirst}, arrival {is_new}, turn {turn}: the port says ({}, {}), the reference says ({after_hunger}, {after_thirst})",
                body.hunger, body.thirst
            ));
        }
    }
    // **Non-vacuity**: the grid must move bodies, must move both statistics, and must reach both the arrival path (which skips the chance) and the per-turn path.
    assert_eq!(moved, 99, "cases the port moved: {moved}");
    // The measured split rather than a guess: only one of the seven shapes moves thirst, so it moves nine times against hunger's hundred and fourteen.
    assert_eq!(
        by_target.get("hunger"),
        Some(&90),
        "hunger moves: {by_target:?}"
    );
    assert_eq!(
        by_target.get("thirst"),
        Some(&9),
        "thirst moves: {by_target:?}"
    );
    assert!(
        differing.is_empty(),
        "{} disagreements over 252 cases; the first twelve are {:#?}",
        differing.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}

/// The roll path the grid cannot hold: `get_mod` draws between a stated minimum and maximum, and only when a maximum is stated.
///
/// The oracle's dice plateau — after two draws from a fresh seed it answers the same value however it is reseeded (see `--dump-rng-probe`, and the note in
/// `docs/parity.md`) — so a recording of a roll would be the plateau's value rather than a die. The rule is therefore pinned here, on dice the test writes itself,
/// which is the same standard the rest of the dice work meets: the comparison is over **identical draws**, never two rolls that happened to agree.
#[test]
fn the_per_turn_amount_rolls_only_when_a_maximum_is_stated() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/per-turn.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(profiles)) = root.get("profiles") else {
        panic!("profiles")
    };
    let read = |index: usize| {
        let Value::Object(inner) = &profiles[index] else {
            panic!("a profile")
        };
        let mut object = cdda_json::Object::new();
        assert!(object.insert("base_mods".to_string(), Value::Object(inner.clone())));
        let id = format!("harness_turn_{index}");
        (id.clone(), cdda_content::effects::read_one(&id, &object))
    };

    // Profile 0 states an amount and no maximum: the low end answers and nothing is rolled, so a recording with no draws at all is what the rule must spend.
    let (_, kind) = read(0);
    let worn = cdda_sim::effects::ActiveEffect {
        id: "harness_turn_0".to_string(),
        intensity: 1,
        duration: 1,
        permanent: false,
    };
    let mut rolls = Rolls::new(1);
    assert_eq!(
        worn.per_turn_amount(&kind, "HUNGER", &mut rolls),
        0,
        "no min and no max answers zero"
    );
    assert!(
        (kind.mod_value("HUNGER", cdda_content::effects::ModAction::Amount, 0, 1) - 3.0).abs()
            < 1e-12
    );

    // The roll itself, on a die the test chooses: a range of two to nine answers whatever the die lands on, truncated.
    let mut object = cdda_json::Object::new();
    let mut group = cdda_json::Object::new();
    assert!(group.insert("hunger_min".to_string(), Value::Array(vec![Value::Int(2)])));
    assert!(group.insert("hunger_max".to_string(), Value::Array(vec![Value::Int(9)])));
    assert!(object.insert("base_mods".to_string(), Value::Object(group)));
    let rolling = cdda_content::effects::read_one("rolling", &object);
    let mut recording = cdda_core_types::rng::Recording {
        draws: vec![cdda_core_types::rng::Draw::Real {
            low: 2.0,
            high: 9.0,
            drawn: 8.75,
        }],
        position: 0,
        engine_draws: Vec::new(),
    };
    recording.position = 0;
    let mut rolls = Rolls::replaying(recording);
    assert_eq!(
        worn.per_turn_amount(&rolling, "HUNGER", &mut rolls),
        8,
        "8.75 truncates to eight"
    );
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "and it really spent the die"
    );

    // No maximum, a minimum: the minimum answers and no die is spent.
    let mut object = cdda_json::Object::new();
    let mut group = cdda_json::Object::new();
    assert!(group.insert("hunger_min".to_string(), Value::Array(vec![Value::Int(4)])));
    assert!(object.insert("base_mods".to_string(), Value::Object(group)));
    let floored = cdda_content::effects::read_one("floored", &object);
    let mut rolls = Rolls::new(1);
    assert_eq!(
        worn.per_turn_amount(&floored, "HUNGER", &mut rolls),
        4,
        "the low end answers"
    );
}
