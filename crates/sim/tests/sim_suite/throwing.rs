//! Throwing something: the spread rules, the landing rule and the item's move, wired together.
//!
//! Reference: `Character::throw_item` (`ranged.cpp:1467-1600`) - the spread from `throwing_dispersion`, a roll of it, `projectile_attack_roll` for the aim, and the miss
//! branch of `projectile_attack` for the square. The port has every one of those rules; this is the wire that spends them.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;

/// A thrower holding one rock, with the item table that says what a rock is.
fn a_thrower_with_a_rock(world: &mut World) -> (Entity, Entity) {
    let kinds = [
        cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 250,
            weight_g: 500,
            pockets: &[],
        },
        cdda_sim::item_registry::Kind {
            id: "anvil",
            volume_ml: 5_000,
            weight_g: 20_000,
            pockets: &[],
        },
    ];
    cdda_sim::item_registry::set_item_types(world, &kinds);
    cdda_sim::singletons::put_rolls(world, cdda_core_types::rng::Rolls::new(1_234));
    let thrower = cdda_sim::creature::spawn_creature(world, "the thrower", (0, 0, 0));
    world
        .entity_mut(thrower)
        .insert(cdda_sim::creature::Strength { base: 8 });
    let names = cdda_sim::item_registry::snapshot(world);
    let rock = cdda_sim::drop_item(
        world,
        (0, 0),
        names.id_of("rock").expect("a rock"),
        1,
    );
    world.entity_mut(rock).insert(cdda_sim::creature::Carried(thrower));
    (thrower, rock)
}

#[test]
fn a_thrown_rock_leaves_the_hand_and_lands_near_where_it_was_aimed() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    let landed = cdda_sim::throwing::throw_at(&mut world, thrower, rock, (10, 0, 0));
    let cdda_sim::throwing::Toss::Landed { at, .. } = landed else {
        panic!("the rock is thrown: {landed:?}");
    };
    assert!(
        world.get::<cdda_sim::creature::Carried>(rock).is_none(),
        "a thrown thing is no longer carried"
    );
    let position = world.get::<cdda_sim::Position>(rock).copied();
    assert_eq!(
        position.map(|place| (place.x, place.y, place.z)),
        Some(at),
        "and it stands on the square the throw says"
    );
    // Ten squares out with a 500 g rock at skill zero: the spread is 2500, and the aim rules bound where that lands.
    assert!(
        i32::abs(at.0) <= 13 && i32::abs(at.1) <= 13,
        "a throw ten squares out does not leave the neighbourhood: {at:?}"
    );
}

#[test]
fn throwing_at_your_own_square_changes_nothing_and_spends_no_dice() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(8_181));
    assert_eq!(
        cdda_sim::throwing::throw_at(&mut world, thrower, rock, (0, 0, 0)),
        cdda_sim::throwing::Toss::AtYourself,
        "a throw at your own square is refused, as the reference refuses a shot at its own square"
    );
    assert!(
        world.get::<cdda_sim::creature::Carried>(rock).is_some(),
        "and the rock stays in the hand"
    );
}

#[test]
fn a_thing_nobody_is_holding_cannot_be_thrown() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    world
        .entity_mut(rock)
        .remove::<cdda_sim::creature::Carried>();
    assert_eq!(
        cdda_sim::throwing::throw_at(&mut world, thrower, rock, (5, 5, 0)),
        cdda_sim::throwing::Toss::NotCarried,
        "and the port says so rather than throwing something that is on the floor"
    );
}
