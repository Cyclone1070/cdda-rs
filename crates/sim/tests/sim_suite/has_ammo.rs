//! **`has_ammo` asks the item in the dialogue's *other* slot, and the three branches are the reference's.**
//!
//! Reference: `f_has_ammo` (`condition.cpp:1963-1974`) reads `d.const_actor( true )->get_const_item()` - the **beta**, which for an inventory effect is the item - and answers
//! `item::ammo_sufficient( character )` (`item.cpp:11569-11582`): a charge-counted stack has enough when its charges reach the requirement, a comestible always has enough, and
//! anything else answers from its remaining shots.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// An item kind with the three facts `ammo_sufficient` reads.
fn a_kind(
    world: &mut World,
    id: &str,
    charges: i32,
    count_by_charges: bool,
    comestible: bool,
) -> Entity {
    cdda_sim::item_registry::set_item_types(
        world,
        &[cdda_sim::item_registry::Kind {
            id,
            volume_ml: 250,
            weight_g: 200,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let kind = cdda_sim::item_registry::id_of(world, id).expect("the kind is registered");
    cdda_sim::item_registry::install_kind_facts(
        world,
        &[(
            kind,
            cdda_sim::item_registry::KindFacts {
                count_by_charges,
                is_comestible: comestible,
                ..cdda_sim::item_registry::KindFacts::default()
            },
        )],
    );
    cdda_sim::drop_item(world, (0, 0), kind, charges)
}

#[test]
fn has_ammo_answers_from_the_item_the_dialogue_holds() {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // The data writes it bare, beside the `if` it guards: `{ "if": "has_ammo", "then": .. }`.
    let value = cdda_json::parse(br#""has_ammo""#).expect("json");
    let condition =
        cdda_content::dialogue::read_condition(&value).expect("**the port reads `has_ammo`**");
    let check = |world: &World, item: Entity| {
        cdda_sim::conditions::holds_with(world, speaker, Some(item), &condition)
    };

    // A charge-counted stack with charges answers yes; with none, no.
    let loaded = a_kind(&mut world, "nails", 12, true, false);
    assert!(check(&world, loaded), "a stack of twelve has enough");
    let empty = a_kind(&mut world, "nails", 0, true, false);
    assert!(!check(&world, empty), "an empty stack does not");

    // A comestible always has enough: eating it spends a charge, so "no ammo" is not a question it fails.
    let food = a_kind(&mut world, "sandwich", 0, false, true);
    assert!(
        check(&world, food),
        "**a comestible always has enough**, even with no charges"
    );

    // Anything else answers from the charges it holds, which is what a tool is.
    let tool = a_kind(&mut world, "flashlight", 3, false, false);
    assert!(check(&world, tool), "a tool with charges has enough");
    let flat = a_kind(&mut world, "flashlight", 0, false, false);
    assert!(!check(&world, flat), "and one without does not");

    // The creature itself is not an item, so the question cannot be answered about it.
    assert!(
        !check(&world, speaker),
        "a creature in the item slot answers no rather than guessing"
    );
}
