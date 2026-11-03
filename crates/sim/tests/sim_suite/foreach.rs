//! A list of ids can be walked, with each element named in a variable as the list runs.
//!
//! Reference: `f_foreach` (`npctalk.cpp:7022-7080`): `foreach` says what the list is (`ids`, `array`, `item_group`, `monstergroup`), `target` says which ids,
//! `var` is written with each element, and `effect` runs once per element. The data uses it to name every flag, every trait or every vitamin in one message.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;

#[test]
fn an_array_is_walked_and_each_element_is_named() {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    // `{"foreach": "array", "target": ["one", "two"], "var": {"context_val": "id"}, "effect": [{"u_message": "<context_val:id>"}]}`
    let text = r#"{"foreach": "array", "target": ["one", "two"], "var": {"context_val": "id"}, "effect": [{"u_message": "<context_val:id>"}]}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let Some(effect) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the port reads a foreach over an array");
    };

    let applied =
        cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the list runs");
    // The variable keeps the last element, which is what the reference's `write_var_value` leaves behind.
    assert!(
        cdda_sim::dialogue_values::text_of(&world, speaker, "id").is_some_and(|value| value == "two"),
        "and the last element is what the variable holds afterwards"
    );
}

#[test]
fn a_flag_list_is_walked_from_the_data_s_own_list() {
    let mut world = World::new();
    cdda_sim::flag_registry::install(
        &mut world,
        vec!["BELTED".to_owned(), "WATERPROOF".to_owned()],
    );
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let text = r#"{"foreach": "ids", "target": "flag", "var": {"context_val": "id"}, "effect": [{"u_message": "<context_val:id>"}]}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let Some(effect) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the port reads a foreach over flags");
    };
    let applied =
        cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the flag list runs");
    assert!(
        cdda_sim::dialogue_values::text_of(&world, speaker, "id")
            .is_some_and(|value| value == "WATERPROOF"),
        "and the variable holds the last flag the data declares"
    );
}

#[test]
fn a_list_the_port_does_not_know_refuses_rather_than_walking_nothing() {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let text = r#"{"foreach": "monstergroup", "target": "GROUP_ZOMBIE", "var": {"context_val": "id"}, "effect": [{"u_message": "x"}]}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let Some(effect) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the reader does not judge the list's kind");
    };
    let applied =
        cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "a list with no registry behind it refuses"
    );
}
