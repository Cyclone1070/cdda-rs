//! What one square can see of another on the same level.
//!
//! Source: `map::sees` (`src/map.cpp:7329-7362`), the same-level branch. The rule is a
//! Bresenham walk that stops **before** checking the square it is looking at, so a wall is
//! still visible as a wall - it just does not let anything behind it be seen.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::vision::sees_on_one_level;

const SIDE: i32 = 12;

/// A room of the given side with the listed squares opaque.
fn room(walls: &[(i32, i32)]) -> impl Fn(i32, i32) -> bool + '_ {
    move |x, y| !walls.contains(&(x, y))
}

fn in_bounds(x: i32, y: i32) -> bool {
    (0..SIDE).contains(&x) && (0..SIDE).contains(&y)
}

#[test]
fn an_open_room_can_be_seen_across() {
    let transparent = room(&[]);
    assert!(sees_on_one_level(
        (1, 1),
        (9, 9),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn a_wall_in_the_way_stops_the_sight() {
    let transparent = room(&[(5, 5)]);
    assert!(!sees_on_one_level(
        (1, 1),
        (9, 9),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn the_square_being_looked_at_is_seen_even_when_it_is_the_wall() {
    // The reference exits the walk before testing the last square, and says so in a comment: "Exit before checking the last square, it's still visible even if
    // opaque" (`map.cpp:7345-7347`). So a wall is seen as a wall; only what is **behind** it is not.
    let transparent = room(&[(9, 9)]);
    assert!(sees_on_one_level(
        (1, 1),
        (9, 9),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn nothing_is_seen_past_the_range() {
    let transparent = room(&[]);
    assert!(!sees_on_one_level(
        (1, 1),
        (9, 9),
        5,
        &transparent,
        in_bounds
    ));
    assert!(
        sees_on_one_level((1, 1), (5, 5), 5, &transparent, in_bounds),
        "and exactly at the range it is still seen"
    );
}

#[test]
fn nothing_off_the_level_is_seen() {
    let transparent = room(&[]);
    assert!(!sees_on_one_level(
        (1, 1),
        (20, 5),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn a_diagonal_through_a_corner_squeezes_between_the_two_walls() {
    // The walk steps through one of the two cells that touch the corner, and which one is the Bresenham tie-break. This pins the answer rather than the intent: the
    // reference's line from (0,0) to (2,2) takes (1,1) first, so a wall pair at (1,0) and (0,1) leaves the diagonal open.
    let transparent = room(&[(1, 0), (0, 1)]);
    assert!(
        sees_on_one_level((0, 0), (2, 2), 60, transparent, in_bounds),
        "the diagonal passes through (1,1), which is open"
    );

    // And a wall on the diagonal itself closes it.
    let blocked = room(&[(1, 1)]);
    assert!(!sees_on_one_level((0, 0), (2, 2), 60, blocked, in_bounds));
}
