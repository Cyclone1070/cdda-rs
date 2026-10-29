//! A route across real squares, with the store answering what each one costs.
//!
//! The search itself is [`cdda_sim::pathfinding::greedy_path`], gated against the reference's own
//! expected paths. What is new here is the *scorer*: the cost of entering a square comes from the world,
//! through `map::combined_movecost`'s rule, and a square described as impassable is refused rather than
//! charged for.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::combined_move_cost;
use cdda_sim::pathfinding::{Direction, manhattan_distance};
use cdda_sim::routing::route;
use cdda_sim::{Position, singletons};

/// Builds a world whose squares are all floor except the ones named, which are walls.
fn a_world(width: i32, height: i32, walls: &[(i32, i32)]) -> World {
    let mut world = World::new();
    let mut prototypes = cdda_sim::Prototypes::default();
    prototypes.terrain.insert_move_cost(1, 2); // floor
    prototypes.terrain.insert_move_cost(2, 0); // wall
    singletons::set_prototypes(&mut world, prototypes);
    for y in 0..height {
        for x in 0..width {
            let code = if walls.contains(&(x, y)) { 2 } else { 1 };
            let entity = world
                .spawn((
                    Position { x, y, z: 0 },
                    cdda_sim::square::Terrain { code },
                ))
                .id();
            singletons::index_mut(&mut world).insert(entity, (x, y, 0));
        }
    }
    world
}

#[test]
fn a_route_goes_round_a_wall_and_costs_both_squares_a_step() {
    // The same shape the reference's own u-bend test uses: a wall at x = 1 on the first two rows.
    let world = a_world(3, 3, &[(1, 0), (1, 1)]);

    let path = route(&world, (0, 0), (2, 0), (3, 3));

    let points: Vec<(i32, i32)> = path.iter().map(|node| (node.x, node.y)).collect();
    assert_eq!(
        points,
        vec![(2, 0), (2, 1), (2, 2), (1, 2), (0, 2), (0, 1), (0, 0)],
        "the wall sends the route down and around, as it does in the reference's own case"
    );
    assert_eq!(path.len(), 7, "seven squares");
}

#[test]
fn an_inpassable_square_is_refused_rather_than_charged_for() {
    // A wall across the whole map: no route exists, and the search must say so rather than walk through.
    let world = a_world(3, 1, &[(1, 0)]);

    let path = route(&world, (0, 0), (2, 0), (3, 1));

    assert!(path.is_empty(), "a wall with no way round ends the search empty");
}

#[test]
fn the_scorer_charges_what_combined_move_cost_says() {
    // Two floors: 2 and 2, straight, is (2 + 2) * 50 / 2 = 100 moves; diagonally 142.
    assert_eq!(combined_move_cost(2, 2, 0, 1), 100);
    assert_eq!(combined_move_cost(2, 2, 0, 2), 142);
    // And the score the search sees is what the store answers, not a constant.
    let world = a_world(2, 1, &[]);
    let scorer = cdda_sim::routing::scorer(&world, (0, 0));
    let straight = scorer((1, 0), Some((0, 0)));
    assert_eq!(straight.node_cost, 100, "the cost of entering the floor");
    assert_eq!(
        straight.estimated_dest_cost,
        manhattan_distance((1, 0), (0, 0)) * 100,
        "and the estimate is one step's worth per square away"
    );
    let _ = Direction::North;
}
