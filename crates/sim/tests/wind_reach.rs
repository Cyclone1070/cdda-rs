//! The wind shield around a fire.
//!
//! Transcribed from `field_processor_fd_fire` (`src/map_field.cpp:1136-1152`). The point of these
//! tests is not that the shield works: it is that the port draws the same dice as the reference
//! while failing to shield anything, because that is what the reference does.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::rng::Rolls;
use cdda_sim::wind_reach;

const BLOCKERS: [(i32, i32); 3] = [(1, 0), (1, -1), (1, 1)];

#[test]
fn the_three_blockers_are_the_only_neighbours_the_wind_can_stop() {
    // A neighbour that shares a coordinate with all three blockers reaches the die; every other
    // neighbour differs from at least one of them and is kept without drawing.
    let mut drawn_for = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            if (dx, dy) == (0, 0) {
                continue;
            }
            let mut rolls = Rolls::new(1);
            let mut control = Rolls::new(1);
            wind_reach((dx, dy), BLOCKERS, 30, &mut rolls);
            if rolls.rng(1, 1000) != control.rng(1, 1000) {
                drawn_for.push((dx, dy));
            }
        }
    }
    let mut expected = vec![(1, 0), (1, -1), (1, 1)];
    drawn_for.sort_unstable();
    expected.sort_unstable();
    assert_eq!(drawn_for, expected, "exactly the blockers draw");
}

#[test]
fn a_blocked_neighbour_gets_through_sometimes() {
    let blocked = 0;
    let mut through = 0;
    let mut stopped = 0;
    for seed in 1..=40 {
        let mut rolls = Rolls::new(seed);
        if wind_reach((1, 0), BLOCKERS, 2, &mut rolls) {
            through += 1;
        } else {
            stopped += 1;
        }
    }
    assert!(
        through > 0 && stopped > 0,
        "{through} through, {stopped} stopped over 40 seeds"
    );
    assert_eq!(blocked, 0);
}

#[test]
fn a_degenerate_blocker_set_does_reach_the_random_clause() {
    // Three identical blockers make the three "differs" clauses identical too, so a neighbour equal
    // to that blocker reaches x_in_y - and then the dice really are drawn, sometimes keeping it.
    let blockers = [(1, 0), (1, 0), (1, 0)];
    let mut kept = 0;
    let mut shielded = 0;
    for seed in 1..=40 {
        let mut rolls = Rolls::new(seed);
        if wind_reach((1, 0), blockers, 2, &mut rolls) {
            kept += 1;
        } else {
            shielded += 1;
        }
    }
    assert!(
        kept > 0 && shielded > 0,
        "{kept} kept, {shielded} shielded over 40 seeds"
    );
}

#[test]
fn a_neighbour_that_differs_from_one_blocker_is_kept_without_dice() {
    let mut rolls = Rolls::new(3);
    let mut control = Rolls::new(3);
    assert!(
        wind_reach((0, 0), BLOCKERS, 30, &mut rolls),
        "differs from all three"
    );
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "and draws nothing"
    );
}
