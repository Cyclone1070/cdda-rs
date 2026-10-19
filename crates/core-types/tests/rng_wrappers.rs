//! The random-number wrappers against the same standard library the reference
//! links against.
//!
//! `corpus/rng/wrappers.csv` is produced by `corpus/rng/wrappers.cpp`, compiled
//! with the platform's libc++ and run once. Every row is one call, in the order
//! the reference would make them, against a single engine seeded with 12345 -
//! so the file pins the whole stream, not isolated values. If the port ever
//! consumed one draw more or fewer than the reference, every row after the
//! difference would move.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    // The comparison is deliberately exact: the point is that the port and the
    // reference agree bit for bit, not that they are close.
    clippy::float_cmp
)]

use cdda_core_types::rng::{Rolls, djb2_hash, rng_sequence};
use std::path::Path;

fn corpus() -> Vec<(String, f64)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/rng/wrappers.csv");
    let text = std::fs::read_to_string(&path).expect("corpus/rng/wrappers.csv");
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (name, value) = line.split_once(',').expect("name,value");
            (name.to_owned(), value.parse::<f64>().expect("a number"))
        })
        .collect()
}

#[test]
fn every_wrapper_draws_the_same_value_the_reference_does() {
    let rows = corpus();
    assert!(rows.len() > 70, "the corpus covers every wrapper");

    let mut rolls = Rolls::new(12_345);
    let mut mismatches = Vec::new();
    let mut index = 0;

    // The call order below is the order `corpus/rng/wrappers.cpp` makes them.
    let mut expect = |name: &str, value: f64, mismatches: &mut Vec<String>| {
        if let Some((expected_name, expected)) = rows.get(index) {
            if expected_name != name || *expected != value {
                mismatches.push(format!(
                    "row {index}: {name} = {value:.20} but corpus says {expected_name} = {expected:.20}"
                ));
            }
        } else {
            mismatches.push(format!("row {index}: {name} is not in the corpus"));
        }
        index += 1;
    };

    for _ in 0..8 {
        expect("rng.0.100", f64::from(rolls.rng(0, 100)), &mut mismatches);
    }
    for _ in 0..4 {
        expect("rng.5.5", f64::from(rolls.rng(5, 5)), &mut mismatches);
    }
    // Reversed bounds are swapped, not refused.
    expect(
        "rng.reversed",
        f64::from(rolls.rng(100, 0)),
        &mut mismatches,
    );
    expect(
        "rng.negative",
        f64::from(rolls.rng(-50, -10)),
        &mut mismatches,
    );
    for _ in 0..4 {
        expect("rng_float", rolls.rng_float(0.0, 1.0), &mut mismatches);
    }
    expect(
        "rng_float.offset",
        rolls.rng_float(-2.5, 7.5),
        &mut mismatches,
    );
    for _ in 0..6 {
        expect(
            "normal_roll.0.1",
            rolls.normal_roll(0.0, 1.0),
            &mut mismatches,
        );
    }
    expect(
        "normal_roll.10.2",
        rolls.normal_roll(10.0, 2.0),
        &mut mismatches,
    );
    for _ in 0..6 {
        expect(
            "rng_normal.0.10",
            rolls.rng_normal(0.0, 10.0),
            &mut mismatches,
        );
    }
    expect(
        "rng_normal.equal",
        rolls.rng_normal(4.0, 4.0),
        &mut mismatches,
    );
    expect(
        "rng_normal.reversed",
        rolls.rng_normal(10.0, 0.0),
        &mut mismatches,
    );
    for _ in 0..4 {
        expect("rng_bits", f64::from(rolls.rng_bits()), &mut mismatches);
    }
    for _ in 0..12 {
        expect(
            "one_in.4",
            f64::from(u8::from(rolls.one_in(4))),
            &mut mismatches,
        );
    }
    expect(
        "one_in.1",
        f64::from(u8::from(rolls.one_in(1))),
        &mut mismatches,
    );
    expect(
        "one_in.0",
        f64::from(u8::from(rolls.one_in(0))),
        &mut mismatches,
    );
    for _ in 0..8 {
        expect(
            "x_in_y.1.3",
            f64::from(u8::from(rolls.x_in_y(1.0, 3.0))),
            &mut mismatches,
        );
    }
    expect(
        "x_in_y.zero",
        f64::from(u8::from(rolls.x_in_y(0.0, 1.0))),
        &mut mismatches,
    );
    expect(
        "x_in_y.all",
        f64::from(u8::from(rolls.x_in_y(5.0, 5.0))),
        &mut mismatches,
    );
    for _ in 0..6 {
        expect("dice.3.6", f64::from(rolls.dice(3, 6)), &mut mismatches);
    }
    expect("dice.0.6", f64::from(rolls.dice(0, 6)), &mut mismatches);
    expect(
        "roll_remainder.1.3",
        f64::from(rolls.roll_remainder(1.3)),
        &mut mismatches,
    );
    expect(
        "roll_remainder.-2.7",
        f64::from(rolls.roll_remainder(-2.7)),
        &mut mismatches,
    );
    expect(
        "roll_remainder.5.0",
        f64::from(rolls.roll_remainder(5.0)),
        &mut mismatches,
    );

    // Sequences use their own engine, so the shared stream is untouched by them.
    for value in rng_sequence(8, 0, 9, 42) {
        expect("rng_sequence.42", f64::from(value), &mut mismatches);
    }
    for value in rng_sequence(4, -3, 3, 7) {
        expect(
            "rng_sequence.7.reversed",
            f64::from(-value),
            &mut mismatches,
        );
    }

    // Appended to the corpus after everything else, so the rows above keep
    // their order and their stream.
    for _ in 0..6 {
        expect(
            "exponential_roll.2",
            rolls.exponential_roll(2.0),
            &mut mismatches,
        );
    }
    expect(
        "rng_exponential.0.5",
        rolls.rng_exponential(0.0, 5.0),
        &mut mismatches,
    );
    expect(
        "rng_exponential.2.6",
        rolls.rng_exponential(2.0, 6.0),
        &mut mismatches,
    );
    expect(
        "rng_exponential.degenerate",
        rolls.rng_exponential(4.0, 4.0),
        &mut mismatches,
    );

    for line in mismatches.iter().take(10) {
        println!("  {line}");
    }
    assert_eq!(index, rows.len(), "every corpus row was claimed");
    assert!(mismatches.is_empty(), "{} differ", mismatches.len());
}

#[test]
fn the_corpus_can_fail() {
    // A guard on the guard: a different seed must not reproduce the first row,
    // so a test that passes is evidence rather than a tautology.
    let rows = corpus();
    let mut rolls = Rolls::new(12_346);
    let first = rows.first().expect("a first row").1;
    assert_ne!(f64::from(rolls.rng(0, 100)), first);
}

#[test]
fn the_world_seed_hash_is_djb2() {
    // Reference: `djb2_hash` (`src/rng.cpp:152-165`) - hash * 33 + byte, from
    // 5381, over the bytes up to the terminator.
    assert_eq!(djb2_hash(""), 5381);
    assert_eq!(djb2_hash("a"), 5381 * 33 + i32::from(b'a'));
    assert_eq!(
        djb2_hash("world"),
        "world"
            .bytes()
            .fold(5381_u32, |hash, byte| hash
                .wrapping_mul(33)
                .wrapping_add(u32::from(byte)))
            .cast_signed()
    );
}
