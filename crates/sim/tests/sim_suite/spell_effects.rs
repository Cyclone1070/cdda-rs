//! What a spell does when it lands.
//!
//! Reference: `magic_spell_effect.cpp`, which has one function per `effect_name`. The port has one of them so far, and
//! the count is the gate: a table of effects has to say how long it is, or "we ported some effects" stops meaning
//! anything.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_content::spells::Spell;
use cdda_sim::spell_effects::{self, Effective};

/// A scaling that does not change with level, which is what most of the test's numbers are.
fn scaling(value: i64) -> cdda_content::spells::Scaling {
    #[allow(clippy::cast_precision_loss)]
    let as_float = value as f64;
    cdda_content::spells::Scaling {
        start: Some(as_float),
        increment: Some(0.0),
        end: Some(as_float),
    }
}

fn a_spell(effect_name: &str, damage: i64, area: i64) -> Spell {
    Spell {
        id: "test_spell".to_owned(),
        name: "Test Spell".to_owned(),
        effect_name: effect_name.to_owned(),
        effect_str: String::new(),
        damage_type: "pure".to_owned(),
        shape: "blast".to_owned(),
        energy_source: String::new(),
        damage: scaling(damage),
        range: scaling(3),
        area: scaling(area),
        energy: scaling(0),
        casting_time: scaling(0),
        difficulty: Some(0.0),
        max_level: Some(10.0),
    }
}

#[test]
fn a_spell_that_changes_pace_changes_it_for_everyone_in_reach() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let near = cdda_sim::creature::spawn_creature(&mut world, "near", (1, 1, 0));
    let far = cdda_sim::creature::spawn_creature(&mut world, "far", (9, 9, 0));
    let spell = a_spell("mod_moves", 300, 2);

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, near),
        300,
        "a creature two squares away is inside a blast of two"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, far),
        0,
        "and one at (9,9) is not"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, caster),
        0,
        "the caster is not its own target, as in the reference"
    );
}

#[test]
fn a_slow_a_soul_spell_takes_moves_away() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let victim = cdda_sim::creature::spawn_creature(&mut world, "victim", (0, 0, 0));
    cdda_sim::creature::give_moves(&mut world, victim, 500);
    let spell = a_spell("mod_moves", -200, 0);

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::Done
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, victim),
        300,
        "two hundred moves taken from five hundred"
    );
}

#[test]
fn an_effect_the_port_does_not_know_spends_nothing_and_says_so() {
    let mut world = World::new();
    let caster = cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    let other = cdda_sim::creature::spawn_creature(&mut world, "other", (0, 0, 0));
    let spell = a_spell("summon", 5, 0);

    assert_eq!(
        spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        Effective::NotPorted,
        "`summon` is in the data and not in the port, and it says so rather than pretending"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, other),
        0,
        "and nothing in the world changed"
    );
}

/// How much of the game's spell effects the port can perform, counted against the data.
///
/// Reference: `magic_spell_effect.cpp` has one function per `effect_name`, and every spell names one. A table of ported
/// effects without a count is a claim; with a count it is a fact that can only change deliberately.
#[test]
fn the_port_can_do_one_effect_of_the_ones_the_game_names() {
    use std::collections::BTreeSet;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(path).expect("the spell dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let spells = cdda_content::spells::load_dump(&value).expect("the dump loads");
    assert!(spells.len() > 200, "the game has {} spells", spells.len());

    let named: BTreeSet<&str> = spells
        .iter()
        .map(|spell| spell.effect_name.as_str())
        .collect();
    assert!(
        named.len() > 30,
        "the game names {} different effects",
        named.len()
    );
    for (name, _) in spell_effects::EFFECTS {
        assert!(
            named.contains(name),
            "the port claims `{name}`, which no spell in the data names"
        );
    }
    assert_eq!(
        spell_effects::EFFECTS.len(),
        1,
        "one effect is ported, out of the {} the game names",
        named.len()
    );
}
