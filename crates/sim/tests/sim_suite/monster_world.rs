//! More than one creature: turns in order, and nobody walks through anybody.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::{MonsterKindId, Position, monster_turns, occupied, spawn_monster};

const FLOOR: u32 = 1;

fn level() -> Level {
    let mut level = Level::new();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    level
}

fn here(world: &mut World, entity: Entity) -> (i32, i32) {
    let position = world.get::<Position>(entity).expect("a position");
    (position.x, position.y)
}

fn normal_speed(_: MonsterKindId) -> i32 {
    100
}

fn floor_only(terrain: u32) -> bool {
    terrain == FLOOR
}

#[test]
fn a_creature_in_the_world_takes_its_turn() {
    let mut world = World::new();
    let monster = spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    monster_turns(&mut world, (9, 0), normal_speed, floor_only, &level());
    assert_eq!(
        here(&mut world, monster),
        (1, 0),
        "one step for one turn's moves"
    );
}

#[test]
fn a_square_with_a_creature_on_it_is_occupied_and_a_dead_one_does_not_count() {
    let mut world = World::new();
    let monster = spawn_monster(&mut world, (3, 3), MonsterKindId(1), 10);
    assert!(occupied(&mut world, (3, 3)));
    assert!(!occupied(&mut world, (3, 4)));
    cdda_sim::creature::set_health_left(&mut world, monster, 0);
    assert!(
        !occupied(&mut world, (3, 3)),
        "a corpse does not block the square"
    );
}

#[test]
fn nobody_walks_into_another_creature() {
    let mut world = World::new();
    // The second creature stands directly in the first one's path.
    let walker = spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    let blocker = spawn_monster(&mut world, (1, 0), MonsterKindId(1), 10);
    monster_turns(&mut world, (9, 0), normal_speed, floor_only, &level());
    assert_eq!(
        here(&mut world, walker),
        (0, 0),
        "it stopped rather than walking through"
    );
    // The blocker then takes its own turn and walks on, which is the point: being blocked is not being stuck.
    assert_eq!(here(&mut world, blocker), (2, 0));
}

#[test]
fn creatures_act_in_the_worlds_own_order_so_a_replay_is_a_replay() {
    // Two creatures behind each other: the first moves, the second follows into the square it left.
    let mut world = World::new();
    let first = spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    let second = spawn_monster(&mut world, (0, 1), MonsterKindId(1), 10);
    monster_turns(&mut world, (9, 0), normal_speed, floor_only, &level());
    assert_eq!(here(&mut world, first), (1, 0));
    // The second creature's step would be (1,0) - straight into the first - so it does not move at all this turn.
    assert_eq!(
        here(&mut world, second),
        (0, 1),
        "it does not stack onto the one in front"
    );
}

#[test]
fn a_creature_spends_its_moves_and_nothing_more() {
    let mut world = World::new();
    let monster = spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    monster_turns(&mut world, (9, 0), normal_speed, floor_only, &level());
    let moves = cdda_sim::creature::moves_left(&world, monster);
    assert!(
        moves <= 0,
        "one action at speed 100 costs all 100 moves: {moves} left"
    );
}
