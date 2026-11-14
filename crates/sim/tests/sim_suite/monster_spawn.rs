//! A monster placed by its type arrives with the numbers the type states.
//!
//! Source: `corpus/monsters/monsters.json` (`--dump-monsters` version two), read through the content loader, so
//! this is the whole path - reference data, port loader, world - rather than a hand-built type. A zombie is the
//! case because its numbers are all non-trivial: eighty hit points, seventy speed, four melee skill, two dice.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_json::{Value, parse};
use cdda_sim::creature::{Armour, MeleeAttack, health_of, spawn_monster_of_type, speed_of};
use cdda_sim::{MonsterKindId, Position};
use std::path::Path;

fn zombie_type() -> cdda_content::monsters::MonsterType {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let text = std::fs::read(path).expect("the monster dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    cdda_content::monsters::load_objects(prototypes)
        .expect("the dump loads")
        .into_iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie")
}

#[test]
fn a_zombie_spawned_from_its_type_has_the_numbers_its_type_states() {
    let monster_type = zombie_type();
    let mut world = World::new();

    let zombie =
        spawn_monster_of_type(&mut world, MonsterKindId(42), &monster_type, (3, 4, 0), 100);

    assert_eq!(health_of(&world, zombie), Some(80), "eighty hit points");
    assert_eq!(
        speed_of(&world, zombie),
        70,
        "speed seventy at one hundred percent"
    );
    assert_eq!(
        world
            .get::<Position>(zombie)
            .map(|place| (place.x, place.y)),
        Some((3, 4)),
        "it stands where it was placed"
    );
    let attack = world.get::<MeleeAttack>(zombie).expect("it can attack").0;
    assert_eq!(attack.dice, 2, "two dice");
    assert_eq!(attack.sides, 3, "of three faces");
    assert_eq!(attack.accuracy_tenths, 40, "and its melee skill of four");
    let armour = world.get::<Armour>(zombie).expect("it has a body");
    assert_eq!((armour.bash, armour.cut), (0, 0), "a zombie wears nothing");
    assert_eq!(
        world
            .get::<cdda_sim::creature::CreatureKind>(zombie)
            .map(|kind| kind.0),
        Some(MonsterKindId(42)),
        "and it knows what it is, so its death can leave a body"
    );
    assert!(
        world.get::<cdda_sim::creature::Goal>(zombie).is_none(),
        "a monster that has just been placed has decided nothing"
    );
}

#[test]
fn the_speed_option_scales_what_the_type_states() {
    let monster_type = zombie_type();
    let mut world = World::new();
    let slow = spawn_monster_of_type(&mut world, MonsterKindId(1), &monster_type, (0, 0, 0), 50);
    assert_eq!(speed_of(&world, slow), 35, "half of seventy");

    let mut world = World::new();
    let fast = spawn_monster_of_type(&mut world, MonsterKindId(1), &monster_type, (0, 0, 0), 200);
    assert_eq!(speed_of(&world, fast), 140, "and double");
}

/// A type's dodge reaches the creature it makes.
#[test]
fn a_spawned_monster_dodges_as_much_as_its_type_does() {
    let monster_type = zombie_type();
    let mut world = World::new();
    let zombie = spawn_monster_of_type(&mut world, MonsterKindId(1), &monster_type, (0, 0, 0), 100);
    let defence = cdda_sim::creature::defence_of(&world, zombie);
    assert_eq!(
        defence.dodge, monster_type.dodge,
        "its dodge is what the type states"
    );
}
