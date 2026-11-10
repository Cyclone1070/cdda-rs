//! A spell that moves a character's pools: `recover_energy`.
//!
//! Reference: `spell_effect::recover_energy` (`magic_spell_effect.cpp:1241-1279`). The spell's damage is the amount and its `effect_str` names the pool, and **a negative
//! amount spends rather than restores** - four of the content's six spells are drains. The sign is also backwards for pain, which is the reference's own comment and not a
//! slip here.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing, clippy::float_cmp)]

use bevy_ecs::prelude::*;

fn a_spell(pool: &str, amount: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "recover_energy".to_owned(),
        effect_str: pool.to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(amount),
        range: cdda_content::spells::Scaling::default(),
        area: cdda_content::spells::Scaling::default(),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: std::collections::BTreeSet::new(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let person = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(person)
        .insert(cdda_sim::creature::Stamina { cur: 5_000.0, max: 9_000.0 });
    world.entity_mut(person).insert(cdda_sim::creature::Pain(0));
    world
        .entity_mut(person)
        .insert(cdda_sim::creature::Mana { cur: 0, max: 100 });
    world
        .entity_mut(person)
        .insert(cdda_sim::creature::BionicPower { joules: 0 });
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, person)
}

#[test]
fn a_drain_spell_takes_the_pool_its_effect_str_names() {
    let (mut world, person) = a_world();
    // `AEA_STAMINA_EMPTY` is the data's own: STAMINA of -1500.
    let spell = a_spell("STAMINA", -1_500.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell landed"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Stamina>(person).map(|held| held.cur),
        Some(3_500.0),
        "**a negative amount spends**, so five thousand less fifteen hundred is thirty-five hundred"
    );
}

/// **Pain is backwards**, and pain never goes below zero.
///
/// Reference: `:1266-1272`: `PAIN` calls `mod_pain( -healing )`, so a spell whose damage is -5 **adds** five pain. The floor at zero is `Creature::set_pain`'s own
/// (`creature.cpp:2191`), which is why a painkiller with nothing left to take away leaves a character at zero rather than negative.
#[test]
fn pain_is_backwards_and_never_goes_below_zero() {
    let (mut world, person) = a_world();
    let agony = a_spell("PAIN", -5.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &agony, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell landed"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Pain>(person).map(|held| held.0),
        Some(5),
        "**minus five damage is five pain**, because the reference negates it"
    );
    let relief = a_spell("PAIN", 50.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &relief, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "and a positive amount is a painkiller"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Pain>(person).map(|held| held.0),
        Some(0),
        "which floors at zero rather than going negative"
    );
}

#[test]
fn mana_fills_to_its_ceiling_and_a_monster_has_no_pool_to_move() {
    let (mut world, person) = a_world();
    let refill = a_spell("MANA", 1_000.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &refill, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell landed"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Mana>(person).map(|held| (held.cur, held.max)),
        Some((100, 100)),
        "**a thousand mana into a hundred-point pool fills it exactly**"
    );

    let monster = cdda_sim::spawn_monster(&mut world, (5, 5), cdda_sim::MonsterKindId(0), 30);
    cdda_sim::PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &refill, 0, (5, 5, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**and a monster has no mana to give**"
    );
    assert!(
        world.get::<cdda_sim::creature::Mana>(monster).is_none(),
        "nothing was added to it"
    );
}
