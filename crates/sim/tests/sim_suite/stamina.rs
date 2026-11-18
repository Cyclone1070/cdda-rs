//! The tank: what an idle turn puts back, and the system that does it every turn.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;

/// **Breath comes back at twenty a turn**, and stops at the ceiling. `corpus/stamina.json` is `--dump-stamina`: the figures that set the rate, six observed turns of rest from a
/// thousand, a two-turn rest, and a rest that runs into the cap.
///
/// Reference: `Character::update_stamina` (`character.cpp:7020-7086`) - `ceil( recovery * turns )` added and then clamped, the recovery being the base rate times
/// `cardiofit / cardio_acc_base` (one for an untrained survivor) and a breathing modifier (one without mouth encumbrance).
#[test]
fn breath_comes_back_twenty_a_turn() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/stamina.json");
    let bytes = std::fs::read(path).expect("the stamina recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };

    let number = |field: &str| -> f64 {
        match root.get(field) {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => f64::from(i32::try_from(*value).expect("an int")),
            other => panic!("{field}: {other:?}"),
        }
    };
    // **The rate is the option, not a guess**: the port's constant must be what the recording was made with, and the two modifiers must be one or the rest is not the port's case.
    let cardiofit = number("cardiofit");
    let cardio_base = number("cardio_acc_base");
    let breathing = number("breathing_mod");
    assert!(
        (cardiofit / cardio_base - 1.0).abs() < 1e-9,
        "an untrained survivor's cardio ratio is one: {cardiofit} / {cardio_base}"
    );
    assert!(
        (breathing - 1.0).abs() < 1e-9,
        "and no mouth encumbrance leaves breathing at one: {breathing}"
    );

    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };
    let mut rest: Vec<i32> = Vec::new();
    let mut two_turns = None;
    let mut at_the_cap = None;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let stamina = match row.get("stamina") {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("an int"),
            other => panic!("stamina: {other:?}"),
        };
        let name = match row.get("case") {
            Some(cdda_json::Value::Str(text)) => text.clone(),
            _ => panic!("a name"),
        };
        match name.as_str() {
            "rest" => rest.push(stamina),
            "two_turns" => two_turns = Some(stamina),
            "at_the_cap" => at_the_cap = Some(stamina),
            other => panic!("unknown case {other}"),
        }
    }
    assert_eq!(rest.len(), 6, "six turns of rest were recorded");
    assert_eq!(rest.first().copied(), Some(1000), "from a thousand");

    // **Each turn puts back the port's own figure**, and the recording says the same: the rate is the data's twenty, and a rested - not winded - survivor gets all of it.
    let rate = cdda_sim::regeneration::BASE_WIND_BACK_PER_TURN;
    let per_turn = cdda_sim::regeneration::wind_back_per_turn(rate, false);
    assert!(
        (per_turn - 20.0).abs() < 1e-9,
        "a turn puts back twenty: {per_turn}"
    );
    for (turn, recorded) in rest.iter().enumerate() {
        let index = i32::try_from(turn).expect("a turn index");
        let ours = 1000 + 20 * index;
        assert_eq!(
            ours, *recorded,
            "**turn {index} of a rest**: {ours} against {recorded}"
        );
    }
    assert_eq!(
        Some(1040),
        two_turns,
        "**two turns at once** put back two turns' worth: `ceil( recovery * turns )`"
    );

    // **And the cap holds**: five points short of the ceiling, a turn puts back five and not twenty.
    let Some(cap) = at_the_cap else {
        panic!("the cap case");
    };
    let ceiling = match root.get("max") {
        Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("an int"),
        other => panic!("max: {other:?}"),
    };
    assert_eq!(cap, ceiling, "a rest at the cap stops at the cap");

    // **The rule through a store**: twenty back from a thousand, and never past the ceiling.
    let mut world = World::new();
    let tired = world
        .spawn((cdda_sim::creature::Stamina {
            cur: 1_000.0,
            max: 8_500.0,
        },))
        .id();
    let nearly_full = world
        .spawn((cdda_sim::creature::Stamina {
            cur: 8_490.0,
            max: 8_500.0,
        },))
        .id();
    let no_tank = world.spawn(()).id();
    let _ = cdda_sim::regeneration::recover_wind(&mut world, tired, rate, false, 1);
    let _ = cdda_sim::regeneration::recover_wind(&mut world, nearly_full, rate, false, 1);
    let nothing = cdda_sim::regeneration::recover_wind(&mut world, no_tank, rate, false, 1);
    assert_eq!(
        world
            .get::<cdda_sim::creature::Stamina>(tired)
            .map(|s| s.cur),
        Some(1_020.0),
        "**a turn of breathing puts twenty back**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Stamina>(nearly_full)
            .map(|s| s.cur),
        Some(8_500.0),
        "and stops at the ceiling"
    );
    assert_eq!(nothing, 0, "a creature without a tank is left alone");
}
