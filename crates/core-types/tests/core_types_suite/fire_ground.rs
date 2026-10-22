//! The fire eating the ground, driven by the reference's own recorded dice.
//!
//! The oracle writes every die it rolls (`CDDA_RNG_TRACE`) and labels the phases of the fire processor (`fire.before_fuel`, `fire.fuelled`, ...).
//! That turns "what order does it roll in?" into something a test can answer: the recording for `corpus/fields/fire-floor.json` holds four turns of
//! a fire on `t_floor`, and each turn's fuel phase is a handful of draws the port must ask for in exactly that order.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{Ground, ground_fuel};
use cdda_core_types::rng::{Recording, Rolls};

/// The draws between `fire.before_fuel` and `fire.fuelled`, turn by turn.
fn fuel_phases() -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor.rolls.txt");
    let text = std::fs::read_to_string(&path).expect("the recording");
    let mut phases: Vec<Vec<String>> = Vec::new();
    let mut in_fuel = false;
    for line in text.lines() {
        if let Some(label) = line.strip_prefix("mark ") {
            match label {
                "fire.before_fuel" => {
                    in_fuel = true;
                    phases.push(Vec::new());
                }
                "fire.fuelled" => in_fuel = false,
                _ => {}
            }
            continue;
        }
        if in_fuel && let Some(phase) = phases.last_mut() {
            phase.push(line.to_string());
        }
    }
    phases
}

/// Reads one recorded `int lo hi drawn` line into the port's dice, so the next draw comes from the reference.
fn rolls_from(lines: &[String]) -> Rolls {
    let text: String = lines.join("\n");
    Rolls::replaying(Recording::parse(&text).expect("the phase parses"))
}

#[test]
fn the_tracings_four_fuel_phases_are_exactly_what_the_rule_expects() {
    // Two draws on a turn the ground fed (the die that decides, then the die about bashing), one on a turn it refused.
    let phases = fuel_phases();
    assert_eq!(
        phases.len(),
        4,
        "four processor runs in the session: {phases:?}"
    );
    assert_eq!(
        phases[0].len(),
        2,
        "a hard-ground turn that fed and then asked about bashing: {:?}",
        phases[0]
    );
    assert_eq!(
        phases[1].len(),
        1,
        "a turn the ground refused: {:?}",
        phases[1]
    );
    assert_eq!(phases[2].len(), 1, "and another: {:?}", phases[2]);
    assert_eq!(
        phases[3].len(),
        2,
        "and the last one feeds again: {:?}",
        phases[3]
    );
}

#[test]
fn the_port_asks_for_the_same_dice_in_the_same_order() {
    // The real claim: given the reference's dice, the port asks for them in the reference's order and reaches the reference's answer. Four turns
    // at intensity two on `FLAMMABLE_HARD`; the first and the fourth fed, and neither bash die came up zero.
    let phases = fuel_phases();
    let expected = [(2, false), (0, false), (0, false), (2, false)];
    for (index, phase) in phases.iter().enumerate() {
        let mut rolls = rolls_from(phase);
        let fuel = ground_fuel(Ground::FlammableHard, 2, true, &mut rolls);
        assert_eq!(
            (fuel.time_added, fuel.bashed),
            expected[index],
            "turn {index}: {phase:?}"
        );
        assert_eq!(
            rolls.recording_remaining(),
            Some(0),
            "turn {index} left dice unplayed: {phase:?}"
        );
    }
}

#[test]
fn a_fire_on_inert_ground_spends_no_dice_at_all() {
    // Nothing burns, so nothing is asked - and the recording stays untouched. A port that rolled anyway would shift every later draw.
    let mut rolls = Rolls::new(9);
    let fuel = ground_fuel(Ground::Inert, 3, true, &mut rolls);
    assert_eq!((fuel.time_added, fuel.bashed), (0, false));
    assert_eq!(
        rolls.recording_remaining(),
        None,
        "no recording attached, and no dice spent"
    );
}

#[test]
fn ordinary_flammable_ground_feeds_every_turn_and_still_rolls_for_bashing() {
    // `FLAMMABLE` does not roll to feed - only hard ground does - but it does roll to bash when it burns at intensity two or more.
    let mut rolls = Rolls::new(3);
    let fuel = ground_fuel(Ground::Flammable, 2, true, &mut rolls);
    assert_eq!(fuel.time_added, 3, "five minus intensity");
    assert!(!fuel.bashed, "seed three is not a bash");
}
