//! Choosing where gas goes: wind, a bail roll, and the reference's neighbour order.
//!
//! Transcribed from `map::spread_gas` (`src/map_field.cpp:276-368`) and
//! `map::get_wind_blockers` (`src/map_field.cpp:2081-2105`). The neighbour order is the
//! reference's: north-west, north, north-east, west, east, south-west, south, south-east.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position, spread_gas, wind_blocker_offsets};

fn spawn(world: &mut World, x: i32, y: i32, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    world
        .spawn((
            Position { x, y },
            Field {
                id: FieldId(1),
                entry,
                priority: 0,
            },
        ))
        .id()
}

fn intensities(world: &mut World) -> Vec<((i32, i32), i32)> {
    let mut query = world.query::<(&Position, &Field)>();
    let mut out: Vec<((i32, i32), i32)> = query
        .iter(world)
        .map(|(p, f)| ((p.x, p.y), f.entry.intensity()))
        .collect();
    out.sort_by_key(|&(square, _)| square);
    out
}

#[test]
fn the_wind_blocker_table_matches_the_reference() {
    // Ordered by descending threshold, exactly as the reference's table is: 330 and 0 both give
    // the row that faces a north wind.
    assert_eq!(wind_blocker_offsets(0), [(1, 0), (1, -1), (1, 1)]);
    assert_eq!(wind_blocker_offsets(60), [(0, -1), (-1, -1), (1, -1)]);
    assert_eq!(wind_blocker_offsets(150), [(-1, 0), (-1, -1), (-1, 1)]);
    assert_eq!(wind_blocker_offsets(240), [(0, 1), (-1, 1), (1, 1)]);
    assert_eq!(wind_blocker_offsets(301), [(1, 1), (1, 0), (0, 1)]);
}

#[test]
fn a_single_unit_of_gas_never_spreads_no_matter_what_it_rolls() {
    for seed in 0..20 {
        let mut world = World::new();
        let source = spawn(&mut world, 5, 5, 1);
        let mut rolls = Rolls::new(seed);
        spread_gas(&mut world, source, 100, 0, 0, |_x, _y| true, &mut rolls);
        assert_eq!(intensities(&mut world), vec![((5, 5), 1)], "seed {seed}");
    }
}

#[test]
fn a_field_that_never_meets_its_spread_chance_stays_put() {
    // percent_spread 0 against rng(1, 100 - windpower): the roll always exceeds it.
    let mut world = World::new();
    let source = spawn(&mut world, 5, 5, 3);
    let mut rolls = Rolls::new(4);
    spread_gas(&mut world, source, 0, 0, 0, |_x, _y| true, &mut rolls);
    assert_eq!(intensities(&mut world), vec![((5, 5), 3)]);
}

#[test]
fn when_it_spreads_it_moves_one_unit_to_one_neighbour() {
    // percent_spread 100 with no wind: the bail roll cannot fail, so the gas always moves, and
    // it moves once - the source thins by one and exactly one other square appears.
    let mut world = World::new();
    let source = spawn(&mut world, 5, 5, 3);
    let mut rolls = Rolls::new(9);
    spread_gas(&mut world, source, 100, 0, 0, |_x, _y| true, &mut rolls);
    let after = intensities(&mut world);
    assert_eq!(after.len(), 2, "the source and one destination: {after:?}");
    assert!(
        after.contains(&((5, 5), 2)),
        "the source thinned by one: {after:?}"
    );
    assert_eq!(
        after.iter().filter(|&&(_, i)| i == 1).count(),
        1,
        "one thin field moved"
    );
}

#[test]
fn a_wall_square_is_not_spread_into() {
    let mut world = World::new();
    let source = spawn(&mut world, 5, 5, 3);
    let mut rolls = Rolls::new(3);
    // Everything north and east of the source is impassable.
    spread_gas(
        &mut world,
        source,
        100,
        0,
        0,
        |x, y| !(y < 5 || x > 5),
        &mut rolls,
    );
    let after = intensities(&mut world);
    for ((x, y), _) in after {
        assert!(
            y >= 5 && x <= 5,
            "spread reached a blocked square: ({x}, {y})"
        );
    }
}
