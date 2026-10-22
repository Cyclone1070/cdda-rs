//! The rule that stops a biome poking into a corner it does not own.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{unify_all_borders, unify_continuous_border};

#[test]
fn a_corner_shared_by_two_equal_sides_is_left_alone() {
    let mut counter_clockwise = 2.0;
    let mut clockwise = 3.0;
    let mut this_square = 4.0;
    unify_continuous_border(
        Some("field"),
        None,
        Some("field"),
        Some("forest"),
        &mut counter_clockwise,
        &mut clockwise,
        &mut this_square,
    );
    assert_eq!(
        (counter_clockwise, clockwise, this_square),
        (2.0, 3.0, 4.0),
        "both sides agree"
    );
}

#[test]
fn a_side_matching_the_corner_gives_way_to_this_square() {
    // The corner is field, the counter-clockwise side is field too, and the clockwise side is this forest. The
    // clockwise side is squeezed and this square gains what it lost.
    let mut counter_clockwise = 2.0;
    let mut clockwise = 3.0;
    let mut this_square = 4.0;
    unify_continuous_border(
        Some("field"),
        Some("field"),
        Some("forest"),
        Some("forest"),
        &mut counter_clockwise,
        &mut clockwise,
        &mut this_square,
    );
    assert!(
        clockwise < 3.0,
        "the side that matched the corner lost influence: {clockwise}"
    );
    assert!(
        this_square < 4.0,
        "and the bookkeeping subtracts from this square: {this_square}"
    );
    assert!(
        (counter_clockwise - 2.0).abs() < 1e-6,
        "the other side is untouched"
    );
}

#[test]
fn the_reference_s_second_branch_subtracts_the_other_weight() {
    // The reference's second branch is written with `cw_weight` in a place where the first branch uses `ccw_weight`,
    // and the port keeps that rather than repairing it: the arithmetic is what the port is reproducing.
    let mut counter_clockwise = 3.0;
    let mut clockwise = 2.0;
    let mut this_square = 4.0;
    unify_continuous_border(
        Some("forest"),
        Some("field"),
        Some("field"),
        Some("forest"),
        &mut counter_clockwise,
        &mut clockwise,
        &mut this_square,
    );
    assert!(
        counter_clockwise < 3.0,
        "the clockwise-matching side is squeezed: {counter_clockwise}"
    );
    // 2 / (4 + 2) = 1/3, taken from 4 - 2, not from 4 - 1/3, which is what the reference's arithmetic does.
    // 3 / (4 + 3) = 0.4286, not 3 / (4 + 2): the branch divides by the weight it is shrinking.
    assert!(
        (counter_clockwise - 0.428_571_44).abs() < 1e-5,
        "{counter_clockwise}"
    );
    assert!(
        (this_square - 2.0).abs() < 1e-6,
        "and this square loses the other side's weight: {this_square}"
    );
}

#[test]
fn each_quadrant_picks_its_own_three_biomes() {
    // A square in the north-west weighs west against north across the north-west corner. Here the corner is field and
    // the west side matches it, so the north side should take the pressure.
    let neighbours = [
        Some("forest"), // north
        Some("field"),  // east
        Some("field"),  // south
        Some("field"),  // west
        Some("field"),  // north-east
        Some("field"),  // south-east
        Some("field"),  // south-west
        Some("field"),  // north-west
    ];
    let mut weights = [1.0_f32; 4];
    let mut self_weight = 4.0;
    unify_all_borders(
        (2, 2),
        24,
        neighbours,
        &mut weights,
        Some("forest"),
        &mut self_weight,
    );
    // The **north** side is the one opposite the corner here, so north is what gets squeezed; west keeps its weight,
    // because west is the side that matched the corner. My first version asserted the squeeze landed on west.
    assert!(weights[0] < 1.0, "the north side was squeezed: {weights:?}");
    assert!((weights[3] - 1.0).abs() < 1e-6, "and west is untouched");
    assert!(
        self_weight < 4.0,
        "and this square's weight was adjusted: {self_weight}"
    );
}

#[test]
fn a_square_whose_neighbours_are_all_its_own_kind_is_untouched() {
    let neighbours = [Some("forest"); 8];
    let mut weights = [1.0_f32; 4];
    let mut self_weight = 4.0;
    unify_all_borders(
        (2, 2),
        24,
        neighbours,
        &mut weights,
        Some("forest"),
        &mut self_weight,
    );
    assert!(
        weights.iter().all(|weight| (weight - 1.0).abs() < 1e-6),
        "untouched: {weights:?}"
    );
    assert!((self_weight - 4.0).abs() < 1e-6);
}
