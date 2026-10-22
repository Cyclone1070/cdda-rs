//! Fire as it lives in the store: a field entity that grows, and one that gutters out.
//!
//! Reference: the fire processor (`src/map_field.cpp:945-1360`) runs inside the per-turn field sweep, so what the store needs is a system
//! that walks the fire entities and hands each one its turn. Two of the processor's inputs are the map's rather than the fire's - how many
//! fires touch it, and whether the ground under it can burn - so they arrive as answers the environment layer supplies.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::fire::Neighbour;
use cdda_core_types::rng::Rolls;
use cdda_sim::{FIRE, Field, FireMap, Position, process_fires};

/// A map that answers every question the same way everywhere, so a test varies one answer at a time.
#[derive(Default)]
struct FlatMap {
    adjacent_fires: i32,
    can_burn: bool,
    time_added: i64,
    can_spread: bool,
    neighbour: Neighbour,
    suppress_smoke: bool,
    smoke_produced: i32,
}

impl FireMap for FlatMap {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        self.adjacent_fires
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        self.can_burn
    }
    fn time_added(&self, _square: (i32, i32, i32)) -> i64 {
        self.time_added
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        self.can_spread
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> Neighbour {
        self.neighbour
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        self.suppress_smoke
    }
    fn smoke_produced(&self, _square: (i32, i32, i32)) -> i32 {
        self.smoke_produced
    }
}

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
            Position { x, y, z: 0 },
        ))
        .id()
}

#[test]
fn a_fed_fire_with_neighbours_grows() {
    let mut world = World::new();
    let entity = fire_at(&mut world, 5, 5, 1, -10_000);
    process_fires(
        &mut world,
        &FlatMap {
            adjacent_fires: 7,
            can_burn: true,
            time_added: 0,
            can_spread: true,
            neighbour: Neighbour::default(),
            ..FlatMap::default()
        },
        0,
        &mut Rolls::new(1),
    );
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
    // One turn old, not brand new: a field created this turn is spared its processors entirely (`map_field.cpp:445-499`), so a fire at age
    // zero would show none of this rule - and the reference's own five-turn trace starts 0, 1, 22 for exactly that reason.
    let entity = fire_at(&mut world, 5, 5, 2, 1);
    process_fires(
        &mut world,
        &FlatMap {
            adjacent_fires: 0,
            can_burn: true,
            time_added: 0,
            can_spread: true,
            neighbour: Neighbour::default(),
            ..FlatMap::default()
        },
        0,
        &mut Rolls::new(1),
    );
    let field = world.get::<Field>(entity).expect("the fire");
    assert_eq!(
        field.entry.age(),
        21,
        "ten seconds a level on top of the turn it had already burned"
    );
    assert_eq!(field.entry.intensity(), 2, "and no growth without fuel");
}

#[test]
fn a_fire_that_ate_this_turn_spends_what_it_ate() {
    // What the fire consumed arrives as turns of fuel, and it is credited before the growth test: two hours of fuel both pays for the
    // level it climbs and leaves the fire longer-lived than it was.
    let mut world = World::new();
    let entity = fire_at(&mut world, 5, 5, 1, -10);
    process_fires(
        &mut world,
        &FlatMap {
            adjacent_fires: 0,
            can_burn: true,
            time_added: 7_200,
            can_spread: true,
            neighbour: Neighbour::default(),
            ..FlatMap::default()
        },
        0,
        &mut Rolls::new(1),
    );
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
            Position { x: 1, y: 1, z: 0 },
        ))
        .id();
    process_fires(
        &mut world,
        &FlatMap {
            adjacent_fires: 0,
            can_burn: false,
            time_added: 0,
            can_spread: true,
            neighbour: Neighbour::default(),
            ..FlatMap::default()
        },
        0,
        &mut Rolls::new(1),
    );
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
            Position { x: 2, y: 2, z: 0 },
        ))
        .id();
    process_fires(
        &mut world,
        &FlatMap {
            adjacent_fires: 9,
            can_burn: true,
            time_added: 5_000,
            can_spread: true,
            neighbour: Neighbour::default(),
            ..FlatMap::default()
        },
        0,
        &mut Rolls::new(1),
    );
    let field = world.get::<Field>(entity).expect("the field");
    assert_eq!(field.entry.age(), 5, "smoke does not grow or spend fuel");
    assert_eq!(field.entry.intensity(), 2);
}
