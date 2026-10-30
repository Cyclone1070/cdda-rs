//! Casting an attack spell at a creature.
//!
//! Reference: `spell::cast` -> `spell_effect::target_attack` -> `damage_target` (`magic.cpp`, `spell_effect.cpp`).
//! The rules the port keeps are the ones a player would notice: the spell reaches as far as its level says, it deals
//! its level-scaled damage, the muzzle lands on one part of a body, and the armour on that part meets it - a vest
//! stops a taser for the same reason it stops a fist.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_content::spells::Scaling;
use cdda_sim::creature::{Body, Garment, WornBy, dress, health_of, set_health, spawn_creature};
use cdda_sim::hit_location::BodyPart;
use cdda_sim::spellcasting::{Cast, cast_attack};
use cdda_sim::spawn_monster;

fn torso_only() -> Body {
    Body {
        parts: vec![BodyPart {
            name: "torso".to_owned(),
            hit_size: 36.0,
            hit_difficulty: 1.0,
            upper: false,
            lower: false,
        }],
    }
}

/// The taser from the game's data: `bio_taser_spell`, fifteen electric damage, one square of reach.
fn taser() -> cdda_content::spells::Spell {
    cdda_content::spells::Spell {
        id: "bio_taser_spell".to_owned(),
        name: "Super Stun Gun".to_owned(),
        effect_name: "attack".to_owned(),
        effect_str: String::new(),
        damage_type: "electric".to_owned(),
        shape: "blast".to_owned(),
        energy_source: "BIONIC".to_owned(),
        damage: Scaling {
            start: Some(15.0),
            increment: Some(0.0),
            end: Some(15.0),
        },
        range: Scaling {
            start: Some(1.0),
            increment: Some(0.0),
            end: Some(1.0),
        },
        area: Scaling::default(),
        energy: Scaling::default(),
        casting_time: Scaling::default(),
        difficulty: None,
        max_level: None,
    }
}

fn world_with_victim(at: (i32, i32)) -> (World, Entity) {
    let mut world = World::new();
    let victim = spawn_creature(&mut world, "a zombie", (at.0, at.1, 0));
    world.entity_mut(victim).insert(torso_only());
    set_health(&mut world, victim, 80);
    (world, victim)
}

#[test]
fn a_taser_at_arm_s_length_takes_fifteen_off_a_bare_torso() {
    let (mut world, victim) = world_with_victim((1, 0));
    let cast = cast_attack(&mut world, (0, 0, 0), &taser(), 0, victim, 0.0);
    assert_eq!(
        cast,
        Cast::Hit {
            part: "torso".to_owned(),
            dealt: 15
        },
        "fifteen electric damage, nothing in the way"
    );
    assert_eq!(health_of(&world, victim), Some(65), "and the health says so");
}

#[test]
fn a_vest_stops_five_of_it() {
    let (mut world, victim) = world_with_victim((1, 0));
    world.spawn((
        WornBy(victim),
        Garment {
            layers: vec![(
                "torso".to_owned(),
                cdda_content::armour::Layer {
                    resist: 5.0,
                    thickness: 1.0,
                    cover: 100,
                },
            )],
        },
    ));
    dress(&mut world);

    let cast = cast_attack(&mut world, (0, 0, 0), &taser(), 0, victim, 0.0);

    assert_eq!(
        cast,
        Cast::Hit {
            part: "torso".to_owned(),
            dealt: 10
        },
        "five points of armour off fifteen"
    );
    assert_eq!(health_of(&world, victim), Some(70));
}

#[test]
fn a_taser_further_than_its_range_does_nothing() {
    let (mut world, victim) = world_with_victim((3, 0));
    let cast = cast_attack(&mut world, (0, 0, 0), &taser(), 0, victim, 0.0);
    assert_eq!(cast, Cast::OutOfRange, "one square of reach is one square");
    assert_eq!(health_of(&world, victim), Some(80), "and it is untouched");
}

#[test]
fn a_spell_that_is_not_an_attack_is_not_cast_by_this_rule() {
    let (mut world, victim) = world_with_victim((1, 0));
    let mut summon = taser();
    summon.effect_name = "summon".to_owned();
    assert_eq!(
        cast_attack(&mut world, (0, 0, 0), &summon, 0, victim, 0.0),
        Cast::NotAnAttack,
        "summoning is a different effect and a different rule"
    );
    assert_eq!(health_of(&world, victim), Some(80));
}

#[test]
fn a_spell_whose_damage_is_an_expression_does_nothing_rather_than_guessing() {
    let (mut world, victim) = world_with_victim((1, 0));
    let mut dice = taser();
    dice.damage = Scaling {
        start: None,
        increment: None,
        end: None,
    };
    assert_eq!(
        cast_attack(&mut world, (0, 0, 0), &dice, 0, victim, 0.0),
        Cast::NothingToHit,
        "the loader refused to read a number, so the cast has none to use"
    );
    assert_eq!(health_of(&world, victim), Some(80), "and nothing happened");
}

#[test]
fn the_lever_kills_a_monster_as_it_would_a_person() {
    // The spell does not care what it hits: a monster with a body and health takes the same damage.
    let mut world = World::new();
    let monster = spawn_monster(&mut world, (1, 0), cdda_sim::MonsterKindId(0), 15);
    world.entity_mut(monster).insert(torso_only());

    let cast = cast_attack(&mut world, (0, 0, 0), &taser(), 0, monster, 0.0);

    assert_eq!(
        cast,
        Cast::Hit {
            part: "torso".to_owned(),
            dealt: 15
        }
    );
    assert_eq!(health_of(&world, monster), Some(0), "and it is dead");
}
