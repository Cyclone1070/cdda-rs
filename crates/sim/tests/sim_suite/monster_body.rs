//! A monster's body, and what it makes possible: a blow with somewhere to land.
//!
//! Reference: `mtype::bodytype` (`mtype.h:236`) is an **anatomy id**, and `mtype::get_all_body_parts()` asks the anatomy for the parts a blow can land on. Before this round
//! the port gave a body only to creatures it had an anatomy for, so an attack spell against a monster answered "nothing to hit" - which is what round 1468 found and named.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// Two anatomy parts and a world with one monster kind whose body type names them.
fn a_world(with_body: bool) -> (World, Entity) {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -6..=6 {
        for x in -6..=6 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 40,
        }],
    );
    if with_body {
        let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_zombie").expect("the kind is installed");
        cdda_sim::monster_registry::set_bodies(
            &mut world,
            &[(
                kind,
                cdda_sim::creature::Body {
                    parts: vec![a_torso()],
                },
            )],
        );
    }
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster)
}

fn a_torso() -> cdda_sim::hit_location::BodyPart {
    cdda_sim::hit_location::BodyPart {
        name: "torso".to_owned(),
        hit_size: 36.0,
        hit_difficulty: 1.0,
        upper: false,
        lower: false,
        ..Default::default()
    }
}

fn an_attack() -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "attack".to_owned(),
        effect_str: String::new(),
        damage_type: "bash".to_owned(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(9.0),
        range: scaling(9.0),
        area: scaling(0.0),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        flags: std::collections::BTreeSet::new(),
    }
}

/// **A monster spawned from a kind carries the body its type names.**
#[test]
fn a_spawned_monster_carries_the_body_its_kind_names() {
    let (mut world, _caster) = a_world(true);
    let zombie = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(1), 40);
    let body = world
        .get::<cdda_sim::creature::Body>(zombie)
        .expect("the monster has a body");
    assert_eq!(
        body.parts.len(),
        1,
        "**the body is the kind's own**, part for part"
    );
    assert_eq!(body.parts[0].name, "torso", "and it is the part the anatomy states");

    let (mut world, _caster) = a_world(false);
    let bodiless = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(1), 40);
    assert!(
        world.get::<cdda_sim::creature::Body>(bodiless).is_none(),
        "**a kind whose body the world never resolved gets no body at all**, rather than one invented part"
    );
}

/// **An attack spell can now hit a monster**, which is what the body is for.
#[test]
fn an_attack_spell_lands_on_a_monsters_body_part() {
    let (mut world, _caster) = a_world(true);
    let zombie = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spellcasting::cast_attack(&mut world, (0, 0, 0), &an_attack(), 0, zombie, 0.5),
        cdda_sim::spellcasting::Cast::Hit {
            part: "torso".to_owned(),
            dealt: 9
        },
        "**nine damage lands on the monster's torso**"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, zombie),
        Some(31),
        "and the store records the loss"
    );

    // The same cast against a monster of a kind with no body still has nowhere to land, and says so.
    let (mut world, _caster) = a_world(false);
    let bodiless = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spellcasting::cast_attack(&mut world, (0, 0, 0), &an_attack(), 0, bodiless, 0.5),
        cdda_sim::spellcasting::Cast::NothingToHit,
        "**a creature with no parts is a creature a blow misses**, which is the honest answer"
    );
}
