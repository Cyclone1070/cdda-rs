//! A blow lands on a part of a body, and the store remembers which.
//!
//! Reference: `Creature::deal_melee_hit` (`creature.cpp:959`) picks the part with
//! `select_body_part( -1, -1, can_attack_high, hit_spread )` and `deal_damage` records the damage against it. The
//! parts come from the game's own anatomy (`corpus/anatomy-parts.json`) crossed with the reference's body part
//! numbers (`corpus/anatomy.json`), so this is the join of the two halves rather than a third opinion.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;
use cdda_json::{Value, parse};
use cdda_sim::creature::{
    Body, MeleeAttack, health_of, set_goal, set_health, set_speed, spawn_creature, wound_on,
};
use cdda_sim::hit_location::BodyPart;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Prototypes, singletons};
use std::path::Path;

const FLOOR: u32 = 1;

fn read(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(name);
    let text = std::fs::read(path).unwrap_or_else(|_| panic!("{name} exists"));
    parse(&text).unwrap_or_else(|_| panic!("{name} is valid JSON"))
}

/// A person's body, assembled from the two corpus files the reference gives.
fn human_body() -> Vec<BodyPart> {
    let Value::Array(anatomies) = read("anatomy-parts.json") else {
        panic!("the anatomies are an array");
    };
    let loaded = cdda_content::anatomies::load_objects(&anatomies).expect("the anatomies load");
    let human = loaded
        .into_iter()
        .find(|anatomy| anatomy.id == "human_anatomy")
        .expect("a person's anatomy");

    let Value::Object(root) = read("anatomy.json") else {
        panic!("the dump's root is an object");
    };
    let Some(Value::Array(parts)) = root.get("parts") else {
        panic!("the dump has parts");
    };
    let numbers = cdda_content::body_parts::load_objects(parts).expect("the parts load");
    human
        .parts
        .iter()
        .map(|id| {
            let found = numbers
                .iter()
                .find(|part| &part.id == id)
                .unwrap_or_else(|| panic!("the reference built no {id}"));
            BodyPart {
                name: found.name.clone(),
                hit_size: found.hit_size,
                hit_difficulty: found.hit_difficulty,
                upper: found.upper,
                lower: found.lower,
            }
        })
        .collect()
}

fn corridor() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..4 {
        level.set_terrain(x, 0, FLOOR, FLOOR);
    }
    cdda_sim::load_level(&mut world, &level);
    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert_move_cost(FLOOR, 2);
    cdda_sim::set_prototypes(
        &mut world,
        Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );
    world
}

fn attacker(world: &mut World) -> Entity {
    let entity = spawn_creature(world, "a zombie", (0, 0, 0));
    set_health(world, entity, 20);
    set_speed(world, entity, 100, 0);
    world.entity_mut(entity).insert(MeleeAttack(cdda_sim::melee::Attack {
        accuracy_tenths: 400,
        fixed: cdda_sim::melee::Damage::default(),
        dice: 1,
        sides: 1,
    }));
    entity
}

#[test]
fn a_blow_lands_on_a_part_of_the_body_and_the_store_remembers_which() {
    let mut world = corridor();
    let zombie_entity = attacker(&mut world);
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    set_health(&mut world, survivor, 100);
    let body = human_body();
    assert_eq!(body.len(), 12, "a person has twelve parts");
    world.entity_mut(survivor).insert(Body { parts: body.clone() });
    set_goal(&mut world, zombie_entity, (1, 0, 0));
    singletons::put_rolls(&mut world, Rolls::new(4));

    cdda_sim::turn::run_turn(&mut world);

    let taken = 100 - health_of(&world, survivor).expect("still alive");
    assert_eq!(taken, 1, "one die of one face does exactly one");
    let wounds = world.get::<cdda_sim::creature::Wounds>(survivor).expect("a wound");
    assert_eq!(wounds.0.len(), 1, "one blow, one part");
    let (part, amount) = wounds.0.iter().next().expect("a part");
    assert_eq!(*amount, taken, "the wound is the damage");
    assert!(
        body.iter().any(|known| &known.name == part),
        "{part} is not a part of a person"
    );
}

#[test]
fn a_creature_with_no_body_takes_the_damage_and_records_no_part() {
    let mut world = corridor();
    let zombie_entity = attacker(&mut world);
    let victim = spawn_creature(&mut world, "another zombie", (1, 0, 0));
    set_health(&mut world, victim, 100);
    set_goal(&mut world, zombie_entity, (1, 0, 0));
    singletons::put_rolls(&mut world, Rolls::new(4));

    cdda_sim::turn::run_turn(&mut world);

    assert_eq!(health_of(&world, victim), Some(99), "the damage still lands");
    assert!(
        world.get::<cdda_sim::creature::Wounds>(victim).is_none(),
        "but a creature with no body has no part to record it against"
    );
    assert_eq!(wound_on(&world, victim, "torso"), 0);
}
