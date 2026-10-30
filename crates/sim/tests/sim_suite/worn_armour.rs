//! What a creature is wearing becomes what protects it, without anyone setting the number by hand.
//!
//! The pieces exist separately: `cdda_content::armour` works out what a garment protects from its materials and
//! thickness, and `PartArmour` is what a blow reads. This is the join, and it is a **system**, so `PartArmour` stays
//! a derived value with one writer rather than a component tests have to remember to fill in.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::{
    Body, Garment, GarmentBlow, PartArmour, Struck, Toughness, Wear, Wore, WornBy, armour_on, damage_worn,
    dress, settle_armour, spawn_creature, take_off, wear,
};

/// A blow of the given damage against a garment resisting one point, with a roll of zero - which meets any chance.
fn blow(premitigated: f64, fraction: f64) -> GarmentBlow {
    GarmentBlow {
        premitigated,
        own_resist: 1.0,
        covered_parts: 1,
        fragile: false,
        sturdy: false,
        fraction,
    }
}

/// A blow too small to hurt a sturdy garment.
fn sturdy_blow() -> GarmentBlow {
    GarmentBlow {
        premitigated: 0.0,
        own_resist: 100.0,
        covered_parts: 1,
        fragile: false,
        sturdy: true,
        fraction: 0.99,
    }
}
use cdda_sim::hit_location::BodyPart;

fn steel_layer() -> cdda_content::armour::Layer {
    // Half a millimetre of steel: six per millimetre, so three points of protection.
    cdda_content::armour::Layer {
        resist: 6.0,
        thickness: 0.5,
        cover: 100,
    }
}

fn vest() -> Garment {
    Garment {
        layers: vec![("torso".to_owned(), steel_layer())],
    }
}

#[test]
fn a_vest_on_the_torso_becomes_armour_on_the_torso() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world.entity_mut(survivor).insert(Body {
        parts: vec![BodyPart {
            name: "torso".to_owned(),
            hit_size: 36.0,
            hit_difficulty: 1.0,
            upper: false,
            lower: false,
        }],
    });
    let worn = world.spawn((WornBy(survivor), vest())).id();

    dress(&mut world);

    assert_eq!(
        armour_on(&world, survivor, "torso"),
        3,
        "half a millimetre of steel is three points"
    );
    assert_eq!(
        armour_on(&world, survivor, "left arm"),
        0,
        "and the vest says nothing about an arm"
    );
    assert!(world.get::<PartArmour>(survivor).is_some(), "the answer is stored for the blow to read");
    assert_eq!(world.get::<WornBy>(worn), Some(&WornBy(survivor)), "and the link is on the item");
}

#[test]
fn taking_the_vest_off_takes_the_armour_away() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let worn = world.spawn((WornBy(survivor), vest())).id();
    dress(&mut world);
    assert_eq!(armour_on(&world, survivor, "torso"), 3);

    world.despawn(worn);
    dress(&mut world);

    assert_eq!(
        armour_on(&world, survivor, "torso"),
        0,
        "a derived answer must not survive the thing it was derived from"
    );
    assert!(
        world.get::<PartArmour>(survivor).is_none(),
        "and no stale armour is left behind for a blow to read"
    );
}

#[test]
fn two_layers_on_one_part_add_up() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world.spawn((WornBy(survivor), vest()));
    world.spawn((
        WornBy(survivor),
        Garment {
            layers: vec![(
                "torso".to_owned(),
                cdda_content::armour::Layer {
                    resist: 1.0,
                    thickness: 2.0,
                    cover: 100,
                },
            )],
        },
    ));

    dress(&mut world);

    assert_eq!(armour_on(&world, survivor, "torso"), 5, "three of steel and two of leather");
}

/// Putting something on and taking it off, which is what a survivor does with a vest.
///
/// Reference: `Character::wear` refuses an item that is not armour and one that is already worn, and the armour the
/// blow reads is derived afterwards. The refusals are answers rather than silence: a game about scavenging that
/// quietly failed to put a vest on would be a game with a bug nobody could see.
#[test]
fn a_garment_can_be_put_on_once_and_taken_off_again() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let vest = world.spawn(vest()).id();
    assert_eq!(armour_on(&world, survivor, "torso"), 0, "nothing on yet");

    assert_eq!(wear(&mut world, survivor, vest), Wore::Worn);
    assert_eq!(armour_on(&world, survivor, "torso"), 3, "half a millimetre of steel");
    assert_eq!(
        wear(&mut world, survivor, vest),
        Wore::AlreadyWorn,
        "and putting it on again is refused"
    );
    assert_eq!(
        armour_on(&world, survivor, "torso"),
        3,
        "so the protection is not doubled by asking twice"
    );

    assert!(take_off(&mut world, vest), "it came off");
    assert_eq!(armour_on(&world, survivor, "torso"), 0, "and the armour went with it");
    assert!(!take_off(&mut world, vest), "taking it off twice is nothing to do");
}

#[test]
fn something_that_is_not_a_garment_cannot_be_worn() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let rock = world.spawn(()).id();
    assert_eq!(
        wear(&mut world, survivor, rock),
        Wore::NotAWearable,
        "a thing that says nothing about what it protects is not clothing"
    );
    assert!(world.get::<WornBy>(rock).is_none(), "and it is not worn either");
    assert_eq!(armour_on(&world, survivor, "torso"), 0);
}

/// A vest wears out and then stops protecting, because a ruined vest is not a vest.
///
/// Reference: `item::damage_armor_durability` -> `mod_damage` (`item.cpp:9441-9452`, `itype.h:1568-1588`). The port
/// despawns a ruined garment rather than leaving it on the wearer with its armour intact: an item whose protection
/// outlives the item is exactly the stale answer `dress` exists to prevent.
#[test]
fn a_garment_wears_out_in_four_steps_and_is_gone_on_the_fifth() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let vest = world.spawn(vest()).id();

    assert_eq!(wear(&mut world, survivor, vest), Wore::Worn);
    assert_eq!(armour_on(&world, survivor, "torso"), 3);

    // A fraction of zero meets any chance there is, so every one of these blows damages the vest.
    for step in 1..=4 {
        assert_eq!(
            damage_worn(&mut world, vest, &blow(20.0, 0.0)),
            Struck::Damaged,
            "step {step}"
        );
        assert_eq!(world.get::<Wear>(vest).map(|wear| wear.damage), Some(step * 1000));
        assert_eq!(armour_on(&world, survivor, "torso"), 3, "and it still protects");
    }

    assert_eq!(
        damage_worn(&mut world, vest, &blow(20.0, 0.0)),
        Struck::Ruined,
        "the fifth step passes the maximum"
    );
    assert!(
        world.get_entity(vest).is_err(),
        "a ruined garment is gone from the store"
    );
    assert_eq!(
        armour_on(&world, survivor, "torso"),
        0,
        "and its protection went with it"
    );
}

#[test]
fn a_blow_that_misses_the_garment_leaves_it_whole() {
    let mut world = World::new();
    let vest = world.spawn(vest()).id();
    assert_eq!(
        damage_worn(&mut world, vest, &sturdy_blow()),
        Struck::Untouched,
        "a sturdy thing under its threshold takes nothing"
    );
    assert_eq!(world.get::<Wear>(vest).map(|wear| wear.damage), None, "and no damage is recorded");
}

/// The flags a garment's data carries decide how hard it is to ruin - and the three of them together are not a
/// nicety. A `STURDY` vest shrugs off the small blow that would wreck an ordinary one, and an `UNBREAKABLE` one
/// never wears out at all.
///
/// Reference: `item::damage_armor_durability` (`item.cpp:9508-9532`).
#[test]
fn a_sturdy_garment_survives_what_ruins_an_ordinary_one() {
    let mut world = World::new();
    let ordinary = world.spawn(vest()).id();
    let tough = world
        .spawn((
            vest(),
            Toughness {
                sturdy: true,
                ..Toughness::default()
            },
        ))
        .id();
    // A small blow: five points against a garment resisting three.
    let small = GarmentBlow {
        premitigated: 5.0,
        own_resist: 3.0,
        covered_parts: 1,
        fragile: false,
        sturdy: true,
        fraction: 0.0,
    };
    assert_eq!(
        damage_worn(&mut world, tough, &small),
        Struck::Untouched,
        "sturdy and under its threshold is nothing"
    );

    // The same flags on the ordinary vest's blow - because the flags come from the component, not the blow - so the
    // comparison is made by giving the ordinary one a blow of its own with the same numbers but no sturdiness.
    let same_but_unsturdy = GarmentBlow {
        sturdy: false,
        ..small
    };
    assert_eq!(
        damage_worn(&mut world, ordinary, &same_but_unsturdy),
        Struck::Damaged,
        "while the same blow damages something that is not sturdy"
    );
}

#[test]
fn an_unbreakable_garment_is_never_damaged() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let garment = world
        .spawn((
            WornBy(survivor),
            vest(),
            Toughness {
                unbreakable: true,
                ..Toughness::default()
            },
        ))
        .id();
    dress(&mut world);

    for _ in 0..10 {
        assert_eq!(
            settle_armour(&mut world, survivor, "torso", 500.0, 0.0),
            Struck::Untouched,
            "not once"
        );
    }
    assert_eq!(world.get::<Wear>(garment).map(|wear| wear.damage), None);
    assert_eq!(armour_on(&world, survivor, "torso"), 3, "and it still protects");
}
