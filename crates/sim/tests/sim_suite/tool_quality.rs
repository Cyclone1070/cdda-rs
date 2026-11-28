//! The best quality a creature can reach, which is what a butchery's blade is measured in.
//!
//! Reference: `Character::max_quality( qual, radius )` (`visitable.cpp:269-300`) takes the best quality over the character's own items, its bionics and its mutations, and
//! over the **ground within `PICKUP_RANGE`** - the reference lets a survivor use a knife lying next to a corpse. `butchery.cpp` asks for `qual_BUTCHER` on every action but a
//! dissection, which asks `qual_CUT_FINE` (`:385`, `:632`).

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// **The blade is the best quality among what the survivor carries and what lies beside the square.**
///
/// Reference: `max_quality( qual, PICKUP_RANGE )` (`visitable.cpp:286-300`), which starts from the character's own visit and then asks the map within six squares - so a
/// knife on the floor beside a corpse is a knife the survivor can butcher with, which is what a player actually does.
#[test]
fn the_blade_is_the_best_quality_carried_or_nearby() {
    let mut world = World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "knife_butcher",
            volume_ml: 250,
            weight_g: 200,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 8.0,
            melee_stab: 10.0,
            to_hit: 1,
        }],
    );
    let knife = cdda_sim::item_registry::id_of(&mut world, "knife_butcher").expect("the kind");
    cdda_sim::item_registry::set_item_qualities(
        &mut world,
        &[(knife, &[("BUTCHER".to_owned(), 2)])],
    );
    let survivor = world.spawn(cdda_sim::Inventory::new()).id();
    assert_eq!(
        cdda_sim::creature::best_quality(&mut world, survivor, "BUTCHER"),
        0,
        "**a survivor holding nothing has no quality at all**, which is the reference's answer for an empty inventory rather than a refusal"
    );

    let item = world
        .spawn(cdda_sim::GroundItem {
            kind: knife,
            charges: 1,
            age: 0,
            burnt: 0,
            damage: 0,
            burn: None,
        })
        .id();
    world
        .get_mut::<cdda_sim::Inventory>(survivor)
        .expect("the inventory")
        .add(item);
    assert_eq!(
        cdda_sim::creature::best_quality(&mut world, survivor, "BUTCHER"),
        2,
        "and the knife in it answers two"
    );
    assert_eq!(
        cdda_sim::creature::best_quality(&mut world, survivor, "CUT_FINE"),
        0,
        "a quality the knife does not have is still nothing"
    );
}

/// **The ground beside the survivor counts, up to six squares and no further.**
///
/// Reference: `max_quality( qual, PICKUP_RANGE )` (`visitable.cpp:286-300`), where `PICKUP_RANGE` is six (`game_constants.h:44`). A knife on the floor next to a corpse is a
/// knife the survivor can butcher with; one across the room is not.
#[test]
fn a_tool_on_the_floor_is_reachable_to_six_squares() {
    let mut world = World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "knife_butcher",
            volume_ml: 250,
            weight_g: 200,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 8.0,
            melee_stab: 10.0,
            to_hit: 1,
        }],
    );
    let knife = cdda_sim::item_registry::id_of(&mut world, "knife_butcher").expect("the kind");
    cdda_sim::item_registry::set_item_qualities(
        &mut world,
        &[(knife, &[("BUTCHER".to_owned(), 1)])],
    );
    let survivor = world
        .spawn((
            cdda_sim::Inventory::new(),
            cdda_sim::Position { x: 0, y: 0, z: 0 },
        ))
        .id();
    for (x, expected) in [(6, 1), (-6, 1), (7, 0)] {
        let near = world
            .spawn((
                cdda_sim::GroundItem {
                    kind: knife,
                    charges: 1,
                    age: 0,
                    burnt: 0,
                    damage: 0,
                    burn: None,
                },
                cdda_sim::Position { x, y: 0, z: 0 },
            ))
            .id();
        assert_eq!(
            cdda_sim::creature::best_quality(&mut world, survivor, "BUTCHER"),
            expected,
            "**a tool {x} squares away is {}**",
            if expected == 1 {
                "reachable"
            } else {
                "out of reach"
            }
        );
        world.despawn(near);
    }
}

/// **A quality the survivor must *have* counts only what the survivor carries.**
///
/// Reference: `Character::has_quality` (`visitable.cpp:197-210`) visits the character's own items - wielded, worn, carried - and **not** the ground, which is
/// `max_quality`'s job (`:286-300`). The two are different questions and the difference is the point: `can_do_activity_there` asks `has_quality( qual_BUTCHER, 1 )`
/// (`activity_item_handling.cpp:1254`), so a knife lying across the room is not a knife the gate accepts, however reachable it is to `max_quality`.
#[test]
fn having_a_quality_counts_only_what_is_carried() {
    let mut world = World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "knife_butcher",
            volume_ml: 250,
            weight_g: 200,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 8.0,
            melee_stab: 10.0,
            to_hit: 1,
        }],
    );
    let knife = cdda_sim::item_registry::id_of(&mut world, "knife_butcher").expect("the kind");
    cdda_sim::item_registry::set_item_qualities(
        &mut world,
        &[(knife, &[("BUTCHER".to_owned(), 2)])],
    );
    let survivor = world.spawn((cdda_sim::Position { x: 0, y: 0, z: 0 },)).id();
    let floor = world
        .spawn((
            cdda_sim::GroundItem {
                kind: knife,
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::Position { x: 1, y: 0, z: 0 },
        ))
        .id();
    assert!(
        !cdda_sim::creature::carried_has_quality(&mut world, survivor, "BUTCHER", 1),
        "**a knife on the floor is not a knife in hand**, which is what the activity gate asks"
    );

    // Carried is the link the store already has: the item says whose it is.
    world
        .entity_mut(floor)
        .insert(cdda_sim::creature::Carried(survivor));
    assert!(
        cdda_sim::creature::carried_has_quality(&mut world, survivor, "BUTCHER", 1),
        "**the same knife, carried, is the quality**"
    );
    assert!(
        cdda_sim::creature::carried_has_quality(&mut world, survivor, "BUTCHER", 2),
        "at level two, which is the knife's level"
    );
    assert!(
        !cdda_sim::creature::carried_has_quality(&mut world, survivor, "BUTCHER", 3),
        "**and not at level three**, which the knife does not reach"
    );
    assert!(
        !cdda_sim::creature::carried_has_quality(&mut world, survivor, "CUT_FINE", 1),
        "a quality the knife does not have is not answered at level one"
    );

    // **And what the hands hold is a kind, not an item.** The reference wields an `item`, and its quality is the kind's; the port's `Wielding` names the kind, so the answer has
    // to be read without an entity behind it - a rule that only looked at carried items would say a person holding a knife has none.
    world
        .entity_mut(floor)
        .remove::<cdda_sim::creature::Carried>();
    assert!(
        !cdda_sim::creature::carried_has_quality(&mut world, survivor, "BUTCHER", 1),
        "**put down, it is not carried**"
    );
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    assert!(
        cdda_sim::creature::carried_has_quality(&mut world, survivor, "BUTCHER", 1),
        "**wielded, it is**"
    );
}
