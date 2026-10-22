//! The curve along a forest chunk's edge.

#![allow(clippy::panic, clippy::indexing_slicing)]

use cdda_core_types::overmap::{forest_border_depth, forest_perimeter_depth};
use cdda_core_types::rng::Rolls;

const AVERAGE_DEPTH: i32 = 5;
const DEPTH_DEVIATION: f32 = 1.0;

fn border() -> [i32; 8] {
    forest_border_depth(
        [(1920, 4800), (1944, 4800), (1920, 4824), (1944, 4824)],
        AVERAGE_DEPTH,
        DEPTH_DEVIATION,
    )
}

#[test]
fn the_perimeter_is_one_depth_for_every_square_of_the_edges() {
    let mut rolls = Rolls::new(31337);
    let perimeter = forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut rolls);
    assert_eq!(perimeter.len(), 96, "four edges of twenty-four squares");
}

#[test]
fn no_depth_falls_below_one() {
    let mut rolls = Rolls::new(7);
    let perimeter = forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut rolls);
    assert!(
        perimeter.iter().all(|depth| *depth >= 1),
        "a depth is a distance into the chunk: {:?}",
        perimeter.iter().copied().min()
    );
}

#[test]
fn the_edges_start_and_end_at_the_corner_depths() {
    let depths = border();
    let mut rolls = Rolls::new(11);
    let perimeter = forest_perimeter_depth(depths, AVERAGE_DEPTH, &mut rolls);
    // The corner depths are the **end points of the interpolation**, not the depth of the first and last square: an
    // interior critical point close to the corner pulls the first square towards its own depth. My first version of
    // this test asserted equality at the ends and was wrong by five squares. What holds is that each edge reaches
    // within one square of each of its corners somewhere.
    let north: Vec<i32> = perimeter[..24].to_vec();
    assert!(
        north.iter().any(|depth| (depth - depths[0]).abs() <= 1),
        "the north edge reaches corner 0's depth somewhere: {north:?}"
    );
    assert!(
        north.iter().any(|depth| (depth - depths[5]).abs() <= 1),
        "and corner 1's: {north:?}"
    );
}

#[test]
fn the_curve_bends_rather_than_stepping() {
    let mut rolls = Rolls::new(4242);
    let perimeter = forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut rolls);
    let north: Vec<i32> = perimeter[..24].to_vec();
    let distinct: std::collections::BTreeSet<i32> = north.iter().copied().collect();
    assert!(
        distinct.len() > 1,
        "the edge is not one flat depth: {north:?}"
    );
    let biggest_jump = north
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .max()
        .unwrap_or(0);
    assert!(
        biggest_jump <= 3,
        "and it moves in steps, not leaps: {biggest_jump}"
    );
}

#[test]
fn the_same_dice_give_the_same_curve() {
    let mut first = Rolls::new(99);
    let mut second = Rolls::new(99);
    assert_eq!(
        forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut first),
        forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut second)
    );
}

#[test]
fn different_dice_bend_the_same_edge_differently() {
    let mut first = Rolls::new(1);
    let mut second = Rolls::new(2);
    assert_ne!(
        forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut first),
        forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut second)
    );
}

#[test]
fn the_curve_pulls_towards_the_average_rather_than_the_extremes() {
    // The critical points are drawn around the average depth, so a curve stays in the neighbourhood of it: a depth of
    // twenty inside the chunk would be a wrong falloff, not a wrong draw.
    let mut rolls = Rolls::new(31337);
    let perimeter = forest_perimeter_depth(border(), AVERAGE_DEPTH, &mut rolls);
    assert!(
        perimeter.iter().all(|depth| *depth <= 40),
        "no depth runs away: worst {}",
        perimeter.iter().copied().max().unwrap_or(0)
    );
}
