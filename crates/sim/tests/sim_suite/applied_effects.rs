//! The effects a creature is wearing, on the creature.
//!
//! Reference: `Creature::effects` with `add_effect` and `remove_effect` (`creature.cpp:1731-1856`, `:1985-2010`). Before
//! this, `effects.rs` knew what repeated application does to an effect's numbers and there was nowhere to keep the
//! answer - the rule existed and nothing wore it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::applied_effects::{self, AppliedEffects};
use cdda_sim::effects::ActiveEffect;

fn an_effect(id: &str, duration: i64) -> ActiveEffect {
    ActiveEffect {
        id: id.to_owned(),
        intensity: 1,
        duration,
        permanent: false,
    }
}

#[test]
fn an_effect_of_the_same_kind_replaces_rather_than_piles_up() {
    let mut world = World::new();
    let creature = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    applied_effects::add(&mut world, creature, an_effect("effect_vomit", 6));
    applied_effects::add(&mut world, creature, an_effect("effect_vomit", 9));

    let worn = world.get::<AppliedEffects>(creature).expect("the creature wears it");
    assert_eq!(worn.0.len(), 1, "one effect of a kind, not two");
    assert_eq!(worn.0[0].duration, 9, "the newer one is what is worn");
    assert!(applied_effects::has(&world, creature, "effect_vomit"));
    assert!(!applied_effects::has(&world, creature, "effect_bleed"));
}

#[test]
fn a_turn_takes_a_turn_off_every_effect_and_removes_the_ones_that_run_out() {
    let mut world = World::new();
    let creature = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    applied_effects::add(&mut world, creature, an_effect("effect_vomit", 2));
    applied_effects::add(&mut world, creature, an_effect("effect_bleed", 1));
    let mut forever = an_effect("effect_permanent_thing", 5);
    forever.permanent = true;
    applied_effects::add(&mut world, creature, forever);

    assert_eq!(
        applied_effects::spend_a_turn(&mut world, creature),
        1,
        "the one-turn bleeding ended on the first turn"
    );
    assert_eq!(
        applied_effects::spend_a_turn(&mut world, creature),
        1,
        "and the two-turn vomit on the second"
    );
    assert!(applied_effects::has(&world, creature, "effect_permanent_thing"));
    assert_eq!(
        applied_effects::intensity_of(&world, creature, "effect_permanent_thing"),
        Some(1),
        "a permanent effect still says how strong it is"
    );
    assert_eq!(
        applied_effects::spend_a_turn(&mut world, creature),
        0,
        "a permanent effect never comes off"
    );
}

#[test]
fn taking_off_an_effect_answers_whether_there_was_one() {
    let mut world = World::new();
    let creature = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    applied_effects::add(&mut world, creature, an_effect("effect_vomit", 4));
    assert!(applied_effects::remove(&mut world, creature, "effect_vomit"));
    assert!(!applied_effects::remove(&mut world, creature, "effect_vomit"));
    assert!(world.get::<AppliedEffects>(creature).is_some_and(|worn| worn.0.is_empty()));
}
