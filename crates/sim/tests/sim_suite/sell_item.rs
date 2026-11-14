//! **Selling is a transfer, not a deletion.**
//!
//! Reference: `f_u_sell_item` (`npctalk.cpp:3884-3930`): the player hands over `count` of an item - charges when the kind is counted in charges, whole items otherwise -
//! the NPC receives them, and the NPC's **debt** rises by `cost`. When the player has too few, the reference says so and runs `false_eocs`.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A survivor, a shopkeeper, and a kind of item the survivor carries.
fn a_shop(
    world: &mut World,
    id: &str,
    count_by_charges: bool,
    carried: i32,
) -> (Entity, Entity, Vec<Entity>) {
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
                ..cdda_sim::item_registry::KindFacts::default()
            },
        )],
    );
    let survivor = cdda_sim::creature::spawn_creature(world, "the survivor", (0, 0, 0));
    let shopkeeper = cdda_sim::creature::spawn_creature(world, "the shopkeeper", (1, 0, 0));
    cdda_sim::npcs::make_npc(world, shopkeeper, "shopkeeper");
    let mut items = Vec::new();
    for _ in 0..carried {
        let item = cdda_sim::drop_item(world, (0, 0), kind, 1);
        world.entity_mut(item).remove::<cdda_sim::Position>();
        world
            .entity_mut(item)
            .insert(cdda_sim::creature::Carried(survivor));
        items.push(item);
    }
    (survivor, shopkeeper, items)
}

fn sell(text: &str) -> cdda_content::dialogue::Effect {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    cdda_content::dialogue::read_effect(&value).expect("**the port reads `u_sell_item`**")
}

#[test]
fn selling_hands_the_items_over_and_raises_the_debt() {
    let mut world = World::new();
    // The data's own entry, from `EOC_MISSION_GET_DRIVEBELTS_done`.
    let effect = sell(r#"{ "u_sell_item": "drivebelt", "count": 3, "cost": 5 }"#);
    let (survivor, shopkeeper, _) = a_shop(&mut world, "drivebelt", false, 3);
    let held = cdda_sim::inventory::count_of(&world, survivor, "drivebelt");
    assert_eq!(held.0, 3, "the survivor starts with three");

    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        survivor,
        Some(shopkeeper),
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the sale runs"
    );

    assert_eq!(
        cdda_sim::inventory::count_of(&world, survivor, "drivebelt").0,
        0,
        "**the survivor has none left**"
    );
    assert_eq!(
        cdda_sim::inventory::count_of(&world, shopkeeper, "drivebelt").0,
        3,
        "**and the shopkeeper has them**, because selling is a transfer and not a deletion"
    );
    assert_eq!(
        cdda_sim::npc_opinion::debt_of(&world, shopkeeper),
        5,
        "the shopkeeper is owed the cost"
    );
}

/// **Too few sends the seller to the false branch, and moves nothing.**
///
/// Reference: `f_u_sell_item` (`npctalk.cpp:3911-3914`): the popup says what is missing and `run_eoc_vector( false_eocs, d )` runs. Nothing leaves the seller's hands.
#[test]
fn selling_what_is_not_there_runs_the_false_branch_and_moves_nothing() {
    let mut world = World::new();
    let effect = sell(
        r#"{ "u_sell_item": "drivebelt", "count": 3, "cost": 5,
             "true_eocs": [ { "id": "EOC_SOLD", "effect": [ { "u_message": "sold" } ] } ],
             "false_eocs": [ { "id": "EOC_SHORT", "effect": [ { "u_message": "not enough" } ] } ] }"#,
    );
    let (survivor, shopkeeper, _) = a_shop(&mut world, "drivebelt", false, 1);
    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        survivor,
        Some(shopkeeper),
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the effect answers"
    );

    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines.iter().any(|line| line.contains("not enough")),
        "**the false branch runs**: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains("sold")),
        "and the true branch does not"
    );
    assert_eq!(
        cdda_sim::inventory::count_of(&world, survivor, "drivebelt").0,
        1,
        "**the one drivebelt stays with its owner**"
    );
    assert_eq!(
        cdda_sim::inventory::count_of(&world, shopkeeper, "drivebelt").0,
        0,
        "and the shopkeeper gets nothing"
    );
    assert_eq!(
        cdda_sim::npc_opinion::debt_of(&world, shopkeeper),
        0,
        "and is owed nothing"
    );
}
