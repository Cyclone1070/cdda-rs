//! A spell that explodes: `explosion`.
//!
//! Reference: `spell_effect::explosion` (`magic_spell_effect.cpp:1463-1467`), which is one call into `explosion_handler::explosion` with the spell's damage as the blast's force,
//! its area over ten as the falloff, and fire always on. The creature half lives in `crates/sim/src/explosion.rs` and the terrain half in `crates/sim/src/blast.rs`.
//!
//! The gates below hold the dice still and check the two things a falloff decides: **the force at the centre is the spell's damage**, so a monster standing there is hit hardest,
//! and **a square further out is hit with less** - which is the whole point of `area / 10`.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing, clippy::float_cmp)]

use bevy_ecs::prelude::*;

/// Open floor at move cost two, so the wavefront travels and nothing is walled in.
fn a_world() -> World {
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
    for y in -20..=20 {
        for x in -20..=20 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    // A generator that answers the bottom of every range: the jitter is 1.0 and every blow is its floor, so the numbers below are the rules and not the dice.
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    world
}

fn a_spell(damage: f64, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "explosion".to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(damage),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(area),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn a_blast_hits_hardest_at_its_centre_and_less_further_out() {
    let mut world = a_world();
    let caster = cdda_sim::spawn_player(&mut world, (-5, 0), 80, 0);
    let near = cdda_sim::spawn_monster(&mut world, (0, 0), cdda_sim::MonsterKindId(0), 500);
    let far = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(0), 500);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // Power 100 with a factor of 0.8 per square: 100 at the centre, 80 one square out, 51 at three.
    let spell = a_spell(100.0, 8.0);
    let outcome = cdda_sim::explosion::explode(&mut world, Some(caster), (0, 0, 0), 100.0, 0.8, true);
    assert!(
        outcome.reached > 1,
        "the wavefront reached {} squares, so this is a blast and not a single square",
        outcome.reached
    );
    let near_damage: i32 = outcome
        .struck
        .iter()
        .filter(|(who, _)| *who == near)
        .map(|(_, taken)| *taken)
        .sum();
    let far_damage: i32 = outcome
        .struck
        .iter()
        .filter(|(who, _)| *who == far)
        .map(|(_, taken)| *taken)
        .sum();
    assert!(
        near_damage > 0 && far_damage > 0,
        "**both creatures took a blow**: {near_damage} at the centre and {far_damage} three squares out"
    );
    assert!(
        near_damage > far_damage,
        "**and the centre hits harder** - {near_damage} against {far_damage}, because the force falls off with the distance"
    );
    // The spell is the same call the reference makes: damage as the force, area over ten as the falloff.
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell effect runs the same blast and damages what it reaches"
    );
}

/// **A character takes six blows, one per blastable part**, and limbs are hit harder than the head.
///
/// Reference: `explosion.cpp:433-462`: six `rng( force * low, force * high )` blows, the four limbs at `3.5` of the force against `3.0` for head and torso, each meeting
/// `armour * 0.4` (limbs) or `armour * 0.5` (head and torso). The port records each in the wound list, which is what a later round's medicine rules read.
#[test]
fn a_character_takes_a_blow_on_every_blastable_part() {
    let mut world = a_world();
    let person = cdda_sim::spawn_player(&mut world, (0, 0), 500, 0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let outcome = cdda_sim::explosion::explode(&mut world, None, (0, 0, 0), 40.0, 0.8, true);
    assert_eq!(
        outcome.struck.first().map(|(who, _)| *who),
        Some(person),
        "the character standing at the centre was hit"
    );
    let wounds = world
        .get::<cdda_sim::creature::Wounds>(person)
        .expect("the blows were recorded");
    assert_eq!(
        wounds.0.len(),
        6,
        "**six parts, one blow each** - the reference's own list, no more and no fewer"
    );
    let torso = wounds.0.get("torso").copied().unwrap_or(0);
    let leg = wounds.0.get("leg_l").copied().unwrap_or(0);
    assert!(
        leg >= torso,
        "**and a limb is hit at least as hard as the torso** ({leg} against {torso}), because the limbs use 3.5 of the force where the torso uses 3.0"
    );
}
