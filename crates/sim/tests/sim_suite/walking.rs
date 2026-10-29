//! A creature walking a route it asked the world for.
//!
//! This is the first behaviour that ties the four pieces together: the creature's place and its action
//! points (`crate::creature`), the route (`crate::routing`), the cost of a step
//! (`map::combined_movecost`) and the store the route was read from. Nothing here is a new rule - the
//! rule is that a creature spends its points and stands where it walked, and the four pieces are
//! already gated against the reference on their own.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::creature::{creature_at, give_moves, moves_left, spawn_creature};
use cdda_sim::routing::{Walk, walk_towards};
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::Prototypes;

const FLOOR: u32 = 1;
const WALL: u32 = 2;

/// A one-row corridor: `length` squares of floor, with a wall where the list says so.
fn corridor(length: i32, walls: &[i32]) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..length {
        let code = if walls.contains(&x) { WALL } else { FLOOR };
        level.set_terrain(x, 0, code, code);
    }
    cdda_sim::load_level(&mut world, &level);

    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert_move_cost(FLOOR, 2);
    terrain.insert(WALL, false);
    terrain.insert_move_cost(WALL, 0);
    cdda_sim::set_prototypes(
        &mut world,
        Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );
    world
}

#[test]
fn a_creature_walks_a_corridor_one_square_per_step() {
    let mut world = corridor(3, &[]);
    let you = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, you, 200);

    let first = walk_towards(&mut world, you, (2, 0, 0), (3, 1));
    assert_eq!(first, Walk::Stepped, "it took the first step");
    assert_eq!(creature_at(&world, (1, 0, 0)), vec![you], "and stands one square on");
    assert_eq!(moves_left(&world, you), 100, "having paid for one floor step");

    let second = walk_towards(&mut world, you, (2, 0, 0), (3, 1));
    assert_eq!(second, Walk::Arrived, "the second step reaches the target");
    assert_eq!(creature_at(&world, (2, 0, 0)), vec![you], "where it meant to be");
    assert_eq!(moves_left(&world, you), 0, "and it spent exactly what it had");
}

#[test]
fn a_creature_will_not_step_where_it_cannot_afford_or_cannot_go() {
    let mut world = corridor(3, &[]);
    let you = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, you, 50);

    let too_poor = walk_towards(&mut world, you, (2, 0, 0), (3, 1));
    assert_eq!(too_poor, Walk::TooSlow, "fifty points cannot pay for a hundred-move step");
    assert_eq!(creature_at(&world, (0, 0, 0)), vec![you], "so it stays put");

    let mut walled = corridor(3, &[1]);
    let stuck = spawn_creature(&mut walled, "the survivor", (0, 0, 0));
    give_moves(&mut walled, stuck, 500);
    let no_way = walk_towards(&mut walled, stuck, (2, 0, 0), (3, 1));
    assert_eq!(no_way, Walk::NoRoute, "a wall across the corridor is not a detour");
}

/// The creature turn is a stage in the schedule, in a declared place, and it acts inside `run_turn`.
///
/// Reference: `game::monmove` runs during the turn, after the fields have taken theirs and before the
/// light is recomputed - a monster that walks into a fire is burned by the fire that was already there
/// this turn, not one that lights afterwards.
#[test]
fn creature_turns_run_in_the_declared_stage_of_a_real_turn() {
    use cdda_sim::turn::TurnStage;

    let order = TurnStage::order();
    let creatures = order
        .iter()
        .position(|stage| *stage == TurnStage::Creatures)
        .expect("the turn has a creature stage");
    let fields = order
        .iter()
        .position(|stage| *stage == TurnStage::Fields)
        .expect("the turn has a field stage");
    let light = order
        .iter()
        .position(|stage| *stage == TurnStage::Light)
        .expect("the turn has a light stage");
    assert!(
        fields < creatures && creatures < light,
        "creatures act after the fields and before the light: {order:?}"
    );

    let mut world = corridor(3, &[]);
    let you = spawn_creature(&mut world, "a shambler", (0, 0, 0));
    give_moves(&mut world, you, 0);
    cdda_sim::creature::set_speed(&mut world, you, 100, 0);
    cdda_sim::creature::set_goal(&mut world, you, (2, 0, 0));

    cdda_sim::turn::run_turn(&mut world);
    assert_eq!(
        creature_at(&world, (1, 0, 0)),
        vec![you],
        "one turn of banking its own speed and walking one step"
    );

    cdda_sim::turn::run_turn(&mut world);
    assert_eq!(creature_at(&world, (2, 0, 0)), vec![you], "and the next turn arrives");
}
