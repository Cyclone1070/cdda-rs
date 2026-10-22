//! A field that is born during a sweep is not decayed by that sweep.
//!
//! Reference: `map::process_fields` (`src/map_field.cpp:164-200`) collects the squares it will visit **before** it starts, so a field created while it runs -
//! a fire spreading into a neighbour, or the hot air a fire leaves - is not visited until the next turn. The reference's own session shows it: the fire that
//! catches at turn five is still at the two minutes it was born with, and a port that decays everything in the world after the fires run gives it one
//! minute less.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use bevy_ecs::prelude::{Entity, World};
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{FIRE, Field, Position};

fn fire(age: i64) -> Field {
    let mut entry = FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(age);
    entry.set_intensity(1);
    Field {
        id: FIRE,
        entry,
        priority: 0,
    }
}

fn ages(world: &mut World) -> Vec<(i32, i32, i64)> {
    let mut query = world.query::<(&Field, &Position)>();
    let mut held: Vec<(i32, i32, i64)> = query
        .iter(world)
        .map(|(field, position)| (position.x, position.y, field.entry.age()))
        .collect();
    held.sort_unstable();
    held
}

#[test]
fn a_fire_born_during_the_sweep_keeps_the_age_it_was_born_with() {
    let mut world = World::new();
    world.spawn((fire(5), Position { x: 0, y: 0, z: 0 }));
    // The sweep looks at the world as it finds it, and this is that list.
    let present: Vec<(Entity, i32, i32)> = {
        let mut query = world.query::<(Entity, &Field, &Position)>();
        query
            .iter(&world)
            .map(|(entity, _, position)| (entity, position.x, position.y))
            .collect()
    };
    // Then a fire catches next door, during the sweep.
    world.spawn((fire(120), Position { x: 1, y: 0, z: 0 }));

    let mut rolls = Rolls::new(11);
    cdda_sim::decay_the_fields_the_sweep_found(
        &mut world,
        &present,
        0,
        &mut rolls,
        |_field| None,
        |_, _, _| {},
    );

    assert_eq!(
        ages(&mut world),
        vec![(0, 0, 6), (1, 0, 120)],
        "the fire that was there ages; the one born during the sweep keeps its two minutes"
    );
}
