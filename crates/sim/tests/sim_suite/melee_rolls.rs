//! The accuracy roll an attack makes, against the reference's own dice.
//!
//! Source: `corpus/melee-rolls.json` is `--dump-melee-rolls`: three seeds, three accuracies, four rolls
//! each, straight from `melee::melee_hit_range` (`melee.cpp:2912`), which is
//! `normal_roll( accuracy * 5, 25 )`. It pins **both** numbers - a wrong mean moves every value, and a
//! wrong deviation changes their spread - which is what a single "it hit" assertion could never do.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use cdda_core_types::rng::Rolls;
use cdda_json::{Value, parse};
use cdda_sim::melee::melee_hit_range;
use std::path::Path;

struct Sample {
    seed: u32,
    accuracy: f32,
    rolls: Vec<f32>,
}

fn samples() -> Vec<Sample> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/melee-rolls.json");
    let text = std::fs::read(path).expect("the melee-rolls dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(entries)) = root.get("samples") else {
        panic!("the dump has no samples");
    };
    let number = |entry: &Value, key: &str| -> f64 {
        match entry {
            Value::Object(object) => match object.get(key) {
                Some(Value::Float(found)) => *found,
                Some(Value::Int(found)) => *found as f64,
                _ => panic!("a sample has no {key}"),
            },
            _ => panic!("a sample is not an object"),
        }
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
            let rolls = match object.get("rolls") {
                Some(Value::Array(list)) => list
                    .iter()
                    .map(|roll| match roll {
                        Value::Float(found) => *found as f32,
                        Value::Int(found) => *found as f32,
                        _ => panic!("a roll is not a number"),
                    })
                    .collect(),
                _ => panic!("a sample has no rolls"),
            };
            Sample {
                seed,
                #[allow(clippy::cast_possible_truncation)]
                accuracy: number(entry, "accuracy") as f32,
                rolls,
            }
        })
        .collect()
}

#[test]
fn every_accuracy_roll_matches_the_reference_to_a_thousandth() {
    let samples = samples();
    assert!(
        samples.len() >= 9,
        "the dump holds {} samples",
        samples.len()
    );
    let mut compared = 0;
    let mut disagreements = Vec::new();
    for sample in &samples {
        let mut rolls = Rolls::new(sample.seed);
        for (index, expected) in sample.rolls.iter().enumerate() {
            let ours = melee_hit_range(&mut rolls, sample.accuracy);
            compared += 1;
            if (ours - expected).abs() > 0.001 {
                disagreements.push(format!(
                    "seed {} accuracy {} roll {index}: reference {expected}, port {ours}",
                    sample.seed, sample.accuracy
                ));
            }
        }
    }
    assert!(compared >= 36, "only {compared} rolls were compared");
    assert!(
        disagreements.is_empty(),
        "{} of {compared} rolls disagreed:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}
