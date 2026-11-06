//! A blow lands on a part of a body, and the store remembers which.
//!
//! Reference: `Creature::deal_melee_hit` (`creature.cpp:959`) picks the part with
//! `select_body_part( -1, -1, can_attack_high, hit_spread )` and `deal_damage` records the damage against it. The
//! parts come from the game's own anatomy (`corpus/anatomy-parts.json`) crossed with the reference's body part
//! numbers (`corpus/anatomy.json`), so this is the join of the two halves rather than a third opinion.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

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

/// Armour is asked **per part**, which is the reason a blow bothers to pick one.
///
/// Reference: `Character::get_armor_type( dt, bp )` (`character_armor.cpp:70-86`) - the resistance a blow meets
/// depends on where it landed, so a vest on the torso is worth nothing against a blow that reaches an arm. Two
/// worlds on one seed land on the same part with the same dice, so the difference in damage is the armour alone.
#[test]
fn armour_protects_the_part_it_covers_and_no_other() {
    /// The damage a blow does to a survivor wearing the given garment, on one seed.
    fn blow_with(garment: Option<cdda_sim::creature::Garment>) -> i32 {
        let mut world = corridor();
        let zombie_entity = spawn_creature(&mut world, "a zombie", (0, 0, 0));
        set_health(&mut world, zombie_entity, 20);
        set_speed(&mut world, zombie_entity, 100, 0);
        world
            .entity_mut(zombie_entity)
            .insert(MeleeAttack(cdda_sim::melee::Attack {
                accuracy_tenths: 400,
                fixed: cdda_sim::melee::Damage::default(),
                dice: 6,
                sides: 1,
            }));
        let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
        set_health(&mut world, survivor, 100);
        world
            .entity_mut(survivor)
            .insert(Body { parts: human_body() });
        if let Some(garment) = garment {
            world.spawn((
                cdda_sim::creature::WornBy(survivor),
                garment,
            ));
        }
        set_goal(&mut world, zombie_entity, (1, 0, 0));
        singletons::put_rolls(&mut world, Rolls::new(11));
        cdda_sim::turn::run_turn(&mut world);
        let wounds = world.get::<cdda_sim::creature::Wounds>(survivor);
        let (part, amount) = wounds
            .expect("a wound")
            .0
            .iter()
            .next()
            .map(|(part, amount)| (part.clone(), *amount))
            .expect("one part");
        assert!(
            human_body().iter().any(|known| known.name == part),
            "{part} is not a part of a person"
        );
        amount
    }

    fn cover(part: &str) -> cdda_sim::creature::Garment {
        cdda_sim::creature::Garment {
            layers: vec![(
                part.to_owned(),
                cdda_content::armour::Layer {
                    // Four per millimetre, one millimetre thick: four points.
                    resist: 4.0,
                    thickness: 1.0,
                    cover: 100,
                },
            )],
            vitals: Vec::new(),
        }
    }

    let bare_part = {
        let mut world = corridor();
        let zombie_entity = spawn_creature(&mut world, "a zombie", (0, 0, 0));
        set_health(&mut world, zombie_entity, 20);
        set_speed(&mut world, zombie_entity, 100, 0);
        world
            .entity_mut(zombie_entity)
            .insert(MeleeAttack(cdda_sim::melee::Attack {
                accuracy_tenths: 400,
                fixed: cdda_sim::melee::Damage::default(),
                dice: 6,
                sides: 1,
            }));
        let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
        set_health(&mut world, survivor, 100);
        world
            .entity_mut(survivor)
            .insert(Body { parts: human_body() });
        set_goal(&mut world, zombie_entity, (1, 0, 0));
        singletons::put_rolls(&mut world, Rolls::new(11));
        cdda_sim::turn::run_turn(&mut world);
        let wounds = world.get::<cdda_sim::creature::Wounds>(survivor).expect("a wound");
        wounds.0.iter().next().map(|(part, _)| part.clone()).expect("one part")
    };

    let bare = blow_with(None);
    assert_eq!(bare, 6, "six dice of one face, nothing in the way");

    // Cover **only** the part the same seed lands on: the blow meets that armour and nothing else's. The armour is
    // worn rather than set, because `PartArmour` is derived now - `creature::dress` writes it from the garments a
    // creature has on, and a hand-written map would be the stale answer the derive exists to prevent.
    assert_eq!(
        blow_with(Some(cover(&bare_part))),
        2,
        "six through four points on the {bare_part} is two"
    );

    // And covering a part the blow does not reach changes nothing, which is the half a whole-body armour
    // number could never express.
    let other = human_body()
        .into_iter()
        .map(|part| part.name)
        .find(|name| name != &bare_part)
        .expect("a person has more than one part");
    assert_eq!(
        blow_with(Some(cover(&other))),
        6,
        "armour on a part the blow misses is worth nothing"
    );
}

/// A garment's wear reaches the part it protects, and nothing else has to ask.
///
/// Reference: `item::_resist` scales a garment's whole material sum by its own wear (`item.cpp:9226-9234`) and `Character::get_armor_type` is the sum of that over the garments covering the
/// part (`character_armor.cpp:70-87`, `character_attire.cpp:1973-1983`). **The port derived `PartArmour` once, when the garment went on**, so a jacket that was whole at the time stayed whole
/// for ever - the wear rule was ported and unreachable, which is the gap a worn-out jacket's four points becoming two has to close.
#[test]
fn a_worn_out_garment_protects_less_of_the_part_it_covers() {
    use cdda_content::armour::Layer;
    use cdda_sim::creature::{Garment, Wear, WornBy, armour_on};

    let mut world = bevy_ecs::world::World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let jacket = world
        .spawn(Garment {
            layers: vec![(
                "torso".to_owned(),
                Layer {
                    resist: 4.0,
                    thickness: 1.0,
                    cover: 100,
                },
            )],
            vitals: Vec::new(),
        })
        .id();
    world.entity_mut(jacket).insert(WornBy(survivor));

    cdda_sim::creature::dress(&mut world);
    assert_eq!(armour_on(&world, survivor, "torso"), 4, "a whole jacket protects by four");

    // **Four thousand points of damage is the fifth level, and the same jacket protects by two** - which `dress` has to be asked for again, because the armour is derived and not a rule of
    // the blow.
    world.entity_mut(jacket).insert(Wear { damage: 4_000 });
    cdda_sim::creature::dress(&mut world);
    assert_eq!(
        armour_on(&world, survivor, "torso"),
        2,
        "**a level-five jacket keeps half its protection, through the same derived figure**"
    );
    assert_eq!(armour_on(&world, survivor, "head"), 0, "and a part it does not cover is still bare");
}
