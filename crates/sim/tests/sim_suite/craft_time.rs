//! How long a batch takes.
//!
//! Transcribed from `recipe::batch_time` (`src/recipe.cpp:108-149`).

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_sim::batch_time_moves;

const SECOND: i64 = 100;

#[test]
fn a_recipe_that_does_not_batch_simply_multiplies() {
    assert_eq!(
        batch_time_moves(10 * SECOND, 3, 0.0, 0.0, 1.0, 0),
        30 * SECOND
    );
}

#[test]
fn batching_saves_time_per_unit_and_flattens_out() {
    let one = batch_time_moves(10 * SECOND, 1, 0.8, 50.0, 1.0, 0);
    let twenty = batch_time_moves(10 * SECOND, 20, 0.8, 50.0, 1.0, 0);
    assert_eq!(one, 10 * SECOND, "one unit is unchanged");
    assert!(
        twenty < 20 * one,
        "twenty is much less than twenty times one"
    );
    let fifty = batch_time_moves(10 * SECOND, 50, 0.8, 50.0, 1.0, 0);
    assert!(
        fifty < 50 * one * 8 / 10 + 50 * one * 2 / 10,
        "the saving approaches but never passes the batch scale"
    );
}

#[test]
fn an_assistant_takes_a_cut_and_never_makes_it_slower() {
    let alone = batch_time_moves(10 * SECOND, 4, 0.0, 0.0, 1.0, 0);
    let one_helper = batch_time_moves(10 * SECOND, 4, 0.0, 0.0, 1.0, 1);
    let two_helpers = batch_time_moves(10 * SECOND, 4, 0.0, 0.0, 1.0, 2);
    assert_eq!(one_helper, alone * 3 / 4);
    assert_eq!(two_helpers, alone * 6 / 10);
}

#[test]
fn a_speed_of_zero_is_treated_as_full_speed() {
    // The reference's own comment: this is what a recipe shows when it cannot be crafted in the dark.
    assert_eq!(
        batch_time_moves(10 * SECOND, 2, 0.0, 0.0, 0.0, 0),
        batch_time_moves(10 * SECOND, 2, 0.0, 0.0, 1.0, 0)
    );
}

#[test]
fn no_batch_is_ever_faster_than_a_single_unit() {
    // With a huge batch scale, the floor at `local_time` is what stops the sum going under it.
    let result = batch_time_moves(10 * SECOND, 1, 10.0, 50.0, 1.0, 0);
    assert!(
        result >= 10 * SECOND,
        "one unit cannot take less than one unit"
    );
}

#[test]
fn half_speed_takes_twice_as_long() {
    assert_eq!(
        batch_time_moves(10 * SECOND, 2, 0.0, 0.0, 0.5, 0),
        40 * SECOND
    );
}
