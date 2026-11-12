//! Which things a spell may be aimed at, as a rule rather than a data question.
//!
//! Reference: `spell::is_valid_target( caster, p )` (`magic.cpp:1697-1719`) and the three filters under it (`:1721-1764`). The classes come from the dump; this gate is about what the
//! rule does with them, using a spell built for each case.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use std::collections::BTreeSet;

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
        &[
            cdda_sim::monster_registry::MonsterKind {
                id: "mon_zombie",
                hp: 40,
            },
            cdda_sim::monster_registry::MonsterKind {
                id: "mon_dog",
                hp: 25,
            },
        ],
    );
    cdda_sim::monster_registry::set_flags_and_species(
        &mut world,
        &[
            (cdda_sim::MonsterKindId(1), BTreeSet::new(), ["ZOMBIE".to_owned()].into_iter().collect()),
            (cdda_sim::MonsterKindId(2), BTreeSet::new(), ["MAMMAL".to_owned()].into_iter().collect()),
        ],
    );
    // A body for the zombie kind, because a blow needs a part to land on and the cast gate at the end of this file hits one.
    cdda_sim::monster_registry::set_bodies(
        &mut world,
        &[(
            cdda_sim::MonsterKindId(1),
            cdda_sim::creature::Body {
                parts: vec![cdda_sim::hit_location::BodyPart {
                    name: "torso".to_owned(),
                    hit_size: 36.0,
                    hit_difficulty: 1.0,
                    upper: false,
                    lower: false,
                    ..Default::default()
                }],
            },
        )],
    );
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    let zombie = cdda_sim::spawn_monster(&mut world, (3, 0), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, caster, zombie)
}

fn a_spell(targets: &[&str], monsters: &[&str], species: &[&str], ignored: &[&str]) -> cdda_content::spells::Spell {
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
        damage: scaling(1.0),
        range: scaling(9.0),
        area: scaling(0.0),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: targets.iter().map(|one| (*one).to_owned()).collect(),
        targeted_monster_ids: monsters.iter().map(|one| (*one).to_owned()).collect(),
        targeted_species_ids: species.iter().map(|one| (*one).to_owned()).collect(),
        ignored_species_ids: ignored.iter().map(|one| (*one).to_owned()).collect(),
        flags: BTreeSet::new(),
    }
}

/// **A creature, its own caster, and a bare square are three different questions.**
#[test]
fn a_spell_may_be_aimed_at_a_creature_or_at_the_ground() {
    use cdda_sim::spell_targets::allows;
    let (mut world, caster, zombie) = a_world();
    assert!(
        allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &[], &[], &[]), (3, 0, 0)),
        "**a spell that names hostile creatures may be aimed at a monster**"
    );
    assert!(
        !allows(&mut world, (0, 0, 0), &a_spell(&["ground"], &[], &[], &[]), (3, 0, 0)),
        "**and one that names only the ground may not**, however empty the square would be"
    );
    assert!(
        allows(&mut world, (0, 0, 0), &a_spell(&["ground"], &[], &[], &[]), (5, 5, 0)),
        "while a square with nothing on it is exactly what `ground` means"
    );
    assert!(
        allows(&mut world, (0, 0, 0), &a_spell(&["self"], &[], &[], &[]), (0, 0, 0)),
        "**`self` is the caster's own square**"
    );
    assert!(
        !allows(&mut world, (0, 0, 0), &a_spell(&["self"], &[], &[], &[]), (3, 0, 0)),
        "and not somebody else's"
    );
    let _ = zombie;
}

/// **An ally is not a hostile, and the caster's own square is not an ally's.**
#[test]
fn a_charmed_monster_is_an_ally_and_never_the_self() {
    use cdda_sim::spell_targets::allows;
    let (mut world, caster, zombie) = a_world();
    cdda_sim::friendly::make_friendly(&mut world, zombie, 10);
    assert!(
        !allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &[], &[], &[]), (3, 0, 0)),
        "**a charmed monster is nobody's hostile target**"
    );
    assert!(
        allows(&mut world, (0, 0, 0), &a_spell(&["ally"], &[], &[], &[]), (3, 0, 0)),
        "it is an ally's"
    );
    assert!(
        !allows(&mut world, (0, 0, 0), &a_spell(&["ally"], &[], &[], &[]), (0, 0, 0)),
        "**and the caster's own square is not an ally**, which the reference says with `p != caster.pos()`"
    );
}

/// **The filters narrow, and a filter that names nothing accepts everything.**
#[test]
fn a_named_monster_and_a_named_species_narrow_the_rule() {
    use cdda_sim::spell_targets::allows;
    let (mut world, caster, _zombie) = a_world();
    assert!(
        allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &["mon_zombie"], &[], &[]), (3, 0, 0)),
        "a spell naming the zombie may hit the zombie"
    );
    assert!(
        !allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &["mon_dog"], &[], &[]), (3, 0, 0)),
        "**and one naming the dog may not**, even though the square holds a hostile creature"
    );
    assert!(
        allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &[], &["ZOMBIE"], &[]), (3, 0, 0)),
        "a species the creature is in is accepted"
    );
    assert!(
        !allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &[], &["MAMMAL"], &[]), (3, 0, 0)),
        "and one it is not in is refused"
    );
    assert!(
        !allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &[], &[], &["ZOMBIE"]), (3, 0, 0)),
        "**an ignored species is left alone**"
    );
    assert!(
        allows(&mut world, (0, 0, 0), &a_spell(&["hostile"], &[], &[], &["MAMMAL"]), (3, 0, 0)),
        "while some other ignored species leaves it a target"
    );
}

/// **A cast that the spell may not aim at does nothing**, which is what makes the rule a rule rather than a table.
#[test]
fn a_cast_at_a_creature_the_spell_does_not_want_does_nothing() {
    let (mut world, caster, zombie) = a_world();
    assert_eq!(
        cdda_sim::spellcasting::cast_attack(
            &mut world,
            (0, 0, 0),
            &a_spell(&["ground"], &[], &[], &[]),
            0,
            zombie,
            0.5
        ),
        cdda_sim::spellcasting::Cast::NothingToHit,
        "**a spell that may only be aimed at the ground finds nothing to hit**, however close the monster is"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, zombie),
        Some(40),
        "and the monster is untouched"
    );
    assert_eq!(
        cdda_sim::spellcasting::cast_attack(
            &mut world,
            (0, 0, 0),
            &a_spell(&["hostile"], &["mon_dog"], &[], &[]),
            0,
            zombie,
            0.5
        ),
        cdda_sim::spellcasting::Cast::NothingToHit,
        "**and one that names the dog passes over the zombie**"
    );
    assert!(
        matches!(
            cdda_sim::spellcasting::cast_attack(
                &mut world,
                (0, 0, 0),
                &a_spell(&["hostile"], &["mon_zombie"], &[], &[]),
                0,
                zombie,
                0.5
            ),
            cdda_sim::spellcasting::Cast::Hit { .. }
        ),
        "while the same spell naming the zombie lands"
    );
    let _ = caster;
}
