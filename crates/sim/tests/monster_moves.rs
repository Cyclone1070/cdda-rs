//! Monsters spending and regaining moves.

#![allow(clippy::panic)]

use cdda_sim::{can_act, regen_moves, step_cost};

#[test]
fn an_ordinary_step_costs_a_turn() {
    // Default terrain reports a move cost of 2 (`map::move_cost`).
    assert_eq!(step_cost(2), 100, "50 * 2");
}

#[test]
fn rough_ground_costs_more() {
    assert_eq!(step_cost(4), 200, "50 * 4");
}

#[test]
fn a_creature_gains_its_speed_each_turn() {
    assert_eq!(regen_moves(0, 100), 100, "normal speed, one turn's worth");
    assert_eq!(regen_moves(50, 70), 120, "what it had, plus its speed");
}

#[test]
fn zero_moves_means_waiting() {
    assert!(!can_act(0), "no moves, no action");
    assert!(can_act(1), "one move is an action");
}

#[test]
fn a_fast_creature_can_act_twice_on_one_step_of_moves() {
    let mut moves = regen_moves(0, 200);
    moves -= step_cost(2);
    assert_eq!(moves, 100, "one step spent");
    moves -= step_cost(2);
    assert_eq!(moves, 0, "and a second");
    assert!(!can_act(moves), "but not a third");
}
