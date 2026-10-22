//! Fire as it lives in the store: a field entity that grows, and one that gutters out.
//!
//! Reference: the fire processor (`src/map_field.cpp:945-1360`) runs inside the per-turn field sweep, so what the store needs is a system
//! that walks the fire entities and hands each one its turn. Two of the processor's inputs are the map's rather than the fire's - how many
//! fires touch it, and whether the ground under it can burn - so they arrive as answers the environment layer supplies.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_sim::{FIRE, Field, Position, process_fires};

fn fire_at(world: &mut World, x: i32, y: i32, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(age);
    entry.set_intensity(intensity);
    world
        .spawn((
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
            Position { x, y },
        ))
        .id()
}

#[test]
fn a_fed_fire_with_neighbours_grows() {
    let mut world = World::new();
    let entity = fire_at(&mut world, 5, 5, 1, -10_000);
    process_fires(&mut world, |_| 7, |_| true, |_| 0);
    let field = world.get::<Field>(entity).expect("the fire");
    assert_eq!(
        field.entry.intensity(),
        2,
        "seven neighbours and fuel grow it a level"
    );
    assert!(field.entry.age() > -10_000, "and it paid for the growth");
}

#[test]
fn a_fire_with_nothing_to_burn_ages_toward_death() {
    let mut world = World::new();
    let entity = fire_at(&mut world, 5, 5, 2, 0);
    process_fires(&mut world, |_| 0, |_| true, |_| 0);
    let field = world.get::<Field>(entity).expect("the fire");
    assert_eq!(field.entry.age(), 20, "ten seconds a level");
    assert_eq!(field.entry.intensity(), 2, "and no growth without fuel");
}

#[test]
fn a_fire_that_ate_this_turn_spends_what_it_ate() {
    // What the fire consumed arrives as turns of fuel, and it is credited before the growth test: two hours of fuel both pays for the
    // level it climbs and leaves the fire longer-lived than it was.
    let mut world = World::new();
    let entity = fire_at(&mut world, 5, 5, 1, -10);
    process_fires(&mut world, |_| 0, |_| true, |_| 7_200);
    let field = world.get::<Field>(entity).expect("the fire");
    assert_eq!(
        field.entry.intensity(),
        2,
        "an hour of fuel reaches the cap on fuel alone"
    );
    assert!(
        field.entry.age() < 0,
        "and it is still fuelled: {}",
        field.entry.age()
    );
}

#[test]
fn a_fire_that_is_already_out_is_taken_off_the_store() {
    // `fire_turn` never lowers an intensity - that is the sweep's decay, not the fire's own rules - so a fire leaves the store when its
    // turn starts with nothing left. My first version of this test tried to age a fire to death here and could not: the rule that kills a
    // fire is the half-life, and it lives in the field sweep.
    let mut world = World::new();
    let mut entry = FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(1_000_000);
    entry.set_intensity(0);
    let entity = world
        .spawn((
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
            Position { x: 1, y: 1 },
        ))
        .id();
    process_fires(&mut world, |_| 0, |_| false, |_| 0);
    assert!(world.get::<Field>(entity).is_none(), "an out fire is gone");
}

#[test]
fn a_field_that_is_not_fire_is_left_alone() {
    let mut world = World::new();
    let mut entry = FieldEntry::new(3, 1_000_000, false, false);
    entry.set_age(5);
    entry.set_intensity(2);
    let entity = world
        .spawn((
            Field {
                id: cdda_core_types::field::FieldId(3),
                entry,
                priority: 0,
            },
            Position { x: 2, y: 2 },
        ))
        .id();
    process_fires(&mut world, |_| 9, |_| true, |_| 5_000);
    let field = world.get::<Field>(entity).expect("the field");
    assert_eq!(field.entry.age(), 5, "smoke does not grow or spend fuel");
    assert_eq!(field.entry.intensity(), 2);
}
