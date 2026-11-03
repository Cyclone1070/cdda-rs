//! An item can be the dialogue's **other participant**, which is what `u_run_inv_eocs` and `transform_item` need.
//!
//! Reference: `f_run_inv_eocs` (`npctalk.cpp:2851-2930`) makes the item the dialogue's `beta`, and `f_transform_item` (`npctalk.cpp:6112-6131`) converts that item. The
//! data's own use is a phone that a computer turns back into a working phone, so the pair is one rule in two halves.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A survivor holding one thing of each kind named.
fn a_survivor_with(world: &mut World, names: &[&str]) -> Vec<Entity> {
    // The registry the effects read: a locked phone, a phone, and a rock - the data's own three.
    let kinds = [
        cdda_sim::item_registry::Kind {
            id: "smart_phone_locked",
            volume_ml: 250,
            weight_g: 150,
            pockets: &[],
        },
        cdda_sim::item_registry::Kind {
            id: "smart_phone",
            volume_ml: 250,
            weight_g: 150,
            pockets: &[],
        },
        cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 250,
            weight_g: 500,
            pockets: &[],
        },
    ];
    cdda_sim::item_registry::set_item_types(world, &kinds);
    let speaker = cdda_sim::creature::spawn_creature(world, "the survivor", (0, 0, 0));
    let mut items = Vec::new();
    for name in names {
        let names_registry = cdda_sim::item_registry::snapshot(world);
        let Some(kind) = names_registry.id_of(name) else {
            continue;
        };
        let item = cdda_sim::drop_item(world, (0, 0), kind, 1);
        world.entity_mut(item).remove::<cdda_sim::Position>();
        world
            .entity_mut(item)
            .insert(cdda_sim::creature::Carried(speaker));
        items.push(item);
    }
    items
}

#[test]
fn a_program_runs_on_every_item_the_data_asks_for() {
    let mut world = World::new();
    let items = a_survivor_with(&mut world, &["smart_phone_locked", "smart_phone_locked", "rock"]);
    assert_eq!(items.len(), 3, "the three items are carried");

    // The data's own shape, from `computer_eocs.json`: turn every locked phone into a phone.
    let text = r#"{
        "u_run_inv_eocs": "all",
        "search_data": [ { "id": "smart_phone_locked" } ],
        "true_eocs": [ { "id": "EOC_RECOVER", "effect": [ { "transform_item": "smart_phone" } ] } ]
    }"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let Some(effect) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the port reads a program run over the inventory");
    };
    let speaker = world
        .get::<cdda_sim::creature::Carried>(items[0])
        .map(|carried| carried.0);
    let Some(speaker) = speaker else {
        panic!("the items know whose they are");
    };
    let applied =
        cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the list runs");

    // Both phones became phones, and the rock was never in the search.
    let names = cdda_sim::item_registry::snapshot(&mut world);
    let kinds: Vec<String> = items
        .iter()
        .filter_map(|item| world.get::<cdda_sim::GroundItem>(*item))
        .filter_map(|ground| names.name_of(ground.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            "smart_phone".to_owned(),
            "smart_phone".to_owned(),
            "rock".to_owned()
        ],
        "and only the matching items were transformed"
    );
}

#[test]
fn a_manual_pick_refuses_because_the_port_has_no_chooser() {
    let mut world = World::new();
    let items = a_survivor_with(&mut world, &["rock"]);
    let Some(speaker) = world
        .get::<cdda_sim::creature::Carried>(items[0])
        .map(|carried| carried.0)
    else {
        panic!("the item knows whose it is");
    };
    let text = r#"{"u_run_inv_eocs": "manual_mult", "true_eocs": [ { "effect": [ { "u_message": "x" } ] } ]}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let Some(effect) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the reader does not judge the mode");
    };
    let applied =
        cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "a menu the port cannot show is a refusal, not a guess"
    );
}
