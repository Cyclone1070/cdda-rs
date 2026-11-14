//! Hot air above a fire: five wisps of it, scattered around the square.
//!
//! Transcribed from `map::create_hot_air` (`src/map_field.cpp:375-400`).

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position, create_hot_air};

fn hot_air_squares(world: &mut World) -> Vec<(i32, i32, u32, i32)> {
    let mut query = world.query::<(&Position, &Field)>();
    let mut out: Vec<(i32, i32, u32, i32)> = query
        .iter(world)
        .map(|(p, f)| (p.x, p.y, f.id.0, f.entry.intensity()))
        .collect();
    out.sort_unstable();
    out
}

#[test]
fn a_fire_scatters_five_wisps_of_hot_air_around_itself() {
    // The reference asks for `intensity` placements, but two of them can land on the same square - and a square that already has smoke gains strength rather
    // than a second entity. So the promise is "up to five, at least one", and five distinct squares are reachable: both are asserted, over spread seeds.
    let mut most = 0;
    for attempt in 0..16_u32 {
        let mut world = World::new();
        let mut rolls = Rolls::new(crate::spread_seed(attempt));
        create_hot_air(&mut world, (5, 5), 0, 2, &mut rolls);
        let wisps = hot_air_squares(&mut world);
        assert!(!wisps.is_empty(), "a fire scatters at least one wisp");
        assert!(
            wisps.len() <= 5,
            "and never more than the intensity asks for: {}",
            wisps.len()
        );
        most = most.max(wisps.len());
    }
    assert_eq!(most, 5, "five distinct squares are reachable");
    let mut world = World::new();
    let mut rolls = Rolls::new(crate::spread_seed(0));
    create_hot_air(&mut world, (5, 5), 0, 2, &mut rolls);
    for (x, y, _id, intensity) in &hot_air_squares(&mut world) {
        assert!(
            (4..=6).contains(x) && (4..=6).contains(y),
            "({x}, {y}) is not adjacent"
        );
        assert_eq!(*intensity, 1, "each wisp is thin");
    }
}

#[test]
fn the_intensity_picks_which_hot_air_this_is() {
    // The reference's switch maps intensity one to four onto four different field types; the
    // ids here are placeholders, but they must differ per intensity or the switch is not ported.
    let mut ids = Vec::new();
    for intensity in 1..=4 {
        let mut world = World::new();
        let mut rolls = Rolls::new(1);
        create_hot_air(&mut world, (0, 0), 0, intensity, &mut rolls);
        ids.push(hot_air_squares(&mut world)[0].2);
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 4, "four intensities, four kinds");
}

#[test]
fn an_impossible_intensity_draws_no_dice_and_creates_nothing() {
    // The reference's default branch is a debug message and a return, before any roll.
    let mut world = World::new();
    let mut rolls = Rolls::new(5);
    create_hot_air(&mut world, (5, 5), 0, 7, &mut rolls);
    assert!(hot_air_squares(&mut world).is_empty());
    assert_eq!(
        rolls.engine().next_u32(),
        Rolls::new(5).engine().next_u32(),
        "the stream is untouched"
    );
}

#[test]
fn a_real_call_advances_the_stream_by_ten_rolls() {
    // Two rng(-1, 1) per placement, five placements.
    let mut world = World::new();
    let mut rolls = Rolls::new(5);
    create_hot_air(&mut world, (5, 5), 0, 1, &mut rolls);
    let mut expected = Rolls::new(5);
    for _ in 0..10 {
        expected.rng(-1, 1);
    }
    assert_eq!(rolls.engine().next_u32(), expected.engine().next_u32());
}
