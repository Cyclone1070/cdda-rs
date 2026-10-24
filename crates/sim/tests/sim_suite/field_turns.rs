//! What a field does on its turn, in the one place fields live.
//!
//! Reference: `map::process_fields_in_submap` (`src/map_field.cpp:470-499`). A field is an **entity** - it has a square, a
//! strength and an age - so the turn is a system over the store, not a container on the map. The three steps are the rule: a dead
//! field is gone before anything runs; a field created this turn is spared its processors and only decays; and the decay comes
//! **last**, so a field that spawns this turn is still at full strength while it does.
//!
//! The intensity matters too: every processor asks the field how strong it is before it acts (`src/map_field.cpp:764`), so a
//! sweep that used one level's numbers for all of them would spawn at the wrong rate on every field with more than one level.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_content::fields::{FieldLevel, FieldTurn, FieldType};
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position, process_field_turns};

const KIND: u32 = 7;

fn level(spawn_chance: i32, spawn_count: i32) -> FieldLevel {
    FieldLevel {
        name: String::new(),
        sym: 0,
        color: String::new(),
        dangerous: false,
        transparent: true,
        translucency: 0.0,
        move_cost: 0,
        upgrade_chance: 0,
        upgrade_duration: 0,
        spawn_chance,
        spawn_count,
        spawn_radius: 0,
        spawn_group: if spawn_chance > 0 {
            "GROUP_ZOMBIE".to_owned()
        } else {
            String::new()
        },
        extra_radiation_min: 0,
        extra_radiation_max: 0,
    }
}

/// A type whose first level spawns and whose second does not, so intensity is observable.
fn spawning_type() -> FieldType {
    FieldType {
        id: "fd_probe".to_owned(),
        priority: 0,
        underwater_age_speedup: 0,
        outdoor_age_speedup: 0,
        percent_spread: 0,
        half_life: 60,
        linear_half_life: false,
        wandering_field: "fd_null".to_owned(),
        levels: vec![level(1, 1), level(0, 0)],
    }
}

fn turn_of() -> impl Fn(&Field) -> Option<FieldTurn> {
    let field_type = spawning_type();
    move |field| {
        (field.id.0 == KIND)
            .then(|| cdda_content::fields::turn_at(&field_type, field.entry.intensity()))
            .flatten()
    }
}

fn spawn_free(_turn: &FieldTurn, _square: (i32, i32), _rolls: &mut Rolls) {}

fn field_at(world: &mut World, x: i32, y: i32, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, false);
    entry.set_age(age);
    entry.set_intensity(intensity);
    world
        .spawn((
            Field {
                id: FieldId(KIND),
                entry,
                priority: 0,
            },
            Position { x, y, z: 0 },
        ))
        .id()
}

#[test]
fn a_field_created_this_turn_is_spared_its_processors() {
    let mut world = World::new();
    let entity = field_at(&mut world, 5, 5, 1, 0);
    let mut rolls = Rolls::new(4);
    let mut turns = 0;
    process_field_turns(
        &mut world,
        0,
        &mut rolls,
        turn_of(),
        |_turn: &FieldTurn, _square, _rolls: &mut Rolls| {
            turns += 1;
        },
    );
    assert_eq!(turns, 0, "a newborn field is spared its processors");
    // It still decays - that is the whole of what happens to it - so its age moves off zero.
    assert_eq!(
        world.get::<Field>(entity).expect("the field").entry.age(),
        1
    );
}

#[test]
fn a_field_that_has_been_there_a_turn_spawns_and_then_decays() {
    let mut world = World::new();
    let entity = field_at(&mut world, 5, 5, 1, 1);
    let mut rolls = Rolls::new(4);
    let mut turns = 0;
    process_field_turns(
        &mut world,
        1,
        &mut rolls,
        turn_of(),
        |_turn: &FieldTurn, _square, _rolls: &mut Rolls| {
            turns += 1;
        },
    );
    assert_eq!(turns, 1, "an aged field takes its turn");
    assert!(world.get::<Field>(entity).expect("the field").entry.age() >= 1);
}

#[test]
fn the_turn_comes_from_the_fields_live_intensity() {
    // The type's first level spawns one monster and its second spawns none, so the turn handed over is visibly different depending
    // only on how strong the field currently is. A sweep that used one level's numbers for everything would hand over the same
    // turn twice.
    let mut weak_world = World::new();
    field_at(&mut weak_world, 4, 4, 1, 1);
    let mut rolls = Rolls::new(9);
    let mut weak = Vec::new();
    process_field_turns(
        &mut weak_world,
        1,
        &mut rolls,
        turn_of(),
        |turn: &FieldTurn, _, _| {
            weak.push(turn.spawn_count);
        },
    );
    assert_eq!(weak, vec![1], "an intensity-one field spawns one");

    let mut strong_world = World::new();
    field_at(&mut strong_world, 4, 4, 2, 1);
    let mut rolls = Rolls::new(9);
    let mut strong = Vec::new();
    process_field_turns(
        &mut strong_world,
        1,
        &mut rolls,
        turn_of(),
        |turn: &FieldTurn, _, _| {
            strong.push(turn.spawn_count);
        },
    );
    assert_eq!(strong, vec![0], "an intensity-two field spawns none");
}

#[test]
fn a_field_that_died_during_its_turn_leaves_the_store() {
    let mut world = World::new();
    let mut entry = FieldEntry::new(1, 1, false, false);
    entry.set_age(1_000_000);
    let entity = world
        .spawn((
            Field {
                id: FieldId(KIND),
                entry,
                priority: 0,
            },
            Position { x: 2, y: 2, z: 0 },
        ))
        .id();
    let mut rolls = Rolls::new(4);
    process_field_turns(&mut world, 1_000_000, &mut rolls, turn_of(), spawn_free);
    assert!(
        world.get::<Field>(entity).is_none(),
        "a decayed field is gone"
    );
    let mut remaining = world.query::<&Field>();
    assert_eq!(
        remaining.iter(&world).count(),
        0,
        "and nothing is left behind"
    );
}

#[test]
fn the_sweep_walks_the_map_column_by_column() {
    // x outside, y inside: the order is the order the dice are spent, and a sweep in the other order produces a world that looks
    // right and is not the same world.
    let mut world = World::new();
    for (x, y) in [(3, 9), (0, 0), (3, 2), (0, 11)] {
        field_at(&mut world, x, y, 1, 1);
    }
    let mut rolls = Rolls::new(21);
    let mut visited = Vec::new();
    process_field_turns(
        &mut world,
        1,
        &mut rolls,
        turn_of(),
        |_turn: &FieldTurn, square, _rolls: &mut Rolls| {
            visited.push(square);
        },
    );
    assert_eq!(
        visited,
        vec![(0, 0), (0, 11), (3, 2), (3, 9)],
        "column by column"
    );
}
