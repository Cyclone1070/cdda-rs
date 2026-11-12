//! A monster that makes people feel guilty for killing its kind.
//!
//! Reference: `spell_effect::guilt` (`magic_spell_effect.cpp:1617-1711`). Three facts carry it and each is a gate below: **only a monster can cast it**, the penalty **fades as the
//! tally grows** and stops altogether at the spell's damage, and a character with no conscience (a psychopath) feels nothing.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing, clippy::float_cmp)]

use bevy_ecs::prelude::*;

/// A monster kind, a world with one person on the spell's square, and that person's kill tally set.
fn a_world(kills: i32) -> (World, Entity, Entity) {
    let mut world = World::new();
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_blob");
    let kind = match kind {
        Some(kind) => kind,
        None => {
            cdda_sim::monster_registry::set_monster_types(
                &mut world,
                &[cdda_sim::monster_registry::MonsterKind { id: "mon_blob", hp: 40 }],
            );
            cdda_sim::monster_registry::id_of(&mut world, "mon_blob").expect("mon_blob")
        }
    };
    let person = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    let caster = cdda_sim::spawn_monster(&mut world, (1, 0), kind, 40);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // The tally is written the way the death path writes it: one kill at a time.
    for _ in 0..kills {
        cdda_sim::kills::record(&mut world, kind);
    }
    (world, person, caster)
}

fn a_spell(max_kills: f64, seconds: f64, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "guilt".to_owned(),
        effect_str: "BLOB".to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(max_kills),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(area),
        energy: cdda_content::spells::Scaling::default(),
        duration: scaling(seconds),
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
fn a_guilt_spell_bites_harder_the_fewer_of_its_kind_you_have_killed() {
    let (mut world, fresh, caster) = a_world(0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // The data's own: `death_guilt` states a hundred kills and eighteen thousand seconds.
    let spell = a_spell(100.0, 18_000.0, 2.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 1, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the caster is a monster and the person is inside the blast"
    );
    let point = world
        .get::<cdda_sim::morale::Morale>(fresh)
        .and_then(|felt| felt.0.first().cloned())
        .expect("the morale point was added");
    assert_eq!(point.kind, "morale_killed_monster", "of the reference's own type");
    assert_eq!(
        point.bonus, -5,
        "**nothing killed yet is a level-one malus of five**: `-5 * guilt_mult * ( 1 - 0 )`"
    );
    assert_eq!(point.max_bonus, -250, "capped at the reference's own -250");
    assert_eq!(point.duration, 18_000, "for the spell's whole duration");

    // Half the way to not caring: half the bite, half the time.
    let (mut world, half, caster) = a_world(50);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 1, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "still felt"
    );
    let point = world
        .get::<cdda_sim::morale::Morale>(half)
        .and_then(|felt| felt.0.first().cloned())
        .expect("the morale point was added");
    assert!(
        point.bonus > -5 && point.bonus < 0,
        "**fifty of a hundred kills halves the bite**: {} against the fresh character's -5",
        point.bonus
    );
    assert!(
        point.duration < 18_000,
        "and halves how long it lasts: {} against 18,000",
        point.duration
    );
}

/// **A character casting it feels nothing**, and a psychopath has no conscience to move.
///
/// Reference: `:1620-1623` returns unless the caster is a monster; `:1660-1664` returns for a psychopath or `PRED3`/`PRED4`. And at `kill_count >= max_kills` the reference stops
/// caring altogether (`:1666-1675`).
#[test]
fn a_character_cannot_cast_it_a_psychopath_ignores_it_and_a_full_tally_stops_it() {
    let (mut world, person, monster) = a_world(0);
    let spell = a_spell(10.0, 1_000.0, 2.0);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &spell, 1, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**a character casting this feels nothing at all**"
    );

    let (mut world, psychopath, caster) = a_world(0);
    cdda_sim::traits::add_trait(&mut world, psychopath, "PSYCHOPATH");
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 1, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "a psychopath has no conscience to move"
    );

    let (mut world, _numb, caster) = a_world(10);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 1, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**ten kills against a limit of ten is where a character stops caring**"
    );
    let _ = monster;
}
