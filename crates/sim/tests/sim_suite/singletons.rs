//! A thing that exists once is still a thing, so it is an entity.
//!
//! The architecture is one store: entities and components. A global is a singleton that has not
//! moved yet, and this is the pattern every one of them moves to — a marker component says what the
//! entity is for, the data is a component on it, and code reaches it through a function that finds
//! or spawns it.
//!
//! The first one converted is the falling queue: the squares whose support was destroyed, which the
//! next turn takes whole and clears. It is small, it is written every turn, and it has a rule that
//! makes the conversion worth testing — a second call must find the same entity rather than make a
//! second queue, because two queues would silently drop half the falling squares.

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldId;
use cdda_sim::falling::FieldRules;
use cdda_sim::{
    SupportLost, TheFallingQueue, TheFieldRules, TheMemory, adjust_radiation, falling_queue_entity,
    field_rules, memory, memory_entity, queue_support_loss, set_field_rules, support_lost,
};
use std::collections::BTreeSet;

#[test]
fn the_same_singleton_is_found_every_time_it_is_asked_for() {
    let mut world = World::new();

    let first = falling_queue_entity(&mut world);
    support_lost(&mut world).queue((1, 2, 3));
    let second = falling_queue_entity(&mut world);

    assert_eq!(
        first, second,
        "asking twice must find the same entity, not spawn a second queue"
    );
    assert_eq!(
        world
            .query_filtered::<Entity, With<TheFallingQueue>>()
            .iter(&world)
            .count(),
        1,
        "there must be exactly one falling queue in the world"
    );
}

#[test]
fn queueing_a_square_writes_the_component_and_no_global() {
    let mut world = World::new();

    queue_support_loss(&mut world, (4, 5, 6));
    queue_support_loss(&mut world, (4, 5, 6));

    let entity = falling_queue_entity(&mut world);
    let queue = world
        .get::<SupportLost>(entity)
        .expect("the queue is a component on the entity that was just found");
    assert_eq!(
        queue.squares.len(),
        1,
        "a square queued twice is one square, which is what a set means"
    );
    assert!(queue.squares.contains(&(4, 5, 6)));
    assert_eq!(
        world
            .query_filtered::<Entity, With<TheFallingQueue>>()
            .iter(&world)
            .count(),
        1,
        "one entity carries the queue, so there cannot also be a global holding a second copy"
    );
}

#[test]
fn a_square_s_dose_is_carried_by_the_square() {
    // Radiation is a property of a place, so it is a component on the thing that stands there -
    // not an array of squares held somewhere else.
    let mut world = World::new();

    adjust_radiation(&mut world, (1, 2, 0), 5);

    assert_eq!(
        cdda_sim::radiation::dose_at(&world, (1, 2, 0)),
        5,
        "the square carries what was put on it"
    );
    assert_eq!(
        cdda_sim::radiation::squares_with_radiation(&world),
        1,
        "one square holds it"
    );

    adjust_radiation(&mut world, (1, 2, 0), 3);

    assert_eq!(
        cdda_sim::radiation::squares_with_radiation(&world),
        1,
        "adding to the same square writes the square, it does not make a second one"
    );
    assert_eq!(cdda_sim::radiation::dose_at(&world, (1, 2, 0)), 8, "5 + 3");

    adjust_radiation(&mut world, (1, 2, 0), -8);

    assert_eq!(
        cdda_sim::radiation::squares_with_radiation(&world),
        0,
        "a square that returns to zero carries nothing at all"
    );
}

#[test]
fn the_characters_memory_is_an_entity() {
    let mut world = World::new();

    let first = memory_entity(&mut world);
    memory(&mut world);
    let second = memory_entity(&mut world);

    assert_eq!(
        first, second,
        "asking for the memory twice finds one memory"
    );
    assert_eq!(
        world
            .query_filtered::<Entity, With<TheMemory>>()
            .iter(&world)
            .count(),
        1,
        "one entity carries the character's memory"
    );
}

#[test]
fn the_falling_rules_are_an_entity() {
    let mut world = World::new();
    let falling = FieldId(11);

    set_field_rules(
        &mut world,
        FieldRules {
            falls: BTreeSet::from([falling]),
            gas: BTreeSet::new(),
        },
    );

    assert!(
        field_rules(&mut world).falls.contains(&falling),
        "the rules that were set are the rules that are read"
    );
    assert_eq!(
        world
            .query_filtered::<Entity, With<TheFieldRules>>()
            .iter(&world)
            .count(),
        1,
        "one entity carries the falling rules"
    );
}
