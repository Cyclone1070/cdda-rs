//! Playing the reference's own dice instead of drawing fresh ones.
//!
//! The reference records every die it rolls (`CDDA_RNG_TRACE`). A port that replays that recording is comparing **rules on
//! identical dice** rather than two independent draws that happen to agree - the difference between a score and a proof, and
//! the only way a generated chunk can be compared square by square.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use cdda_core_types::rng::{Recording, Rolls};

#[test]
fn a_recorded_bits_draw_comes_back_exactly() {
    // The first two draws of a real run, copied from a recording this project made: a bound draw and a raw 32-bit value.
    let recording =
        Recording::parse("int 0 2147483647 1577506885\nbits 3894119815\n").expect("a recording");
    let mut rolls = Rolls::replaying(recording);
    assert_eq!(rolls.rng(0, 2_147_483_647), 1_577_506_885);
    assert_eq!(rolls.rng_bits(), 3_894_119_815);
    assert_eq!(rolls.recording_remaining(), Some(0), "both dice are spent");
}

#[test]
fn the_bounds_are_checked_rather_than_ignored() {
    // A recorded draw knows the range it was asked for. Replaying it under a different range means the port is asking a
    // different question than the reference did, and silently answering would hide exactly the divergence this exists to
    // catch - so it fails, loudly, naming both ranges.
    let recording = Recording::parse("int 0 10 7\n").expect("a recording");
    let mut rolls = Rolls::replaying(recording);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rolls.rng(0, 20)));
    assert!(
        outcome.is_err(),
        "a different range must not be answered silently"
    );
}

#[test]
fn a_run_without_a_recording_draws_its_own_dice() {
    // The ordinary path: no recording attached, so the engine answers and nothing pretends a reference was replayed.
    let mut rolls = Rolls::new(1234);
    assert_eq!(rolls.recording_remaining(), None);
    let first = rolls.rng_bits();
    let mut again = Rolls::new(1234);
    assert_eq!(
        again.rng_bits(),
        first,
        "the same seed draws the same value"
    );
}

#[test]
fn a_recording_is_parsed_into_the_draws_it_holds() {
    let recording = Recording::parse("bits 7\nint -3 42 0\n\n").expect("a recording");
    assert_eq!(recording.remaining(), 2, "a blank line is not a draw");
    let mut rolls = Rolls::replaying(recording);
    assert_eq!(rolls.rng_bits(), 7);
    assert_eq!(rolls.rng(-3, 42), 0);
}

#[test]
fn an_unreadable_line_is_refused_with_the_line_in_the_message() {
    let error = Recording::parse("bits not-a-number\n").expect_err("a bad line");
    assert!(error.contains("not-a-number"), "{error}");
    let error = Recording::parse("roll 4\n").expect_err("an unknown kind");
    assert!(error.contains("roll"), "{error}");
}

#[test]
fn a_recording_running_out_is_a_caller_bug_and_says_so() {
    // Running dry means the port spent more dice than the reference did on the same rules: a divergence in the order of
    // draws, which is the thing replay exists to expose.
    let recording = Recording::parse("bits 1\n").expect("a recording");
    let mut rolls = Rolls::replaying(recording);
    assert_eq!(rolls.rng_bits(), 1);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rolls.rng_bits()));
    assert!(
        outcome.is_err(),
        "an exhausted recording must not invent a die"
    );
}
