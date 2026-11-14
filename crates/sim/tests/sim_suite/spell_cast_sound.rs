//! What a cast is heard as.
//!
//! Reference: `damage_targets` (`magic_spell_effect.cpp:523-560`) makes the spell's sound **on each target square** before it applies the effect, and the loudness is
//! `spell::sound_volume` (`magic.cpp:1630-1640`) - so a silent attack is a silent hit, and a loud one is heard where it lands rather than where it was cast.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn a_world() -> (World, Entity, Entity) {
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
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    // A second character **with a body part to be hit on**, because `damage_one` asks the anatomy which part a blow lands on and a creature without one has nothing to hit.
    let victim = cdda_sim::spawn_player(&mut world, (3, 0), 80, 0);
    world.entity_mut(victim).insert(cdda_sim::creature::Body {
        parts: vec![cdda_sim::hit_location::BodyPart {
            name: "torso".to_owned(),
            hit_size: 36.0,
            hit_difficulty: 1.0,
            upper: false,
            lower: false,
            ..Default::default()
        }],
    });
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster, victim)
}

fn an_attack(flags: &[&str]) -> cdda_content::spells::Spell {
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
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
    }
}

fn heard(world: &mut World) -> Vec<(i32, i32, i32)> {
    cdda_sim::sounds::heard_this_turn(world)
        .into_iter()
        .map(|sound| (sound.position.0, sound.position.1, sound.position.2))
        .collect()
}

#[test]
fn a_hit_is_heard_where_it_lands_with_a_third_of_its_damage() {
    let (mut world, caster, victim) = a_world();
    let spell = an_attack(&[]);
    assert_eq!(
        cdda_sim::spellcasting::cast_attack(&mut world, (0, 0, 0), &spell, 0, victim, 0.5),
        cdda_sim::spellcasting::Cast::Hit {
            part: "torso".to_owned(),
            dealt: 9
        },
        "the hit landed"
    );
    assert_eq!(
        heard(&mut world),
        vec![(3, 0, 0)],
        "**the sound is made at the victim's square**, not the caster's"
    );
    let volume: i32 = cdda_sim::sounds::heard_this_turn(&mut world)
        .iter()
        .map(|sound| sound.volume)
        .sum();
    assert_eq!(volume, 3, "**and nine damage is three of sound**");
    let _ = caster;
}

/// **A `SILENT` attack is a silent hit; a `LOUD` one is heard twice as far.**
#[test]
fn silence_and_noise_are_the_spells_flags() {
    let (mut world, _caster, victim) = a_world();
    assert!(
        matches!(
            cdda_sim::spellcasting::cast_attack(
                &mut world,
                (0, 0, 0),
                &an_attack(&["SILENT"]),
                0,
                victim,
                0.5
            ),
            cdda_sim::spellcasting::Cast::Hit { .. }
        ),
        "the silent cast still hit"
    );
    assert!(
        heard(&mut world).is_empty(),
        "**and nothing heard it at all**"
    );

    let (mut world, _caster, victim) = a_world();
    assert!(
        matches!(
            cdda_sim::spellcasting::cast_attack(
                &mut world,
                (0, 0, 0),
                &an_attack(&["LOUD"]),
                0,
                victim,
                0.5
            ),
            cdda_sim::spellcasting::Cast::Hit { .. }
        ),
        "the loud cast hit too"
    );
    let volume: i32 = cdda_sim::sounds::heard_this_turn(&mut world)
        .iter()
        .map(|sound| sound.volume)
        .sum();
    assert_eq!(
        volume, 7,
        "**`LOUD` is three of sound plus four**, which is `1 + damage / 3` on top of `damage / 3`"
    );
}
