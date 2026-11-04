#![allow(clippy::expect_used, clippy::float_cmp, clippy::panic)]

//! The avatar's own blow: what a character holding nothing or holding a knife does to a monster.
//!
//! Reference: `Character::melee_attack_abstract` (`melee.cpp:625-630`) builds the blow from `used_weapon()`, which is the wielded item or `null_item_reference()`; the accuracy is
//! `Character::get_melee_hit_base` and the damage is the weapon's `melee_damage`. The port keeps the weapon as a **kind** - an id - on a `Wielding` component, so the blow is the
//! same rule a monster's is once the attack is worked out.

use bevy_ecs::prelude::*;
use cdda_sim::item_registry::{Kind, set_item_types};

/// A world with three kinds in it: empty hands' `null`, a rock, and a combat knife.
fn world_with_kinds() -> (World, cdda_sim::ItemTypeId, cdda_sim::ItemTypeId, cdda_sim::ItemTypeId) {
    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    let kinds = [
        Kind {
            id: "null",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        },
        Kind {
            id: "rock",
            volume_ml: 50,
            weight_g: 657,
            pockets: &[],
            thrown_damage: 7.0,
            melee_bash: 7.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -1,
        },
        Kind {
            id: "knife_combat",
            volume_ml: 250,
            weight_g: 500,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 21.0,
            to_hit: -1,
        },
    ];
    set_item_types(&mut world, &kinds);
    let null = cdda_sim::item_registry::id_of(&mut world, "null").expect("the null kind");
    let rock = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock kind");
    let knife = cdda_sim::item_registry::id_of(&mut world, "knife_combat").expect("the knife kind");
    (world, null, rock, knife)
}

/// A character with nothing in its hands swings with the null item: it aims worst and carries no damage.
///
/// The reference's own dump says `null` has melee bash 0 and `to_hit` -2, so a punch that hurt would be the port's invention rather than its port. Dexterity eight still gives the
/// character two points of accuracy, which is `get_hit_base`'s whole contribution.
#[test]
fn empty_hands_swing_for_nothing_and_aim_worst() {
    let (mut world, _, _, _) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 });

    let blow = cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor);
    assert_eq!(blow.accuracy_tenths, 20 - 20, "two points of dexterity and the null item's -2 balance");
    assert_eq!(blow.fixed.bash, 0, "empty hands carry no bash");
    assert_eq!(blow.fixed.stab, 0, "and no stabbing");
    assert_eq!(blow.dice, 0, "and no dice at all");
}

/// A knife in the hand carries its own damage and its own balance.
#[test]
fn a_knife_is_worth_its_own_stabbing_and_its_own_balance() {
    let (mut world, _, _, knife) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 })
        .insert(cdda_sim::creature::Wielding(knife));

    let blow = cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor);
    assert_eq!(blow.fixed.bash, 0, "a combat knife has no bash");
    assert_eq!(blow.fixed.stab, 21, "and twenty-one points of stabbing");
    assert_eq!(
        blow.accuracy_tenths, 10,
        "two points of dexterity less the knife's own -1 balance"
    );
}

/// And the blow lands on a monster: a knife takes hit points off a zombie, and empty hands take none.
#[test]
fn a_knife_draws_blood_and_empty_hands_do_not() {
    let (mut world, _, _, knife) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 });
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    cdda_sim::creature::set_health(&mut world, zombie, 80);
    world
        .entity_mut(zombie)
        .insert(cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 });

    let bare = cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert_eq!(bare, Some(0), "empty hands swing and do nothing");
    assert_eq!(
        cdda_sim::creature::health_of(&world, zombie),
        Some(80),
        "the zombie is untouched"
    );

    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    let cut = cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert!(
        cut.is_some_and(|dealt| dealt > 0),
        "a knife in the hand draws blood: {cut:?}"
    );
    assert!(
        cdda_sim::creature::health_of(&world, zombie).is_some_and(|hp| hp < 80),
        "and the zombie is hurt"
    );
}

/// A practised hand aims better, by the weapon's own skill and by melee.
///
/// Reference: `Character::get_hit_weapon` (`melee.cpp:356-370`) is `skill / 3 + melee_skill / 2 + to_hit`, where `skill` is the one the weapon's **biggest damage type** trains
/// (`item::melee_skill`, `item.cpp:11071-11090`). A crowbar's damage is bash 19 and cut 1, so its skill is bashing; a combat knife's is stabbing. The melee skill is worth half a
/// point a level to every weapon, which is why a survivor who has practised anything at all is better with a rock than one who has not.
#[test]
fn a_practised_hand_aims_better_by_the_weapons_own_skill() {
    use std::collections::BTreeMap;

    let (mut world, _null, rock, knife) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 });

    // Nothing practised: dexterity's two points and the rock's own -1 balance.
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(rock));
    let bare = cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor);
    assert_eq!(
        bare.accuracy_tenths, 10,
        "two points of dexterity less one of balance, and no skill at all"
    );

    // Bashing three and melee four: three tenths a level of bashing and five a level of melee.
    world.entity_mut(survivor).insert(cdda_sim::creature::Skills(
        BTreeMap::from([("bashing".to_owned(), 3), ("melee".to_owned(), 4)]),
    ));
    assert_eq!(
        cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor).accuracy_tenths,
        10 + 10 + 20,
        "the rock trains bashing, so its skill counts"
    );

    // A knife trains stabbing, which the survivor has not practised: only the melee skill helps it.
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    assert_eq!(
        cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor).accuracy_tenths,
        10 + 20,
        "practising bashing does nothing for a blade"
    );
}

/// The weapon's skill is the one its **biggest** damage type trains, and a weapon with no damage trains nothing.
#[test]
fn a_weapons_skill_is_the_one_its_biggest_damage_trains() {
    use cdda_sim::melee::weapon_skill;
    assert_eq!(weapon_skill(19.0, 1.0, 0.0), Some("bashing"), "a crowbar is a club");
    assert_eq!(weapon_skill(0.0, 0.0, 21.0), Some("stabbing"), "a knife is a point");
    assert_eq!(weapon_skill(0.0, 12.0, 3.0), Some("cutting"), "a blade is an edge");
    assert_eq!(weapon_skill(0.0, 3.0, 12.0), Some("stabbing"), "and the point wins when it does more");
    assert_eq!(weapon_skill(0.0, 0.0, 0.0), None, "a thing that does no damage trains nothing");
}
