//! What a creature is wearing becomes what protects it, without anyone setting the number by hand.
//!
//! The pieces exist separately: `cdda_content::armour` works out what a garment protects from its materials and
//! thickness, and `PartArmour` is what a blow reads. This is the join, and it is a **system**, so `PartArmour` stays
//! a derived value with one writer rather than a component tests have to remember to fill in.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::{
    Body, Garment, PartArmour, WornBy, armour_on, dress, spawn_creature,
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
