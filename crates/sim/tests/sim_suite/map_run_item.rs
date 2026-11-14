//! **Items lying on the map can be the dialogue's other participant, and a radius says which ones.**
//!
//! Reference: `f_map_run_item_eocs` (`npctalk.cpp:6806-6875`): the target is `loc`, or the actor's own square when the data names none; the search walks every square
//! within `max_radius` of it and skips the ones inside `min_radius`, then runs the programs on the items there. A `manual`/`manual_mult` option is a **menu**, which this
//! headless port cannot show - so it refuses rather than choosing for the player.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A survivor standing at the origin, with `rock` items on the squares named.
fn a_world_with_rocks(at: &[(i32, i32)]) -> (World, Entity) {
    let mut world = World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 250,
            weight_g: 500,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        }],
    );
    let kind = cdda_sim::item_registry::id_of(&mut world, "rock").expect("registered");
    for square in at {
        cdda_sim::drop_item(&mut world, *square, kind, 1);
    }
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    (world, speaker)
}

#[test]
fn a_radius_says_which_items_the_program_runs_on() {
    let (mut world, speaker) = a_world_with_rocks(&[(0, 1), (2, 0), (9, 0)]);
    // `max_radius` 2 keeps the two near rocks and drops the far one; `min_radius` 0 includes the square the actor stands on.
    let text = r#"{
        "u_map_run_item_eocs": "all",
        "min_radius": 0,
        "max_radius": 2,
        "true_eocs": [ { "id": "EOC_TOUCHED", "effect": [ { "u_message": "touched" } ] } ],
        "false_eocs": [ { "id": "EOC_NONE", "effect": [ { "u_message": "none" } ] } ]
    }"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value)
        .expect("**the port reads `u_map_run_item_eocs`**");
    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        speaker,
        None,
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the effect runs"
    );

    let lines = cdda_sim::messages::lines(&world);
    let touched = lines.iter().filter(|line| line.contains("touched")).count();
    assert_eq!(
        touched, 2,
        "**the two rocks inside the radius were touched and the one outside was not**: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains("none")),
        "and the false branch never ran"
    );
}

#[test]
fn a_menu_option_refuses_because_the_port_has_no_chooser() {
    let (mut world, speaker) = a_world_with_rocks(&[(0, 1)]);
    // The data's only use of this effect (`EOC_map_item_test1`) is exactly this shape.
    let text = r#"{
        "u_map_run_item_eocs": "manual_mult",
        "min_radius": 1,
        "max_radius": 10,
        "title": "Test: Item collection",
        "true_eocs": [ { "id": "EOC_MOVED", "effect": [ { "u_message": "moved" } ] } ]
    }"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let effect =
        cdda_content::dialogue::read_effect(&value).expect("the reader does not judge the mode");
    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        speaker,
        None,
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "**a menu the port cannot show is a refusal, not a guess**"
    );
    assert!(
        !cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("moved")),
        "and nothing ran"
    );
}

/// **`min_radius` is inclusive and it excludes**, which is the half of the radius rule a zero can hide.
///
/// Reference: the walk takes a square when `rl_dist( center, pos ) >= min_radius` (`npctalk.cpp:6840`), so a rock under the actor's own feet is outside a search that starts
/// at one.
#[test]
fn the_inner_radius_excludes_the_square_the_actor_stands_on() {
    let (mut world, speaker) = a_world_with_rocks(&[(0, 0), (1, 0)]);
    let text = r#"{
        "u_map_run_item_eocs": "all",
        "min_radius": 1,
        "max_radius": 2,
        "true_eocs": [ { "id": "EOC_TOUCHED", "effect": [ { "u_message": "touched" } ] } ]
    }"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    cdda_sim::dialogue_effects::run_effects(
        &mut world,
        speaker,
        None,
        std::slice::from_ref(&effect),
    );

    let lines = cdda_sim::messages::lines(&world);
    assert_eq!(
        lines.iter().filter(|line| line.contains("touched")).count(),
        1,
        "**the rock at the actor's feet is inside `min_radius` and the one beside it is not**: {lines:?}"
    );
}
