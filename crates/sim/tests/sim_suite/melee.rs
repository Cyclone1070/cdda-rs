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

use cdda_core_types::rng::Rolls;
use cdda_sim::CreatureSize;
use cdda_sim::melee::{
    Damage, after_armour, dice_total, dodge_roll, dodged, effective_resist, hit_spread, is_a_hit,
    monster_melee_damage, size_melee_penalty,
};

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

/// A spread of exactly zero is **both**: the target is told it dodged, and the attack still lands.
///
/// Reference: `Creature::deal_melee_attack` fires the dodge event on `hit_spread <= 0`
/// (`creature.cpp:924`), while `monster::melee_attack` deals damage on `hitspread >= 0`
/// (`monster.cpp:2151`). The two comparisons disagree at zero, and the reference ships that way. A port
/// that picked one rule for both would be tidier and wrong at exactly one point - the sort of tidy that
/// no test written from the outside would catch. Last round's test asserted zero was a miss; this is the
/// correction, and the reason is a line number on each side.
#[test]
fn zero_is_a_dodge_and_a_hit() {
    assert!(is_a_hit(0), "`hitspread >= 0` deals damage, so zero lands");
    assert!(dodged(0), "and `hit_spread <= 0` tells the target it dodged");
    assert!(!is_a_hit(-1));
    assert!(dodged(-1));
    assert!(is_a_hit(1), "one point over hits and is not a dodge");
    assert!(!dodged(1));
}

/// Armour takes off what it can and never more than the blow had.
///
/// Reference: `monster::absorb_hit` (`monster.cpp:2085`) - `elem.amount -= std::min( r.get_effective_resist(
/// elem ) + get_worn_armor_val( elem.type ), elem.amount )` - where `get_effective_resist` is
/// `std::max( type_resist( du.type ) - du.res_pen, 0 )` (`damage.cpp:724-727`).
///
/// Two separate facts hide in that line: penetration eats the resistance *before* the subtraction, and
/// the whole result is clamped by the damage, so armour can leave a blow at zero but never below it.
#[test]
fn armour_resists_less_whatever_the_attack_penetrates() {
    assert_eq!(effective_resist(5.0, 0.0), 5.0, "plain resistance");
    assert_eq!(effective_resist(5.0, 2.0), 3.0, "two points of penetration take two off");
    assert_eq!(effective_resist(5.0, 5.0), 0.0, "exactly through");
    assert_eq!(effective_resist(5.0, 9.0), 0.0, "and never negative");
}

#[test]
fn a_blow_can_be_stopped_but_never_reversed() {
    assert_eq!(after_armour(10.0, 5.0, 0.0, 0.0), 5.0, "half of it gets through");
    assert_eq!(after_armour(10.0, 5.0, 2.0, 0.0), 3.0, "worn armour adds to the resist");
    assert_eq!(after_armour(10.0, 5.0, 0.0, 3.0), 8.0, "penetration takes from the resist");
    assert_eq!(after_armour(4.0, 5.0, 2.0, 0.0), 0.0, "and armour thicker than the blow leaves nothing");
    assert_eq!(after_armour(10.0, 0.0, 0.0, 0.0), 10.0, "no armour, no reduction");
}

/// The damage a monster's claws do, before armour: its type's own damage plus dice.
///
/// Reference: `monster::melee_attack` (`monster.cpp:2143-2146`) adds `dice( melee_dice, melee_sides )` of
/// bash to the type's fixed `melee_damage`, carrying the type's armour penetration. `dice` is
/// `sum of rng( 1, sides )` number times (`rng.cpp:211-218`).
#[test]
fn a_monster_s_damage_is_its_dice_plus_its_type_s_own() {
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    for _ in 0..20 {
        let rolled = dice_total(&mut rolls, 2, 4);
        assert!((2..=8).contains(&rolled), "2d4 lies between two and eight, got {rolled}");
    }

    // A type that does one die of four, on top of a fixed three cut, with two points of penetration.
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    let hit = monster_melee_damage(&mut rolls, Damage { bash: 3, cut: 0, stab: 0, penetration: 2 }, 1, 4);
    assert_eq!(hit.penetration, 2, "the penetration comes from the type");
    assert!(
        (4..=7).contains(&hit.bash),
        "three fixed plus one to four rolled is four to seven, got {}",
        hit.bash
    );

    // And a type with no dice adds nothing, which is how a monster whose only damage is a fixed
    // instance works - the reference tests `melee_dice > 0` before rolling at all.
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    let fixed_only = monster_melee_damage(&mut rolls, Damage { bash: 3, cut: 5, stab: 0, penetration: 0 }, 0, 0);
    assert_eq!(fixed_only.bash, 3);
    assert_eq!(fixed_only.cut, 5);
}

/// A whole swing, with both rolls given rather than drawn.
///
/// The rule is separated from the dice on purpose: a test that replayed a recording would be asserting on
/// the replay's own mapping as much as on the rule, and a test that drew would pass or fail by luck. Here
/// the roll is stated and the answer is checked.
#[test]
fn a_swing_runs_from_the_roll_to_what_got_through() {
    let attack = cdda_sim::melee::Attack {
        accuracy_tenths: 20,
        fixed: Damage {
            bash: 0,
            cut: 0,
            stab: 0,
            penetration: 0,
        },
        dice: 1,
        sides: 4,
    };
    let defence = cdda_sim::melee::Defence {
        dodge: 2,
        size: CreatureSize::Medium,
        resist: 1.0,
        cut_resist: 0.0,
        stab_resist: 0.0,
        worn_armour: 0.0,
        immobile: false,
    };
    // Accuracy 2.0 means a mean of 10 and a deviation of 25; a roll of 30 is one deviation high. Two dodge
    // is worth ten, so the spread is twenty: a hit, and not close enough to zero to be a dodge as well.
    let landed = cdda_sim::melee::resolve(30.0, Damage { bash: 3, cut: 0, stab: 0, penetration: 0 }, defence);
    assert_eq!(landed.spread, 20, "30 taken down by ten of dodge");
    assert!(landed.hit);
    assert!(!landed.dodged, "a clean hit is not a dodge");
    assert_eq!(landed.damage, 2.0, "three damage less one point of armour");

    // And the same swing on a roll of minus thirty: a miss, which costs the target nothing.
    let missed = cdda_sim::melee::resolve(-30.0, Damage { bash: 3, cut: 0, stab: 0, penetration: 0 }, defence);
    assert_eq!(missed.spread, -40, "minus thirty, less ten for dodge");
    assert!(!missed.hit, "a negative spread is a miss");
    assert!(missed.dodged, "and the target is told it dodged");
    assert_eq!(missed.damage, 0.0, "a miss deals nothing");

    // The dice are none of this function's business, which is the point of splitting it: the same rule with
    // a different damage roll gives a different number and the same spread.
    let bigger = cdda_sim::melee::resolve(30.0, Damage { bash: 9, cut: 0, stab: 0, penetration: 0 }, defence);
    assert_eq!(bigger.spread, landed.spread, "the spread does not depend on the dice");
    assert_eq!(bigger.damage, 8.0, "and eight of nine gets through one point of armour");

    // `strike` is the same rule with the engine's dice, so its damage is whatever it rolled.
    let mut rolls = Rolls::new(1);
    let drawn = cdda_sim::melee::strike(&mut rolls, attack, defence);
    assert!(drawn.damage >= 0.0, "a rolled swing never deals negative damage");
}

/// A blow carries every damage type it is made of, and each meets its own armour.
///
/// Reference: `Creature::deal_melee_hit` passes the whole `damage_instance` to `absorb_hit`, which walks its units and calls `get_effective_resist` per unit's **type**
/// (`damage.cpp:724-727`). The port resolved **bash only** until now, so a knife's twenty-one points of stabbing were worth nothing at all - a rule that looked correct and could
/// never be reached.
#[test]
fn a_blow_is_worth_all_of_its_damage_and_each_meets_its_own_armour() {
    use cdda_sim::melee::{Damage, Defence, resolve};

    let bare = Defence {
        dodge: 0,
        size: cdda_sim::CreatureSize::Medium,
        resist: 0.0,
        cut_resist: 0.0,
        stab_resist: 0.0,
        worn_armour: 0.0,
        immobile: false,
    };

    // A knife: no bash, twenty-one stab. A bash-only resolve answers zero here.
    let knife = Damage {
        bash: 0,
        cut: 0,
        stab: 21,
        penetration: 0,
    };
    let struck = resolve(100.0, knife, bare);
    assert!(struck.hit, "a hundred is a hit on anything that cannot dodge");
    assert!(
        (f64::from(struck.damage) - 21.0).abs() < f64::EPSILON,
        "all twenty-one points arrive: {}",
        struck.damage
    );

    // And a target armoured against cutting but not stabbing takes the stabbing whole.
    let armoured = Defence {
        cut_resist: 10.0,
        ..bare
    };
    assert!(
        (f64::from(resolve(100.0, knife, armoured).damage) - 21.0).abs() < f64::EPSILON,
        "cut armour does not stop stabbing"
    );

    // A blow made of two types adds both, each after its own armour.
    let mixed = Damage {
        bash: 10,
        cut: 10,
        stab: 0,
        penetration: 0,
    };
    assert!(
        (f64::from(resolve(100.0, mixed, armoured).damage) - 10.0).abs() < f64::EPSILON,
        "ten of bash arrives and ten of cut is stopped: {}",
        f64::from(resolve(100.0, mixed, armoured).damage)
    );
}
