//! What a creature is wearing becomes what protects it, without anyone setting the number by hand.
//!
//! The pieces exist separately: `cdda_content::armour` works out what a garment protects from its materials and
//! thickness, and `PartArmour` is what a blow reads. This is the join, and it is a **system**, so `PartArmour` stays
//! a derived value with one writer rather than a component tests have to remember to fill in.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::{
    Body, Garment, PartArmour, Wore, WornBy, armour_on, dress, spawn_creature, take_off, wear,
};
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
