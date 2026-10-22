//! The hot air a fire makes, landing in the store.
//!
//! Reference: the last thing the fire processor does is `create_hot_air` (`src/map_field.cpp:1429-1447`), which picks a hot air field by the fire's
//! intensity and drops it on **five** squares within one step. The rule is ported (`cdda_core_types::fire`); this is the join that makes it a haze.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::fire::Neighbour;
use cdda_core_types::rng::{Recording, Rolls};
use cdda_sim::{FIRE, Field, FireMap, HOT_AIR_2, Position, process_fires};

/// A map that asks for hot air and rolls nothing else: no fuel, no spread, no smoke.
struct Quiet;

impl FireMap for Quiet {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn ground(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Ground {
        cdda_core_types::fire::Ground::Inert
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> Neighbour {
        Neighbour::default()
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn smoke_produced(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
}

fn fire(world: &mut World, x: i32, y: i32, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(4, 1_000_000, false, true);
    entry.set_age(1);
    entry.set_intensity(intensity);
    world
        .spawn((
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
            Position { x, y, z: 0 },
        ))
        .id()
}

#[test]
fn a_fire_that_asks_for_hot_air_puts_it_on_the_squares_the_dice_chose() {
    // Gate `int 0 4 1` asks for it; the ten `int -1 1` draws choose the squares, x and y in that order.
    let mut world = World::new();
    fire(&mut world, 5, 5, 2);
    let mut rolls = Rolls::replaying(
        Recording::parse(
            "int 0 4 1\nint -1 1 1\nint -1 1 1\nint -1 1 1\nint -1 1 0\nint -1 1 0\nint -1 1 -1\nint -1 1 0\nint -1 1 -1\nint -1 1 -1\nint -1 1 0",
        )
        .expect("a recording"),
    );
    process_fires(&mut world, &Quiet, 0, &mut rolls);
    let mut query = world.query::<(&Field, &Position)>();
    let mut squares: Vec<(i32, i32, i32)> = query
        .iter(&world)
        .filter(|(field, _)| field.id == HOT_AIR_2)
        .map(|(field, position)| (position.x, position.y, field.entry.intensity()))
        .collect();
    squares.sort_unstable();
    // The dice choose (1,1), (1,0), (0,-1), (0,-1), (-1,0) as offsets from the fire at (5,5). Two of them land on the same square, and a second field
    // of the same kind **adds up** rather than replacing - which is the merge rule from round 305, seen here in the wild: the square north of the fire
    // ends at intensity two because two of the five fell on it.
    assert_eq!(
        squares,
        vec![(4, 5, 1), (5, 4, 2), (6, 5, 1), (6, 6, 1)],
        "four squares, one of them doubled"
    );
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "every recorded die was used"
    );
}

#[test]
fn hot_air_takes_the_field_for_the_fires_own_intensity() {
    // Intensity three makes hot air three, not hot air one: `create_hot_air` switches on the fire's intensity.
    let mut world = World::new();
    fire(&mut world, 2, 2, 3);
    // Spread the five squares out, so the count is five rather than the number of distinct squares.
    let mut rolls = Rolls::replaying(
        Recording::parse("int 0 4 2\nint -1 1 1\nint -1 1 -1\nint -1 1 0\nint -1 1 0\nint -1 1 1\nint -1 1 1\nint -1 1 -1\nint -1 1 1\nint -1 1 0\nint -1 1 1")
            .expect("a recording"),
    );
    process_fires(&mut world, &Quiet, 0, &mut rolls);
    let mut query = world.query::<&Field>();
    let ids: Vec<FieldId> = query
        .iter(&world)
        .filter(|field| field.id != FIRE)
        .map(|field| field.id)
        .collect();
    assert_eq!(ids.len(), 5, "five squares: {ids:?}");
    assert!(
        ids.iter().all(|id| *id == cdda_sim::HOT_AIR_3),
        "all of them hot air three: {ids:?}"
    );
}

#[test]
fn a_refused_gate_makes_no_hot_air_and_spends_one_die() {
    let mut world = World::new();
    fire(&mut world, 5, 5, 2);
    let mut rolls = Rolls::replaying(Recording::parse("int 0 4 0").expect("a recording"));
    process_fires(&mut world, &Quiet, 0, &mut rolls);
    let mut query = world.query::<&Field>();
    assert_eq!(
        query.iter(&world).filter(|field| field.id != FIRE).count(),
        0
    );
    assert_eq!(rolls.recording_remaining(), Some(0));
}
