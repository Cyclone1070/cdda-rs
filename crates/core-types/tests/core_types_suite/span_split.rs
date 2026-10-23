//! The span list the light sweep carries, and the four pieces a change of opacity cuts it into.
//!
//! Reference: `slope` and `span` (`src/shadowcasting.cpp:19-63`) and `split_span` (`:69-159`). The cast does not walk rays; it sweeps a widening wedge and keeps a list of
//! spans, each one a frustum of that wedge that met the same opacity so far. When a square's opacity differs from the span's, the span is cut into up to four pieces:
//!
//! ```text
//! +-------+ <- end major
//! |   D   |  not yet processed rows, old opacity
//! +---+---+ <- leading edge major
//! | B | C |  B: already-processed squares of this row, old opacity
//! +---+---+  C: the rest of this row, new opacity
//! |   A   |  previously processed rows, old opacity
//! +-------+ <- start major
//! ```
//!
//! Which pieces exist depends on where the edges fall, and the two `skip_` flags are what stop the new spans re-checking the row and column that caused the split - the
//! reference notes that without them the sweep can loop forever.

#![allow(clippy::expect_used)]
// A span list is a list: the tests name the pieces by where they ended up, and a wrong index is the failure they are looking for.
#![allow(clippy::indexing_slicing)]

use cdda_core_types::shadowcasting::{Slope, Span, split_span};

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-6,
        "{what}: got {passed}, expected {expected}"
    );
}

#[test]
fn a_slope_compares_by_cross_multiplication_and_not_by_its_two_numbers() {
    // `operator<` (`shadowcasting.cpp:34-38`): `a/b < c/d` is `a*d < c*b`. Comparing rise first would make 1/3 look smaller than 2/5, which it is not.
    assert!(Slope::new(1, 3) < Slope::new(2, 5), "0.333 is below 0.4");
    assert!(Slope::new(2, 5) > Slope::new(1, 3));
    assert_eq!(Slope::new(1, 2), Slope::new(2, 4), "the same angle twice");
    assert!(Slope::new(2, 4) <= Slope::new(1, 2));
}

#[test]
fn a_negative_run_flips_the_rise_and_keeps_the_run_positive() {
    // The constructor (`shadowcasting.cpp:20-29`) takes `abs( run )` and negates the rise when the run came in negative, so every run is positive and the
    // cross-multiplied comparison above stays valid.
    assert_eq!(Slope::new(1, -2), Slope::new(-1, 2));
    assert_eq!(Slope::new(1, -2).run(), 2);
    assert_eq!(Slope::new(1, -2).rise(), -1);
    assert_eq!(Slope::new(-3, 4).rise(), -3, "and a negative rise stays");
}

#[test]
fn an_opaque_square_leaves_the_processed_part_and_starts_a_new_span_beyond_it() {
    // A span one row tall, meeting a wall part way along. `A` and `B` are processed rows/columns and stay in this span; `D` is everything past the wall and becomes a
    // new span after it with the old cumulative transparency.
    let mut spans = vec![Span::new(
        Slope::new(0, 1),
        Slope::new(1, 1),
        Slope::new(0, 1),
        Slope::new(1, 1),
        0.5,
    )];
    let mut current_transparency = 0.5;
    let mut new_start_minor = Slope::new(1, 1);
    let moved = split_span(
        &mut spans,
        0,
        &mut current_transparency,
        0.0,
        2,
        &mut new_start_minor,
        Slope::new(0, 1),
        Slope::new(1, 2),
        Slope::new(0, 1),
        Slope::new(1, 2),
    );
    assert_eq!(spans.len(), 2, "one piece split off");
    assert_eq!(moved, 0, "the span being processed did not move");
    assert_close(
        spans[0].cumulative_value,
        0.5,
        "the span keeps its own transparency history",
    );
    assert_close(
        spans[1].cumulative_value,
        0.5,
        "and so does the piece beyond the wall",
    );
    assert!(
        spans[1].skip_first_row,
        "the new span starts on the next row"
    );
    assert_close(current_transparency, 0.0, "the wall is now what we are in");
}

#[test]
fn a_transparent_square_of_another_opacity_cuts_out_the_processed_part() {
    // Both squares are transparent but with different transparency, so the processed part becomes pieces of its own - with the **next** cumulative value - while the
    // remainder carries on. `skip_first_column` is set so the same column is not checked twice.
    let mut spans = vec![Span::new(
        Slope::new(0, 1),
        Slope::new(2, 1),
        Slope::new(0, 1),
        Slope::new(1, 1),
        0.25,
    )];
    let mut current_transparency = 0.25;
    let mut new_start_minor = Slope::new(1, 1);
    // The pieces cut off go *before* the span being processed, so the span that carries on moves - which is why the function answers with its index.
    let moved = split_span(
        &mut spans,
        0,
        &mut current_transparency,
        0.5,
        3,
        &mut new_start_minor,
        Slope::new(1, 3),
        Slope::new(2, 3),
        Slope::new(0, 1),
        Slope::new(1, 2),
    );
    assert!(
        spans.len() >= 2,
        "the part of this row already covered is cut off"
    );
    assert!(
        spans[moved].skip_first_column,
        "and the span that carries on is told not to re-check the column that caused the cut"
    );
    assert_close(
        spans[0].cumulative_value,
        ((3.0 - 1.0) * 0.25 + 0.25) / 3.0,
        "the pieces carry the running mean, one step further on",
    );
    assert_close(current_transparency, 0.5, "we are now in the new square");
}
