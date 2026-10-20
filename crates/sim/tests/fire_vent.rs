//! The flame burst and the fire vent, which take turns being each other.
//!
//! Transcribed from `field_processor_fd_flame_burst` and `field_processor_fd_fire_vent`
//! (`src/map_field.cpp:626-650`). The field ids here are placeholders: the real ones come from
//! the field loader, and the behaviour being tested is the pair swapping, not the identity.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position, process_fire_vent, process_flame_burst};

const FLAME_BURST: FieldId = FieldId(900);
const FIRE_VENT: FieldId = FieldId(901);

fn spawn(world: &mut World, id: FieldId, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, true);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Position { x: 5, y: 5 },
            Field {
                id,
                entry,
                priority: 0,
            },
        ))
        .id()
}

/// Only the fire pair: the processors also scatter hot air, which is another field on the
/// same map and would otherwise drown the assertion.
fn fields(world: &mut World) -> Vec<(u32, i32, i64)> {
    let mut query = world.query::<&Field>();
    query
        .iter(world)
        .filter(|f| f.id.0 >= 900 && f.id.0 <= 901)
        .map(|f| (f.id.0, f.entry.intensity(), f.entry.age()))
        .collect()
}

#[test]
fn a_strong_flame_burst_just_burns_down() {
    let mut world = World::new();
    let entity = spawn(&mut world, FLAME_BURST, 3, 12);
    let mut rolls = Rolls::new(1);
    process_flame_burst(&mut world, entity, &mut rolls);
    assert_eq!(
        fields(&mut world),
        vec![(900, 2, 12)],
        "one weaker, same age"
    );
}

#[test]
fn a_spent_flame_burst_becomes_a_fire_vent_carrying_its_age() {
    let mut world = World::new();
    let entity = spawn(&mut world, FLAME_BURST, 1, 12);
    let mut rolls = Rolls::new(1);
    process_flame_burst(&mut world, entity, &mut rolls);
    assert_eq!(
        fields(&mut world),
        vec![(901, 3, 12)],
        "the burst is replaced by a vent at full strength, and the old one is gone"
    );
}

#[test]
fn a_spent_fire_vent_becomes_a_flame_burst() {
    let mut world = World::new();
    let entity = spawn(&mut world, FIRE_VENT, 1, 7);
    let mut rolls = Rolls::new(1);
    process_fire_vent(&mut world, entity, &mut rolls);
    assert_eq!(fields(&mut world), vec![(900, 3, 7)]);
}

#[test]
fn a_strong_fire_vent_thins_only_one_time_in_three() {
    // The vent rolls one_in(3) each turn; across seeds both answers must appear, and nothing
    // else may happen. A test that only checked one seed would pass on a broken roll.
    let mut thinned = 0;
    let mut stayed = 0;
    for seed in 0..30 {
        let mut world = World::new();
        let entity = spawn(&mut world, FIRE_VENT, 3, 5);
        let mut rolls = Rolls::new(seed);
        process_fire_vent(&mut world, entity, &mut rolls);
        match fields(&mut world).as_slice() {
            [(901, 3, 5)] => stayed += 1,
            [(901, 2, 5)] => thinned += 1,
            other => panic!("seed {seed} produced {other:?}"),
        }
    }
    assert!(
        thinned > 0 && stayed > 0,
        "both outcomes over 30 seeds: {thinned} thinned, {stayed} stayed"
    );
}

#[test]
fn a_burning_burst_leaves_hot_air_over_the_square() {
    let mut world = World::new();
    let entity = spawn(&mut world, FLAME_BURST, 3, 12);
    let mut rolls = Rolls::new(1);
    process_flame_burst(&mut world, entity, &mut rolls);
    let mut query = world.query::<&Field>();
    let hot: Vec<i32> = query
        .iter(&world)
        .filter(|f| (910..=913).contains(&f.id.0))
        .map(|f| f.entry.intensity())
        .collect();
    assert_eq!(hot.len(), 5, "five wisps, as create_hot_air makes");
    assert!(hot.iter().all(|&i| i == 1), "all thin");
}
