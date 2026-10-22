//! Hot air, checked against the dice the reference actually rolled for it.
//!
//! The recording labels the fire processor's phases, so the hot-air phase of each turn is a list of draws with no guessing: a gate roll
//! `rng( 0, 4 - adjacent_fires )`, then - only when that gate is non-zero - five pairs of `rng( -1, 1 )` for the squares it lands on.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{hot_air_field, hot_air_squares, wants_hot_air};
use cdda_core_types::rng::{Recording, Rolls};

fn hot_air_phases() -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor.rolls.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the recording at {}: {error}", path.display()));
    let mut phases: Vec<Vec<String>> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some(label) = line.strip_prefix("mark ") {
            if label == "fire.before_hot_air" {
                inside = true;
                phases.push(Vec::new());
            } else {
                inside = false;
            }
            continue;
        }
        if inside && let Some(phase) = phases.last_mut() {
            phase.push(line.to_string());
        }
    }
    phases
}

fn recording_of(lines: &[String]) -> Rolls {
    let text: String = lines.join("\n");
    Rolls::replaying(Recording::parse(&text).expect("the phase parses"))
}

#[test]
fn the_gate_is_a_non_zero_roll_and_a_fire_with_five_neighbours_does_not_roll() {
    // Four turns of the session, four gate rolls: a zero is a refusal and anything else is a request, which is why the gate is a `rng( 0, 4 )` rather
    // than a `one_in( 5 )` - the reference wants to know *how* close the square was to being too crowded as well as whether it is.
    let phases = hot_air_phases();
    assert_eq!(phases.len(), 4, "{phases:?}");
    let wanted = [true, false, true, false];
    for (index, phase) in phases.iter().enumerate() {
        let gate = phase.first().expect("a gate roll");
        let mut rolls = recording_of(std::slice::from_ref(gate));
        assert_eq!(
            wants_hot_air(0, &mut rolls),
            wanted[index],
            "turn {index}: {gate}"
        );
    }
    // And no die at all when five fires are already nearby.
    let mut rolls = Rolls::new(1);
    assert!(!wants_hot_air(5, &mut rolls));
}

#[test]
fn requested_hot_air_lands_on_five_squares_drawn_x_then_y() {
    // The session's second and fourth turns asked for hot air; each is followed by exactly ten `int -1 1` draws, and the port must consume them in
    // the same order and land on the same squares.
    let phases = hot_air_phases();
    for (index, phase) in phases.iter().enumerate() {
        let tail: Vec<String> = phase
            .iter()
            .skip(1)
            .filter(|line| line.starts_with("int -1 1 "))
            .take(10)
            .cloned()
            .collect();
        if tail.len() < 10 {
            continue;
        }
        let mut rolls = recording_of(&tail);
        let squares = hot_air_squares(&mut rolls);
        let expected: Vec<(i32, i32)> = tail
            .chunks(2)
            .map(|pair| {
                let value = |line: &String| -> i32 {
                    line.rsplit(' ')
                        .next()
                        .expect("a value")
                        .parse()
                        .expect("a number")
                };
                (value(&pair[0]), value(&pair[1]))
            })
            .collect();
        assert_eq!(squares.to_vec(), expected, "turn {index}");
        assert_eq!(
            rolls.recording_remaining(),
            Some(0),
            "turn {index} left dice unplayed"
        );
    }
}

#[test]
fn the_hot_air_field_follows_the_fire_intensity_and_nothing_else() {
    assert_eq!(hot_air_field(1), Some(1));
    assert_eq!(hot_air_field(4), Some(4));
    assert_eq!(hot_air_field(0), None, "no field for a fire that is out");
    assert_eq!(
        hot_air_field(5),
        None,
        "and none for an intensity the reference treats as a bug"
    );
}
