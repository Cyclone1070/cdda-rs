//! Whether creating the avatar moves the dice.
//!
//! Source: `corpus/rng-probe.json` is `--dump-rng-probe`: three seeds, three draws each, taken
//! **before** and **after** the reference builds its avatar. The two halves being identical is the
//! fact worth pinning - a game that drew while setting up a character would shift every later roll in
//! a save - and the numbers themselves pin the distribution, which is `rng( 1, 100 )`
//! (`harness.cpp:2264`), so an off-by-one in the bounds shows up as three wrong numbers per seed.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::rng::MinStdRand0;
use cdda_json::{Value, parse};
use std::path::Path;

struct Sample {
    seed: u32,
    draws: Vec<i32>,
}

fn halves() -> (Vec<Sample>, Vec<Sample>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/rng-probe.json");
    let text = std::fs::read(path).expect("the rng probe exists");
    let value = parse(&text).expect("the probe is valid JSON");
    let Value::Object(root) = value else {
        panic!("the probe's root is not an object");
    };
    let read = |key: &str| -> Vec<Sample> {
        let Some(Value::Array(entries)) = root.get(key) else {
            panic!("the probe has no {key}");
        };
        entries
            .iter()
            .map(|entry| {
                let Value::Object(object) = entry else {
                    panic!("a sample is not an object");
                };
                let seed = match object.get("seed") {
                    Some(Value::Int(found)) => u32::try_from(*found).expect("a seed fits"),
                    _ => panic!("a sample has no seed"),
                };
                let draws = match object.get("draws") {
                    Some(Value::Array(list)) => list
                        .iter()
                        .map(|draw| match draw {
                            Value::Int(found) => i32::try_from(*found).expect("a draw"),
                            _ => panic!("a draw is not a number"),
                        })
                        .collect(),
                    _ => panic!("a sample has no draws"),
                };
                Sample { seed, draws }
            })
            .collect()
    };
    (read("before_avatar"), read("after_avatar"))
}

#[test]
fn the_reference_draws_nothing_while_building_its_avatar() {
    let (before, after) = halves();

    assert!(before.len() >= 3, "the probe holds at least three seeds");
    for (before, after) in before.iter().zip(after.iter()) {
        assert_eq!(before.seed, after.seed, "the halves are the same seeds");
        assert_eq!(
            before.draws, after.draws,
            "seed {}: the avatar's construction drew from the engine",
            before.seed
        );
    }
}

#[test]
fn three_draws_from_each_seed_are_the_reference_s_own() {
    let (before, _) = halves();
    let mut compared = 0;
    let mut disagreements = Vec::new();
    for sample in &before {
        let mut engine = MinStdRand0::seeded(sample.seed);
        let ours: Vec<i32> = sample
            .draws
            .iter()
            .map(|_| engine.uniform_int(1, 100))
            .collect();
        compared += ours.len();
        // **The reference's list is reversed**, and that is the harness rather than the engine: it
        // prints three `rng( 1, 100 )` calls as arguments of one `fprintf` (`harness.cpp:2264`), and
        // the order those arguments are evaluated in is unspecified - gcc walks them right to left,
        // so the third draw is printed first. The values are the engine's; the order is the printer's.
        let theirs: Vec<i32> = sample.draws.iter().rev().copied().collect();
        if ours != theirs {
            disagreements.push(format!(
                "seed {}: reference {:?} (printed reversed), port {:?}",
                sample.seed, sample.draws, ours
            ));
        }
    }
    assert!(compared >= 9, "only {compared} draws were compared");
    assert!(disagreements.is_empty(), "{}", disagreements.join("\n"));
}
