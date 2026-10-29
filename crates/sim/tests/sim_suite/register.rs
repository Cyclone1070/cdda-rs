//! The store: things are entities, positions live on them, and the grid is rebuilt from them.

#![allow(clippy::panic, clippy::expect_used, dead_code)]

use bevy_ecs::prelude::*;
use cdda_sim::{Position, PositionIndex};

#[derive(Debug, Component)]
struct Smoke {
    strength: i32,
}

#[test]
fn a_component_carries_its_own_numbers() {
    let mut world = World::new();
    let entity = world
        .spawn((Position { x: 0, y: 0, z: 0 }, Smoke { strength: 2 }))
        .id();
    let _ = entity;
    let mut query = world.query::<&Smoke>();
    let strengths: Vec<i32> = query.iter(&world).map(|smoke| smoke.strength).collect();
    assert_eq!(strengths, vec![2]);
}

#[derive(Debug, Component)]
struct Person {
    name: &'static str,
}

#[test]
fn things_are_entities_with_a_position_and_the_grid_finds_them() {
    let mut world = World::new();
    let smoke = world
        .spawn((Position { x: 3, y: 4, z: 0 }, Smoke { strength: 2 }))
        .id();
    let person = world
        .spawn((Position { x: 3, y: 4, z: 0 }, Person { name: "Ada" }))
        .id();

    PositionIndex::rebuild(&mut world);
    let index = cdda_sim::index(&world).expect("the index");
    let here = index.at(3, 4);
    assert_eq!(here.len(), 2, "a square holds both smoke and a person");
    assert!(here.contains(&smoke));
    assert!(here.contains(&person));
    assert!(index.at(0, 0).is_empty(), "an empty square is a gap");
}

#[test]
fn a_new_kind_of_thing_needs_no_new_container() {
    // The point of one store: adding a kind of thing is adding a component. The grid, the
    // queries and the save keep working without being told about it.
    #[derive(Debug, Component)]
    struct Blood {
        age: i32,
    }
    let mut world = World::new();
    let blood = world
        .spawn((Position { x: 1, y: 1, z: 0 }, Blood { age: 0 }))
        .id();
    PositionIndex::rebuild(&mut world);
    assert!(cdda_sim::index(&world).expect("the index").at(1, 1).contains(&blood));
}

#[test]
fn moving_a_thing_moves_it_in_the_grid_once_the_grid_is_rebuilt() {
    let mut world = World::new();
    let person = world
        .spawn((Position { x: 2, y: 2, z: 0 }, Person { name: "Ada" }))
        .id();
    PositionIndex::rebuild(&mut world);
    assert!(cdda_sim::index(&world).expect("the index").at(2, 2).contains(&person));

    if let Some(mut position) = world.get_mut::<Position>(person) {
        position.x = 5;
    }
    PositionIndex::rebuild(&mut world);
    let index = cdda_sim::index(&world).expect("the index");
    assert!(
        index.at(2, 2).is_empty(),
        "the old square no longer holds it"
    );
    assert!(index.at(5, 2).contains(&person), "the new square does");
}

#[test]
fn despawning_a_thing_removes_it_from_the_grid() {
    let mut world = World::new();
    let person = world
        .spawn((Position { x: 7, y: 7, z: 0 }, Person { name: "Ada" }))
        .id();
    PositionIndex::rebuild(&mut world);
    assert_eq!(cdda_sim::index(&world).expect("the index").occupied_squares(), 1);

    world.despawn(person);
    PositionIndex::rebuild(&mut world);
    assert_eq!(cdda_sim::index(&world).expect("the index").occupied_squares(), 0);
}
