//! Which way streets run, and the line one makes.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{Direction, straight_path};

#[test]
fn the_four_directions_are_the_four_the_reference_names() {
    assert_eq!(Direction::ALL.len(), 4);
    assert_eq!(Direction::North.index(), 0);
    assert_eq!(Direction::East.index(), 1);
    assert_eq!(Direction::South.index(), 2);
    assert_eq!(Direction::West.index(), 3);
}

#[test]
fn a_step_north_decreases_y_and_a_step_east_increases_x() {
    assert_eq!(Direction::North.displace(1), (0, -1));
    assert_eq!(Direction::East.displace(1), (1, 0));
    assert_eq!(Direction::South.displace(1), (0, 1));
    assert_eq!(Direction::West.displace(1), (-1, 0));
    assert_eq!(Direction::East.displace(3), (3, 0));
}

#[test]
fn turning_right_four_times_comes_back_to_where_it_started() {
    for direction in Direction::ALL {
        let mut turned = direction;
        for _ in 0..4 {
            turned = turned.turn_right();
        }
        assert_eq!(turned, direction);
    }
}

#[test]
fn turning_left_is_the_other_way_round() {
    for direction in Direction::ALL {
        assert_eq!(direction.turn_left().turn_right(), direction);
        assert_ne!(direction.turn_left(), direction.turn_right());
    }
    assert_eq!(Direction::North.turn_right(), Direction::East);
    assert_eq!(Direction::North.turn_left(), Direction::West);
}

#[test]
fn a_straight_path_is_as_long_as_it_was_asked_for_and_starts_where_it_was_told() {
    let path = straight_path((10, 10), Direction::East, 5);
    assert_eq!(path.len(), 5, "five tiles, counting the one it starts on");
    assert_eq!(path.first().copied(), Some((10, 10)));
    assert_eq!(path.last().copied(), Some((14, 10)));
}

#[test]
fn every_step_of_a_path_moves_one_tile_the_same_way() {
    for direction in Direction::ALL {
        let path = straight_path((50, 50), direction, 6);
        let (dx, dy) = direction.displace(1);
        for (index, node) in path.iter().enumerate() {
            let step = i32::try_from(index).unwrap_or(0);
            assert_eq!(
                *node,
                (50 + dx * step, 50 + dy * step),
                "{direction:?} step {index}"
            );
        }
    }
}

#[test]
fn a_path_of_no_length_is_no_tiles() {
    assert!(straight_path((3, 3), Direction::South, 0).is_empty());
    assert_eq!(straight_path((3, 3), Direction::South, 1), vec![(3, 3)]);
}
