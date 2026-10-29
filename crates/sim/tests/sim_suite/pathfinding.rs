//! Two paths the reference's own tests expect, transcribed.
//!
//! Source: `oracle/src/tests/simple_pathfinding_test.cpp`. `greedy_simple_line_path` expects four nodes
//! from (3,0) back to (0,0); `greedy_u_bend` expects seven that walk around a wall and come back. They
//! are cases the reference asserts about itself, so they are the red gate here rather than an invented
//! grid.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_sim::pathfinding::{Direction, Score, greedy_path, manhattan_distance};

#[test]
fn a_straight_line_is_four_nodes_from_the_destination_back() {
    let finish = (3, 0);
    let max = (10, 10);
    let scorer = |position: (i32, i32), _previous: Option<(i32, i32)>| {
        Score::new(0, manhattan_distance(position, finish))
    };

    let path = greedy_path((0, 0), finish, max, &scorer);

    let points: Vec<(i32, i32)> = path.iter().map(|node| (node.x, node.y)).collect();
    assert_eq!(points, vec![(3, 0), (2, 0), (1, 0), (0, 0)], "four nodes");
    let directions: Vec<Option<Direction>> = path.iter().map(|node| node.direction).collect();
    assert_eq!(
        directions,
        vec![
            Some(Direction::West),
            Some(Direction::West),
            Some(Direction::West),
            None,
        ],
        "each node points at the next one towards the start, and the start has no way back"
    );
}

#[test]
fn a_u_bend_walks_round_the_wall_and_comes_back() {
    // SxF    3x0
    // .x.    3x1
    // ...    3x2
    let start = (0, 0);
    let finish = (2, 0);
    let max = (3, 3);
    let scorer = |position: (i32, i32), _previous: Option<(i32, i32)>| {
        if position.0 == 1 && position.1 != 2 {
            return Score::REJECTED;
        }
        Score::new(0, manhattan_distance(position, finish))
    };

    let path = greedy_path(start, finish, max, &scorer);

    let points: Vec<(i32, i32)> = path.iter().map(|node| (node.x, node.y)).collect();
    assert_eq!(
        points,
        vec![(2, 0), (2, 1), (2, 2), (1, 2), (0, 2), (0, 1), (0, 0)],
        "the wall at x=1 sends the path down and around"
    );
    assert_eq!(path.len(), 7, "seven nodes");
    let directions: Vec<String> = path
        .iter()
        .map(|node| format!("{:?}", node.direction))
        .collect();
    assert_eq!(
        directions,
        vec![
            "Some(South)",
            "Some(South)",
            "Some(West)",
            "Some(West)",
            "Some(North)",
            "Some(North)",
            "None",
        ],
        "every turn of the U-bend, as the reference's own test lists it"
    );
}
