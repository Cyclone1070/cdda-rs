//! The wandering edge of a forest chunk.

#![allow(clippy::panic)]

use cdda_core_types::overmap::forest_border_depth;

// The reference's own constants for a map chunk: average depth 5, peak deviation 1.
const AVERAGE_DEPTH: i32 = 5;
const DEPTH_DEVIATION: f32 = 1.0;

#[test]
fn the_four_corners_give_eight_depths() {
    let depths = forest_border_depth(
        [(0, 0), (24, 0), (0, 24), (24, 24)],
        AVERAGE_DEPTH,
        DEPTH_DEVIATION,
    );
    assert_eq!(depths.len(), 8);
    assert!(
        depths.iter().all(|depth| *depth >= 0),
        "a depth is a distance: {depths:?}"
    );
}

#[test]
fn the_same_corners_give_the_same_edge() {
    let corners = [(1920, 4800), (1944, 4800), (1920, 4824), (1944, 4824)];
    let first = forest_border_depth(corners, AVERAGE_DEPTH, DEPTH_DEVIATION);
    let second = forest_border_depth(corners, AVERAGE_DEPTH, DEPTH_DEVIATION);
    assert_eq!(
        first, second,
        "the edges come from the position, not from dice"
    );
}

#[test]
fn a_different_position_gives_a_different_edge() {
    let first = forest_border_depth(
        [(0, 0), (24, 0), (0, 24), (24, 24)],
        AVERAGE_DEPTH,
        DEPTH_DEVIATION,
    );
    let second = forest_border_depth(
        [(24, 0), (48, 0), (24, 24), (48, 24)],
        AVERAGE_DEPTH,
        DEPTH_DEVIATION,
    );
    assert_ne!(first, second, "one chunk along, a different edge");
}

#[test]
fn the_depths_stay_near_the_average_rather_than_wandering_far() {
    // The deviation is one square, so a depth far from five would mean the transform is wrong. Over many positions,
    // every depth lands within a few squares of the average.
    let mut seen = Vec::new();
    // Positions whose corner hash happens to be zero are skipped: the transform takes the logarithm of the four
    // coordinates and a zero draw makes it infinite, which the reference clamps and the test below pins. A position
    // is degenerate whenever one coordinate is twice the other, so those are skipped by their result rather than by
    // an assumed pattern.
    for step in 0..60 {
        let x = i64::from(step) * 24;
        let depths = forest_border_depth(
            [(x, 0), (x + 24, 0), (x, 24), (x + 24, 24)],
            AVERAGE_DEPTH,
            DEPTH_DEVIATION,
        );
        if depths.iter().all(|depth| *depth < 100) {
            seen.extend(depths);
        }
    }
    assert!(
        seen.len() > 100,
        "most positions give ordinary depths: {}",
        seen.len()
    );
    assert!(
        seen.iter().all(|depth| *depth <= 12),
        "no depth runs away: worst {}",
        seen.iter().copied().max().unwrap_or(0)
    );
    assert!(
        seen.iter().any(|depth| *depth != AVERAGE_DEPTH),
        "and they are not all the average"
    );
}

#[test]
fn a_corner_at_the_origin_is_the_degenerate_case_the_reference_also_has() {
    // A corner at (0,0) hashes to zero, so the unit draw is zero and the transform takes the logarithm of it: the
    // magnitude becomes infinite and the reference clamps the result to the largest integer. That is not a bug in the
    // port - it is what `log(0)` does in the reference's own arithmetic - and the port reproduces it rather than
    // hiding it behind a guard, because a guard here would change worlds generated near the origin.
    let depths = forest_border_depth(
        [(0, 0), (0, 0), (0, 0), (0, 0)],
        AVERAGE_DEPTH,
        DEPTH_DEVIATION,
    );
    assert_eq!(&depths[..4], &[i32::MAX; 4], "the cosine half is infinite");
    assert_eq!(
        &depths[4..],
        &[0; 4],
        "and the sine half has no magnitude to carry"
    );
}
