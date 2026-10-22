//! Levels: the same column, different floors.
//!
//! The reference's map is three dimensional and its rules depend on it - gas falls before it drifts, smoke rises, a fire on one floor does not thicken the
//! smoke on the next. These tests are about the store keeping floors apart at all, which has to be true before any of those rules can be right.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::{Recording, Rolls};
use cdda_sim::{Field, Position, PositionIndex, SMOKE, add_field_strength, process_gas};

fn smoke(world: &mut World, x: i32, y: i32, z: i32, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, false);
    entry.set_intensity(intensity);
    world
        .spawn((
            Field {
                id: SMOKE,
                entry,
                priority: 0,
            },
            Position { x, y, z },
        ))
        .id()
}

fn intensity_at(world: &mut World, x: i32, y: i32, z: i32) -> Vec<i32> {
    let mut query = world.query::<(&Field, &Position)>();
    let mut found: Vec<i32> = query
        .iter(world)
        .filter(|(field, position)| {
            field.id == SMOKE && position.x == x && position.y == y && position.z == z
        })
        .map(|(field, _)| field.entry.intensity())
        .collect();
    found.sort_unstable();
    found
}

#[test]
fn the_index_keeps_floors_apart() {
    // Same column, two floors: the index must answer for the floor it was asked about, not for the column.
    let mut world = World::new();
    let ground = smoke(&mut world, 3, 4, 0, 2);
    let cellar = smoke(&mut world, 3, 4, -1, 1);
    PositionIndex::rebuild(&mut world);
    let index = world.resource::<PositionIndex>();
    assert_eq!(index.at_level(3, 4, 0), &[ground]);
    assert_eq!(index.at_level(3, 4, -1), &[cellar]);
    assert_eq!(index.at(3, 4), &[ground], "`at` is street level");
    assert!(index.at_level(3, 4, 1).is_empty());
}

#[test]
fn adding_smoke_on_one_floor_does_not_thicken_the_smoke_below() {
    // The merge rule adds to a field of the same kind on the same square. On a different floor it is a different square, and a fire upstairs must not
    // feed the smoke in the cellar.
    let mut world = World::new();
    smoke(&mut world, 5, 5, 0, 2);
    add_field_strength(&mut world, (5, 5, -1), SMOKE, 1);
    assert_eq!(
        intensity_at(&mut world, 5, 5, 0),
        vec![2],
        "the floor above is untouched"
    );
    assert_eq!(
        intensity_at(&mut world, 5, 5, -1),
        vec![1],
        "and the cellar got its own field"
    );
}

#[test]
fn adding_smoke_on_the_same_floor_still_merges() {
    // The counterpart: the same floor still adds up, or the test above would pass for the wrong reason.
    let mut world = World::new();
    smoke(&mut world, 5, 5, 0, 2);
    add_field_strength(&mut world, (5, 5, 0), SMOKE, 1);
    assert_eq!(intensity_at(&mut world, 5, 5, 0), vec![3]);
}

#[test]
fn a_gas_over_a_hole_falls_instead_of_drifting() {
    // The reference tries the square below before any neighbour (`src/map_field.cpp:314-321`). With a hole under the gas and every side open, it must
    // go down - and the store has to know which floor it went to.
    let mut world = World::new();
    smoke(&mut world, 5, 5, 1, 3);
    let mut rolls = Rolls::replaying(Recording::parse("int 1 100 1").expect("a recording"));
    process_gas(
        &mut world,
        SMOKE,
        10,
        0,
        |_from, to| to == (5, 5, 0) || to.2 == 1,
        &mut rolls,
    );
    assert_eq!(
        intensity_at(&mut world, 5, 5, 0),
        vec![1],
        "the gas fell to the floor below"
    );
    assert_eq!(
        intensity_at(&mut world, 5, 5, 1),
        vec![2],
        "and the square above kept one level less"
    );
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "falling is free: only the gate was drawn"
    );
}

#[test]
fn a_gas_with_no_floor_to_fall_through_does_not_reach_the_floor_below() {
    // The counterpart: the same setup with the floor closed, so it must drift sideways instead of appearing in the room below.
    let mut world = World::new();
    smoke(&mut world, 5, 5, 1, 3);
    let mut rolls = Rolls::replaying(
        Recording::parse("int 1 100 1\nint 0 7 0\nint 0 7 0\nint 0 7 0").expect("a recording"),
    );
    process_gas(&mut world, SMOKE, 10, 0, |_from, to| to.2 == 1, &mut rolls);
    assert_eq!(
        intensity_at(&mut world, 5, 5, 0),
        Vec::<i32>::new(),
        "nothing went through the floor"
    );
    assert_eq!(
        intensity_at(&mut world, 5, 5, 1),
        vec![2],
        "the gas drifted on its own floor"
    );
}

#[test]
fn a_gas_with_nothing_sideways_rises() {
    // Rising is the last resort (`:345-352`): with no floor above and nothing beside, it goes up.
    let mut world = World::new();
    smoke(&mut world, 5, 5, 1, 3);
    let mut rolls =
        Rolls::replaying(Recording::parse("int 1 100 1\nint 0 7 0").expect("a recording"));
    process_gas(
        &mut world,
        SMOKE,
        10,
        0,
        |_from, to| to == (5, 5, 2),
        &mut rolls,
    );
    assert_eq!(
        intensity_at(&mut world, 5, 5, 2),
        vec![1],
        "the smoke rose to the next floor"
    );
    assert_eq!(intensity_at(&mut world, 5, 5, 1), vec![2]);
}

/// A map that makes a lot of smoke and refuses nothing.
struct Smoky;
impl cdda_sim::FireMap for Smoky {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        9
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn time_added(&self, _square: (i32, i32, i32)) -> i64 {
        0
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Neighbour {
        cdda_core_types::fire::Neighbour::default()
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn smoke_produced(&self, _square: (i32, i32, i32)) -> i32 {
        100
    }
}
/// A map that records which square it was asked about, and burns hard.
struct Watched(std::cell::RefCell<Vec<(i32, i32, i32)>>);

impl cdda_sim::FireMap for Watched {
    fn adjacent_fires(&self, square: (i32, i32, i32)) -> i32 {
        self.0.borrow_mut().push(square);
        0
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn time_added(&self, _square: (i32, i32, i32)) -> i64 {
        0
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Neighbour {
        cdda_core_types::fire::Neighbour::default()
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn smoke_produced(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
}

#[test]
fn the_fire_system_asks_the_map_about_a_whole_square_not_a_column() {
    // A fire on the first floor is not the same square as a fire on the ground, so the questions the system asks the map have to carry the floor. This is
    // the test that fails while `FireMap` still speaks in two coordinates.
    let mut world = World::new();
    let mut entry = FieldEntry::new(4, 1_000_000, false, true);
    entry.set_age(1);
    entry.set_intensity(2);
    world.spawn((
        Field {
            id: cdda_sim::FIRE,
            entry,
            priority: 0,
        },
        Position { x: 5, y: 5, z: 1 },
    ));
    let watched = Watched(std::cell::RefCell::new(Vec::new()));
    let mut rolls = Rolls::new(11);
    cdda_sim::process_fires(&mut world, &watched, 0, &mut rolls);
    let asked = watched.0.borrow();
    assert!(!asked.is_empty(), "the map was asked something");
    assert!(
        asked.iter().all(|square| square.2 == 1),
        "every question named the fire's own floor: {asked:?}"
    );
}

#[test]
fn a_fire_upstairs_leaves_its_smoke_upstairs() {
    // The product lands where the fire is, not on the ground floor.
    let mut world = World::new();
    let mut entry = FieldEntry::new(4, 1_000_000, false, true);
    entry.set_age(1);
    entry.set_intensity(3);
    world.spawn((
        Field {
            id: cdda_sim::FIRE,
            entry,
            priority: 0,
        },
        Position { x: 2, y: 2, z: 1 },
    ));
    let mut rolls = Rolls::new(3);
    cdda_sim::process_fires(&mut world, &Smoky, 0, &mut rolls);
    let mut query = world.query::<(&Field, &Position)>();
    assert!(
        query
            .iter(&world)
            .any(|(field, position)| field.id == SMOKE && position.z == 1),
        "the smoke is on the fire's own floor"
    );
    let mut query = world.query::<(&Field, &Position)>();
    assert!(
        !query
            .iter(&world)
            .any(|(field, position)| field.id == SMOKE && position.z == 0),
        "no smoke fell through the floor"
    );
}
