//! How hard a creature is to hit, before any damage is dealt.
//!
//! Source: three small functions the reference composes into one number.
//!
//! * `monster::dodge_roll` (`monster.cpp:2717`) is `get_dodge() * 5`;
//! * `Creature::size_melee_penalty` (`creature.cpp:836`) is 30, 15, 0, -10, -20 from tiny to huge;
//! * `Creature::deal_melee_attack` (`creature.cpp:903-929`) subtracts both from the attacker's roll and
//!   adds 40 when the target cannot move at all - "even a clumsy person would not miss a turret".
//!
//! A spread of zero or less is a miss: the reference's own test for the dodge event is
//! `hit_spread <= 0`, so zero is not a graze.

#![allow(clippy::panic)]

#![allow(clippy::float_cmp)]

use cdda_sim::CreatureSize;
use cdda_sim::melee::{dodge_roll, hit_spread, is_a_hit, size_melee_penalty};

#[test]
fn a_dodge_roll_is_five_per_point_of_dodge() {
    assert_eq!(dodge_roll(0), 0.0, "no dodge, no roll");
    assert_eq!(dodge_roll(1), 5.0);
    assert_eq!(dodge_roll(3), 15.0);
    assert_eq!(dodge_roll(8), 40.0, "which is exactly why eight dodge stops a clumsy attack");
}

#[test]
fn a_bigger_target_is_easier_to_hit() {
    assert_eq!(size_melee_penalty(CreatureSize::Tiny), 30, "a cat is hard to hit");
    assert_eq!(size_melee_penalty(CreatureSize::Small), 15);
    assert_eq!(size_melee_penalty(CreatureSize::Medium), 0, "a person is the baseline");
    assert_eq!(size_melee_penalty(CreatureSize::Large), -10, "a cow is easier");
    assert_eq!(size_melee_penalty(CreatureSize::Huge), -20);
}

#[test]
fn the_spread_is_the_roll_less_what_the_target_avoids() {
    // A medium target with no dodge: the roll is the spread.
    assert_eq!(hit_spread(50, 0.0, CreatureSize::Medium, false), 50);
    // Eight dodge takes forty off.
    assert_eq!(hit_spread(50, dodge_roll(8), CreatureSize::Medium, false), 10);
    // A large target adds ten back.
    assert_eq!(hit_spread(5, 0.0, CreatureSize::Large, false), 15);
    // And a turret that cannot move is all but unmissable.
    assert_eq!(hit_spread(-35, 0.0, CreatureSize::Medium, true), 5);
    assert!(
        is_a_hit(hit_spread(-35, 0.0, CreatureSize::Medium, true)),
        "the forty for being immobile turns a certain miss into a hit"
    );
}

#[test]
fn zero_is_a_miss_not_a_graze() {
    assert!(!is_a_hit(0), "the reference dodges on `hit_spread <= 0`");
    assert!(!is_a_hit(-1));
    assert!(is_a_hit(1), "one point over is a hit");
}
