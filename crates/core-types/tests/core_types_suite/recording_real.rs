//! Recording the dice that are not whole numbers.
//!
//! The reference's `rng_float` (`src/rng.cpp:84-95`) is the only door to a real draw, and every rule that rounds probabilistically goes through it:
//! `roll_remainder`, `x_in_y`, `rng_normal`, `rng_exponential`. Until now the trace did not record those at all, which meant a session that made one was
//! silently unreplayable - the port would take the next whole number the reference had drawn for something else, and nothing said a die was missing.
//!
//! A real is recorded with its bounds and its value at full precision, because a replay must reach the *same* value the reference reached, not a value in
//! the same range: the whole point of `roll_remainder( 0.0484 )` is whether it landed.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::rng::{Draw, Recording, Rolls};

#[test]
fn a_recorded_real_keeps_its_value_and_its_bounds() {
    let recording = Recording::parse("real 0 1 0.0484\n").expect("a recording");
    assert_eq!(
        recording.draws.first().copied(),
        Some(Draw::Real {
            low: 0.0,
            high: 1.0,
            drawn: 0.0484,
        })
    );
}

#[test]
fn a_replay_answers_a_float_question_with_the_recorded_value() {
    let mut rolls = Rolls::replaying(Recording::parse("real 0 1 0.25\n").expect("a recording"));
    assert!((rolls.rng_float(0.0, 1.0) - 0.25).abs() < f64::EPSILON);
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn a_float_question_the_reference_did_not_ask_is_caught() {
    // Same range check as a whole number: a port that asks `rng_float( 0, 1 )` where the reference asked `rng_float( 0, 4 )` is desynchronised, and the
    // message has to say so rather than quietly answer.
    let mut rolls = Rolls::replaying(Recording::parse("real 0 4 1.5\n").expect("a recording"));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = rolls.rng_float(0.0, 1.0);
    }));
    assert!(caught.is_err(), "a mismatched float draw must be caught");
}

#[test]
fn whole_numbers_and_reals_share_one_stream_in_the_order_they_happened() {
    // The reference makes one stream, not one per kind, so a replay must take them in order however they are mixed.
    let mut rolls = Rolls::replaying(
        Recording::parse("int 0 2 1\nreal 0 1 0.75\nint 1 100 40\n").expect("a recording"),
    );
    assert_eq!(rolls.rng(0, 2), 1);
    assert!((rolls.rng_float(0.0, 1.0) - 0.75).abs() < f64::EPSILON);
    assert_eq!(rolls.rng(1, 100), 40);
    assert_eq!(rolls.recording_remaining(), Some(0));
}
