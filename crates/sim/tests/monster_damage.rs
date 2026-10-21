//! What a blow does to a creature.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::{Monster, MonsterKindId, melee_blow, spawn_monster, strike, through_armour};

#[test]
fn a_blow_is_its_dice_plus_its_bonus() {
    let mut rolls = Rolls::new(7);
    // Three four-sided dice, no bonus: minimum three, maximum twelve.
    let blow = melee_blow(&mut rolls, 3, 4, 0);
    assert!((3..=12).contains(&blow), "three to twelve: {blow}");
}

#[test]
fn the_same_seed_gives_the_same_blow() {
    let mut first = Rolls::new(99);
    let mut second = Rolls::new(99);
    assert_eq!(
        melee_blow(&mut first, 2, 6, 3),
        melee_blow(&mut second, 2, 6, 3)
    );
}

#[test]
fn a_kind_without_dice_still_hits_for_its_bonus() {
    let mut rolls = Rolls::new(1);
    assert_eq!(
        melee_blow(&mut rolls, 0, 0, 4),
        4,
        "no dice, just the bonus"
    );
    assert_eq!(
        melee_blow(&mut rolls, 0, 0, -3),
        0,
        "and a negative bonus does nothing"
    );
}

#[test]
fn dice_mean_a_minimum_of_one_per_die() {
    let mut rolls = Rolls::new(5);
    for _ in 0..20 {
        let blow = melee_blow(&mut rolls, 4, 2, 0);
        assert!((4..=8).contains(&blow), "four to eight: {blow}");
    }
}

#[test]
fn armour_takes_what_it_can_and_never_heals() {
    assert_eq!(through_armour(10, 3), 7);
    assert_eq!(
        through_armour(3, 10),
        0,
        "a blow smaller than the armour does nothing"
    );
    assert_eq!(through_armour(5, 5), 0, "exactly enough armour stops it");
}

#[test]
fn a_struck_creature_loses_health_and_dies_at_zero() {
    let mut world = World::new();
    let monster = spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    assert!(!strike(&mut world, monster, 4, 0), "six left");
    assert_eq!(world.get::<Monster>(monster).expect("a monster").hp, 6);
    assert!(!strike(&mut world, monster, 4, 2), "two through: four left");
    assert_eq!(world.get::<Monster>(monster).expect("a monster").hp, 4);
    assert!(strike(&mut world, monster, 10, 0), "and this one kills it");
    assert!(world.get::<Monster>(monster).expect("a monster").hp <= 0);
}
