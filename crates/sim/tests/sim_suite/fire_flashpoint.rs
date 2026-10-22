//! A strong fire feeding its neighbours.
//!
//! Transcribed from `field_processor_fd_fire` (`src/map_field.cpp:1164-1215`): intensity above 1,
//! `one_in(3)`, then five minutes of age handed over per neighbour fed, with a fed fire under 2
//! growing to 2.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{FIRE, Field, Position, process_flashpoint, should_boost};

fn fire(world: &mut World, at: (i32, i32), intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, true);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Position {
                x: at.0,
                y: at.1,
                z: 0,
            },
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
        ))
        .id()
}

fn state(world: &mut World, entity: Entity) -> (i32, i64) {
    world
        .get::<Field>(entity)
        .map_or((0, 0), |field| (field.entry.intensity(), field.entry.age()))
}

#[test]
fn a_weaker_or_older_neighbour_is_worth_feeding() {
    assert!(should_boost(2, -600, 2, -600), "same size and age");
    assert!(should_boost(3, -600, 1, -600), "smaller neighbour");
    assert!(
        should_boost(2, -600, 3, -100),
        "bigger but older: it has less time left"
    );
    assert!(
        !should_boost(2, -600, 3, -900),
        "bigger and younger: leave it alone"
    );
}

#[test]
fn a_small_fire_never_feeds_anyone_and_draws_nothing() {
    let mut world = World::new();
    let source = fire(&mut world, (5, 5), 1, -600);
    let neighbour = fire(&mut world, (6, 5), 1, -600);
    let mut rolls = Rolls::new(3);
    let mut control = Rolls::new(3);
    assert_eq!(process_flashpoint(&mut world, source, &mut rolls), 0);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "the stream did not move"
    );
    assert_eq!(
        state(&mut world, neighbour),
        (1, -600),
        "and nobody was fed"
    );
}

#[test]
fn a_fed_neighbour_grows_to_two_and_gains_five_minutes() {
    for seed in 1..=60 {
        let mut world = World::new();
        let source = fire(&mut world, (5, 5), 2, -600);
        let neighbour = fire(&mut world, (6, 5), 1, -600);
        let mut rolls = Rolls::new(seed);
        let adjacent = process_flashpoint(&mut world, source, &mut rolls);
        if state(&mut world, neighbour) == (1, -600) {
            continue;
        }
        assert_eq!(
            state(&mut world, neighbour),
            (2, -900),
            "seed {seed}: the fed neighbour is bigger and has five more minutes"
        );
        let (source_intensity, source_age) = state(&mut world, source);
        assert_eq!(source_intensity, 2, "the source keeps its size");
        assert!(source_age > -600, "seed {seed}: and paid some of its life");
        assert_eq!(adjacent, 1, "exactly one neighbour was on fire");
        return;
    }
    panic!("some seed in 1..=60 must feed the neighbour");
}

#[test]
fn a_stronger_younger_neighbour_is_left_alone() {
    let mut world = World::new();
    let source = fire(&mut world, (5, 5), 2, -600);
    let neighbour = fire(&mut world, (6, 5), 3, -900);
    let mut rolls = Rolls::new(1);
    process_flashpoint(&mut world, source, &mut rolls);
    assert_eq!(state(&mut world, neighbour), (3, -900), "untouched");
}
