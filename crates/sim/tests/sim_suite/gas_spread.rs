//! Gas spreads to a neighbour, thinning as it goes.
//!
//! Transcribed from `map::gas_spread_to` (`src/map_field.cpp:250-273`), which is the rule the
//! smoke processor uses once it has chosen a direction. The direction choice needs wind and a
//! dice roll and is not ported here - `cases` takes the square the caller decided on, so the
//! arithmetic can be gated on its own.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position, gas_spread_to};

fn world() -> World {
    World::new()
}

fn spawn(world: &mut World, x: i32, y: i32, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Position { x, y, z: 0 },
            Field {
                id: FieldId(1),
                entry,
                priority: 0,
            },
        ))
        .id()
}

#[test]
fn spreading_into_a_square_that_already_has_the_gas_strengthens_it_and_splits_the_age() {
    // Source: intensity 3, age 6. Neighbour: intensity 1, age 4.
    // age_fraction = 6 / 3 = 2, so the neighbour becomes 2 with age 6, the source 2 with age 4.
    let mut world = world();
    let source = spawn(&mut world, 5, 5, 3, 6);
    let neighbour = spawn(&mut world, 6, 5, 1, 4);

    gas_spread_to(&mut world, source, (6, 5));

    let a = world.get::<Field>(source).expect("the source survives");
    let b = world
        .get::<Field>(neighbour)
        .expect("the neighbour survives");
    assert_eq!(a.entry.intensity(), 2);
    assert_eq!(a.entry.age(), 4);
    assert_eq!(b.entry.intensity(), 2);
    assert_eq!(b.entry.age(), 6);
}

#[test]
fn spreading_into_clear_air_creates_a_thin_field_carrying_the_share_of_the_age() {
    let mut world = world();
    let source = spawn(&mut world, 5, 5, 3, 6);

    gas_spread_to(&mut world, source, (6, 5));

    let a = world.get::<Field>(source).expect("the source survives");
    assert_eq!(a.entry.intensity(), 2, "it thinned as it spread");
    assert_eq!(a.entry.age(), 4, "and gave up its share of the age");
    let mut query = world.query::<(&Position, &Field)>();
    let created: Vec<(i32, i32, i32, i64)> = query
        .iter(&world)
        .map(|(p, f)| (p.x, p.y, f.entry.intensity(), f.entry.age()))
        .collect();
    assert!(
        created.contains(&(6, 5, 1, 2)),
        "a new thin field appeared: {created:?}"
    );
}

#[test]
fn a_single_unit_of_gas_does_not_spread() {
    // `spread_gas` bails out below intensity 2 (`src/map_field.cpp:294`), so the thinnest gas
    // stays where it is instead of evaporating into a trail of ones.
    let mut world = world();
    let source = spawn(&mut world, 5, 5, 1, 6);

    gas_spread_to(&mut world, source, (6, 5));

    let a = world.get::<Field>(source).expect("the source survives");
    assert_eq!(a.entry.intensity(), 1, "unchanged");
    let mut query = world.query::<&Field>();
    assert_eq!(query.iter(&world).count(), 1, "and nothing was created");
}

#[test]
fn the_rolls_are_not_touched_by_the_arithmetic() {
    // Spreading's arithmetic draws no dice: the dice belong to the direction choice, so a test
    // that spreads a field must leave a stream untouched or every later roll shifts.
    let mut world = world();
    let source = spawn(&mut world, 5, 5, 3, 6);
    let mut rolls = Rolls::new(11);
    gas_spread_to(&mut world, source, (6, 5));
    let _ = rolls;
    assert_eq!(
        rolls.engine().next_u32(),
        Rolls::new(11).engine().next_u32()
    );
}
