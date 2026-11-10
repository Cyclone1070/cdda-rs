//! Casting an attack spell at a creature.
//!
//! Reference: `spell::cast` -> `spell_effect::target_attack` -> `damage_target` (`magic.cpp`, `spell_effect.cpp`).
//! The rules the port keeps are the ones a player would notice: the spell reaches as far as its level says, it deals
//! its level-scaled damage, the muzzle lands on one part of a body, and the armour on that part meets it - a vest
//! stops a taser for the same reason it stops a fist.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use bevy_ecs::prelude::*;
use cdda_content::spells::Scaling;
use cdda_sim::creature::{Body, Garment, WornBy, dress, health_of, set_health, spawn_creature};
use cdda_sim::hit_location::BodyPart;
use cdda_sim::spellcasting::{Cast, cast_at, cast_attack, lands_on};
use cdda_sim::spawn_monster;

fn torso_only() -> Body {
    Body {
        parts: vec![BodyPart {
            name: "torso".to_owned(),
            hit_size: 36.0,
            hit_difficulty: 1.0,
            upper: false,
            lower: false,
                ..Default::default()
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
        duration: Scaling::default(),
        casting_time: Scaling::default(),
        difficulty: None,
        max_level: None,
        flags: std::collections::BTreeSet::new(),
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
                    // **The layer names the type the spell deals**, because a garment now protects by type: the taser is electric, so a layer that resists bash would stop nothing.
                    damage_type: "electric".to_owned(),
                    resist: 5.0,
                    thickness: 1.0,
                    cover: 100,
                },
            )],
            vitals: Vec::new(),
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

/// Where a spell lands is a draw over the parts' hit sizes, and the draw comes from the store.
///
/// Reference: `anatomy::random_body_part` (`anatomy.cpp:203-206`) - `rng_float( 0, size_sum )` and the first part
/// whose running total reaches it. It is **not** the melee rule: a struck part is tilted by how well the blow
/// landed, while a spell lands somewhere on the body and a big part is simply likelier.
#[test]
fn a_spell_lands_where_the_hit_sizes_say_and_not_where_the_melee_rule_would() {
    let torso = BodyPart {
        name: "torso".to_owned(),
        hit_size: 90.0,
        hit_difficulty: 1.0,
        upper: false,
        lower: false,
                ..Default::default()
            };
    let head = BodyPart {
        name: "head".to_owned(),
        hit_size: 10.0,
        hit_difficulty: 3.0,
        upper: true,
        lower: false,
                ..Default::default()
            };
    let parts = vec![torso, head];

    assert_eq!(lands_on(&parts, 0.0).as_deref(), Some("torso"), "the first draw is the first part");
    assert_eq!(
        lands_on(&parts, 0.89).as_deref(),
        Some("torso"),
        "ninety per cent of the weight is torso, so a draw at 0.89 is still torso"
    );
    assert_eq!(
        lands_on(&parts, 0.91).as_deref(),
        Some("head"),
        "and past ninety per cent it is the head"
    );
    assert_eq!(
        lands_on(&parts, 0.9999).as_deref(),
        Some("head"),
        "the last of the range is the last part, not nothing"
    );
}

#[test]
fn the_same_seed_lands_the_blow_in_the_same_place() {
    let cast_once = |seed: u32| {
        let (mut world, victim) = world_with_victim((1, 0));
        cdda_sim::singletons::put_rolls(
            &mut world,
            cdda_core_types::rng::Rolls::new(seed),
        );
        cast_at(&mut world, (0, 0, 0), &taser(), 0, victim)
    };
    assert_eq!(cast_once(7), cast_once(7), "a seed replays the same cast");
    // Two seeds may land the same way on a one-part body, so what is being proven here is the replay, not a
    // difference: a cast that drew from somewhere else would not answer the same twice.
    assert_eq!(cast_once(11), cast_once(11), "and so does another");
}

/// A vest that stops blow after blow eventually stops being a vest.
///
/// Reference: `Character::absorb_hit` (`character_armor.cpp`) asks every garment covering the struck part to survive
/// the blow, using the damage that **arrived** rather than the damage that got through. The distinction is the whole
/// test: the wearer is barely hurt in each of these casts, and the vest still comes apart.
#[test]
fn a_vest_that_stops_blow_after_blow_comes_apart_and_then_the_flesh_does() {
    let (mut world, victim) = world_with_victim((1, 0));
    let vest = world
        .spawn((
            WornBy(victim),
            Garment {
                layers: vec![(
                    "torso".to_owned(),
                    cdda_content::armour::Layer {
                        // **The layer names the type the spell deals**, because a garment now protects by type: the taser is electric, so a layer that resists bash would stop nothing.
                    damage_type: "electric".to_owned(),
                        resist: 5.0,
                        thickness: 1.0,
                        cover: 100,
                    },
                )],
                vitals: Vec::new(),
            },
        ))
        .id();
    dress(&mut world);
    set_health(&mut world, victim, 1_000);

    // The first four casts each damage the vest; the wearer takes ten apiece either way.
    for cast in 1..=4 {
        let landed = cast_attack(&mut world, (0, 0, 0), &taser(), 0, victim, 0.0);
        assert_eq!(
            landed,
            Cast::Hit {
                part: "torso".to_owned(),
                dealt: 10
            },
            "cast {cast} still meets five points of armour"
        );
    }
    assert_eq!(health_of(&world, victim), Some(960));

    // And the fifth ruins it.
    let landed = cast_attack(&mut world, (0, 0, 0), &taser(), 0, victim, 0.0);
    assert_eq!(
        landed,
        Cast::Hit {
            part: "torso".to_owned(),
            dealt: 10
        },
        "the fifth blow is still stopped - the vest is ruined *by* stopping it"
    );
    assert!(world.get_entity(vest).is_err(), "and the vest is gone");

    let landed = cast_attack(&mut world, (0, 0, 0), &taser(), 0, victim, 0.0);
    assert_eq!(
        landed,
        Cast::Hit {
            part: "torso".to_owned(),
            dealt: 15
        },
        "the next blow meets nothing at all"
    );
}

/// A spell that names an effect applies it, for the duration the spell states.
///
/// Reference: `damage_targets` (`magic_spell_effect.cpp:553-563`) calls `add_effect_to_target` for every valid target
/// before it deals damage, so an effect lands even on a spell whose damage is zero or negative. Twenty-seven of the
/// game's attack spells are in that shape - `AO_SLOW`, `AEA_CONFUSED`, `AEA_ADRENALINE` and the rest.
#[test]
fn a_spell_that_names_an_effect_puts_it_on_its_target() {
    use cdda_sim::applied_effects::{self, AppliedEffects};

    let mut world = World::new();
    // The effect type the store will look the spell's `effect_str` up in. It is built by loading a dump entry, which is
    // how a type reaches the store in the game - a literal here would be a second constructor the port does not have.
    let loader = cdda_json::parse(
        br#"{"id":"stunned","max_intensity":1,"max_duration":"100 turns"}"#,
    )
    .expect("valid json");
    let cdda_json::Value::Object(entry) = loader else {
        panic!("an effect type is an object");
    };
    let kind = cdda_content::effects::read_one("stunned", &entry);
    cdda_sim::effect_registry::set_effect_types(&mut world, &[kind]);
    let victim = spawn_creature(&mut world, "a zombie", (1, 0, 0));
    world.entity_mut(victim).insert(torso_only());
    set_health(&mut world, victim, 80);

    let mut spell = taser();
    spell.effect_str = "stunned".to_owned();
    spell.duration = Scaling {
        start: Some(40.0),
        increment: Some(0.0),
        end: Some(40.0),
    };
    spell.damage = Scaling {
        start: Some(0.0),
        increment: Some(0.0),
        end: Some(0.0),
    };

    let cast = cast_attack(&mut world, (0, 0, 0), &spell, 0, victim, 0.0);
    assert_eq!(
        cast,
        Cast::Hit {
            part: "torso".to_owned(),
            dealt: 0
        },
        "no damage, but the spell still landed"
    );
    let worn = world
        .get::<AppliedEffects>(victim)
        .expect("the effect is on the target");
    assert_eq!(worn.0.len(), 1);
    assert_eq!(worn.0[0].id, "stunned");
    assert_eq!(worn.0[0].duration, 40, "forty turns, as the spell states");
    assert!(applied_effects::has(&world, victim, "stunned"));

    // An effect the store does not know is not invented: the blow still lands, and nothing is worn.
    let other = spawn_creature(&mut world, "another", (1, 1, 0));
    world.entity_mut(other).insert(torso_only());
    set_health(&mut world, other, 80);
    let mut unknown = spell.clone();
    unknown.effect_str = "not_an_effect".to_owned();
    let _ = cast_attack(&mut world, (0, 0, 0), &unknown, 0, other, 0.0);
    assert!(!applied_effects::has(&world, other, "not_an_effect"));
}
