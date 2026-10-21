//! When the world refuses a step.

#![allow(clippy::panic)]

use cdda_sim::{Action, take_turn_blocked};

fn open(_: (i32, i32)) -> bool {
    true
}

#[test]
fn a_blocked_step_costs_nothing_and_ends_the_turn() {
    // The reference returns false before charging: blocked movement is free.
    let turn = take_turn_blocked((0, 0), (9, 0), 100, 100, |_| false);
    assert!(turn.actions.is_empty(), "no step was taken");
    assert_eq!(turn.moves, 100, "and nothing was paid");
    assert_eq!(turn.position, (0, 0));
}

#[test]
fn a_monster_walks_until_it_meets_the_wall() {
    // Open for the first two squares, a wall at x == 2.
    let turn = take_turn_blocked((0, 0), (9, 0), 400, 100, |(x, _)| x < 2);
    assert_eq!(turn.actions, vec![Action::Step((1, 0))]);
    assert_eq!(turn.position, (1, 0), "stopped in front of the wall");
    assert_eq!(
        turn.moves, 300,
        "two actions paid for, the blocked one free"
    );
}

#[test]
fn an_open_world_lets_a_monster_walk_its_whole_turn() {
    let turn = take_turn_blocked((0, 0), (9, 0), 300, 100, open);
    assert_eq!(turn.actions.len(), 3, "three steps for three hundred moves");
    assert_eq!(turn.position, (3, 0));
    assert_eq!(turn.moves, 0);
}

#[test]
fn a_blocked_world_does_not_stop_a_monster_from_hitting_what_is_next_to_it() {
    // The wall is everywhere, but the target is adjacent: attacking never asks for passage.
    let turn = take_turn_blocked((0, 0), (0, 1), 300, 100, |_| false);
    assert_eq!(turn.hits, 3, "three attacks");
    assert_eq!(turn.moves, 0, "each paid for");
    assert_eq!(turn.position, (0, 0), "and it never moved");
}
