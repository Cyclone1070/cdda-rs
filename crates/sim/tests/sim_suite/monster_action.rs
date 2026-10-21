//! What a monster does with the moves it has.

#![allow(clippy::panic)]

use cdda_sim::{Action, choose_action, is_adjacent, step_toward};

#[test]
fn the_four_sides_and_the_four_diagonals_are_adjacent() {
    for offset in [-1, 0, 1] {
        for other in [-1, 0, 1] {
            if offset == 0 && other == 0 {
                continue;
            }
            assert!(
                is_adjacent((5, 5), (5 + offset, 5 + other)),
                "{offset},{other}"
            );
        }
    }
}

#[test]
fn two_squares_away_is_not_adjacent() {
    assert!(!is_adjacent((5, 5), (7, 5)));
    assert!(
        !is_adjacent((5, 5), (5, 5)),
        "its own square is not next to it"
    );
}

#[test]
fn a_step_closes_one_axis_at_a_time_when_already_aligned() {
    assert_eq!(
        step_toward((0, 0), (3, 0)),
        (1, 0),
        "straight along one axis"
    );
    assert_eq!(step_toward((0, 0), (3, 3)), (1, 1), "diagonally, both axes");
    assert_eq!(
        step_toward((2, 2), (0, 0)),
        (1, 1),
        "and toward a lower coordinate"
    );
}

#[test]
fn something_next_to_you_gets_hit() {
    assert_eq!(choose_action((5, 5), (6, 6)), Action::Attack);
    assert_eq!(choose_action((5, 5), (5, 4)), Action::Attack);
}

#[test]
fn something_further_away_is_walked_toward() {
    assert_eq!(choose_action((5, 5), (9, 5)), Action::Step((6, 5)));
    assert_eq!(choose_action((5, 5), (0, 9)), Action::Step((4, 6)));
}

#[test]
fn standing_on_the_target_is_a_wait() {
    assert_eq!(choose_action((5, 5), (5, 5)), Action::Wait);
}
