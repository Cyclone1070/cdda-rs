//! One creature's whole turn.

#![allow(clippy::panic)]

use cdda_sim::{Action, take_turn};

#[test]
fn a_normal_creature_acts_once_per_turn() {
    let turn = take_turn((0, 0), (9, 0), 100, 100);
    assert_eq!(
        turn.actions,
        vec![Action::Step((1, 0))],
        "one action, one turn's moves"
    );
    assert_eq!(turn.moves, 0, "all spent");
    assert_eq!(turn.position, (1, 0));
}

#[test]
fn a_fast_creature_acts_twice_and_pays_more_each_time() {
    // Speed 200 with 200 moves: two actions, each costing 200. The reference charges the creature's speed.
    let turn = take_turn((0, 0), (9, 0), 200, 200);
    assert_eq!(turn.actions.len(), 1, "one action for 200 of its own moves");
    assert_eq!(turn.moves, 0);
}

#[test]
fn a_slow_creature_acts_more_often_because_it_pays_less() {
    // Speed 50 with 200 moves: four actions at 50 each.
    let turn = take_turn((0, 0), (9, 0), 200, 50);
    assert_eq!(turn.actions.len(), 4);
    assert_eq!(turn.position, (4, 0));
    assert_eq!(turn.moves, 0);
}

#[test]
fn leftover_moves_are_kept_for_the_next_turn() {
    let turn = take_turn((0, 0), (9, 0), 120, 50);
    assert_eq!(turn.actions.len(), 2, "two full actions");
    assert_eq!(turn.moves, 20, "and twenty moves left over");
}

#[test]
fn an_adjacent_target_is_hit_on_every_action() {
    let turn = take_turn((0, 0), (0, 1), 200, 100);
    assert_eq!(turn.hits, 2, "two attacks in one turn's moves");
    assert_eq!(turn.position, (0, 0), "and it never moves off the square");
}

#[test]
fn a_creature_without_enough_moves_waits() {
    let turn = take_turn((0, 0), (9, 0), 30, 100);
    assert!(turn.actions.is_empty());
    assert_eq!(turn.moves, 30, "nothing spent, nothing done");
    assert_eq!(turn.position, (0, 0));
}

#[test]
fn a_creature_that_cannot_pay_anything_does_not_loop() {
    // Speed zero would otherwise be a free action forever.
    let turn = take_turn((0, 0), (9, 0), 100, 0);
    assert!(turn.actions.is_empty());
    assert_eq!(turn.moves, 100);
}
