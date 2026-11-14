//! Taking something in hand.
//!
//! Reference: `Character::weapon` and `Character::wield` (`character.cpp:13150-13200`). The reference keeps three sets
//! of items - carried, worn, wielded - and the port had two and a half. A wielded thing being *also* carried is the part
//! worth pinning: it is the same entity, marked, and out of its pocket.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::ItemTypeId;
use cdda_sim::creature::{
    Carried, InPocket, Wielded, WieldedBy, give_moves, moves_left, spawn_creature, wield, wielding,
};

#[test]
fn wielding_takes_a_carried_thing_in_hand_and_out_of_its_pocket() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, survivor, 500);
    let pack = world.spawn(()).id();
    let crowbar = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(7), 1);
    world.entity_mut(crowbar).remove::<cdda_sim::Position>();
    world.entity_mut(crowbar).insert(Carried(survivor));
    world.entity_mut(crowbar).insert(InPocket {
        worn: pack,
        index: 0,
    });

    assert_eq!(wield(&mut world, survivor, crowbar, 500), Wielded::Done);

    assert_eq!(
        wielding(&mut world, survivor),
        Some(crowbar),
        "it is in hand"
    );
    assert!(
        world.get::<InPocket>(crowbar).is_none(),
        "and no longer in a pocket, because a thing cannot be in two places"
    );
    assert!(
        world.get::<Carried>(crowbar).is_some(),
        "but it is still carried: a wielded thing is a carried thing that is marked"
    );
    assert_eq!(
        moves_left(&world, survivor),
        500 - 125,
        "a hundred moves for the handling and twenty-five for half a litre"
    );
}

#[test]
fn a_creature_holds_one_thing_at_a_time() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, survivor, 2_000);
    let mut items = Vec::new();
    for _ in 0..2 {
        let item = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(7), 1);
        world.entity_mut(item).remove::<cdda_sim::Position>();
        world.entity_mut(item).insert(Carried(survivor));
        items.push(item);
    }
    let first = items.first().copied().expect("two were made");
    let second = items.get(1).copied().expect("two were made");

    assert_eq!(wield(&mut world, survivor, first, 100), Wielded::Done);
    assert_eq!(wield(&mut world, survivor, second, 100), Wielded::Done);

    assert_eq!(
        wielding(&mut world, survivor),
        Some(second),
        "the second is in hand"
    );
    assert!(
        world.get::<WieldedBy>(first).is_none(),
        "and the first is not, though it is still carried"
    );
    assert!(world.get::<Carried>(first).is_some());
}

#[test]
fn something_not_carried_cannot_be_wielded() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, survivor, 500);
    let on_the_floor = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(7), 1);

    assert_eq!(
        wield(&mut world, survivor, on_the_floor, 500),
        Wielded::NotInHand,
        "picking it up is a separate act"
    );
    assert_eq!(wielding(&mut world, survivor), None);
}

#[test]
fn too_few_moves_leaves_the_hands_as_they_were() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let item = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(7), 1);
    world.entity_mut(item).remove::<cdda_sim::Position>();
    world.entity_mut(item).insert(Carried(survivor));

    assert_eq!(
        wield(&mut world, survivor, item, 500),
        Wielded::TooSlow,
        "a creature with no moves left cannot take anything in hand"
    );
    assert!(world.get::<WieldedBy>(item).is_none());
}
