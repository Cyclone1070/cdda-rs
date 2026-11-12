//! A spell that books something for later: `timed_event`.
//!
//! Reference: `spell_effect::timed_event` (`magic_spell_effect.cpp:1281-1304`). The name in `effect_str` has to match the reference's own table **letter for letter**, and the
//! spell's duration says how many turns ahead the event lands - a turn being a second, which is what makes the dump's 18,000 an eighteen-thousand-turn wait.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn a_spell(effect_str: &str, seconds: f64) -> cdda_content::spells::Spell {
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "timed_event".to_owned(),
        effect_str: effect_str.to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: cdda_content::spells::Scaling::default(),
        range: cdda_content::spells::Scaling::default(),
        area: cdda_content::spells::Scaling::default(),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling {
            start: Some(seconds),
            increment: Some(0.0),
            end: Some(seconds),
        },
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
fn a_timed_event_spell_books_its_event_for_its_own_duration() {
    let mut world = World::new();
    cdda_sim::calendar::set_turn(&mut world, 0);
    let caster = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    // `AEA_LIGHT` is the data's own: `artifact_light` for eighteen thousand seconds.
    let spell = a_spell("artifact_light", 18_000.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell scheduled its event"
    );
    assert_eq!(
        cdda_sim::timed_events::has_happened(&world, "artifact_light"),
        Some(false),
        "**and it has not happened yet, because it was booked for later**"
    );
    cdda_sim::calendar::set_turn(&mut world, 17_999);
    assert_eq!(
        cdda_sim::timed_events::has_happened(&world, "artifact_light"),
        Some(false),
        "still not, one turn early"
    );
    cdda_sim::calendar::set_turn(&mut world, 18_000);
    assert_eq!(
        cdda_sim::timed_events::has_happened(&world, "artifact_light"),
        Some(true),
        "and it has at exactly the turn the duration named"
    );
}

/// **The name is matched letter for letter**, and the data gets one of them wrong.
///
/// Reference: the table at `magic_spell_effect.cpp:1283-1293` is lowercase, and its lookup is a `std::map` find, so `death_triffid_heart`'s `ROOTS_DIE` misses and the reference
/// schedules its `NONE` sentinel - an event nothing ever asks about. The port schedules nothing at all, which is the same observable behaviour, and this gate is what stops a
/// later round from "helpfully" lowercasing the name and changing what the content does.
#[test]
fn an_event_name_the_reference_does_not_know_books_nothing() {
    let mut world = World::new();
    cdda_sim::calendar::set_turn(&mut world, 0);
    let caster = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    let shouted = a_spell("ROOTS_DIE", 60_000.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &shouted, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "the spell books nothing, because the reference's table does not have that spelling"
    );
    cdda_sim::calendar::set_turn(&mut world, 60_000);
    assert!(
        !matches!(
            cdda_sim::timed_events::has_happened(&world, "roots_die"),
            Some(true)
        ),
        "**and the lowercase event the data meant is never due**, whether the store answers \"not yet\" or has nothing to answer with"
    );
    cdda_sim::calendar::set_turn(&mut world, 0);
    let spelled_right = a_spell("roots_die", 60_000.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spelled_right, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "while the spelling the table has does schedule it"
    );
    cdda_sim::calendar::set_turn(&mut world, 60_000);
    assert_eq!(
        cdda_sim::timed_events::has_happened(&world, "roots_die"),
        Some(true),
        "and it is due at the turn it was booked for"
    );
}
