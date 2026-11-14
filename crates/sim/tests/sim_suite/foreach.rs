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

    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        speaker,
        None,
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the list runs"
    );
    // The variable keeps the last element, which is what the reference's `write_var_value` leaves behind.
    assert!(
        cdda_sim::dialogue_values::text_of(&world, speaker, "id")
            .is_some_and(|value| value == "two"),
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
    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        speaker,
        None,
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the flag list runs"
    );
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
    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        speaker,
        None,
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "a list with no registry behind it refuses"
    );
}

/// A spell's id may come from a variable, and a cast that needs a chooser refuses.
///
/// Reference: `f_cast_spell` (`npctalk.cpp:5591-5657`): the id is a `str_or_var` (`get_str_or_var( spell_jo.get_member( "id" ), "id" )`), and `targeted` makes the
/// spell pick its own target with `select_target` - a player's choice. The port reads both and refuses the targeted one at run time rather than casting at a square
/// nobody chose.
#[test]
fn a_spell_id_may_be_a_variable() {
    let text = r#"{"u_cast_spell": {"id": {"context_val": "spell_to_cast"}, "message": "x"}, "targeted": true}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let Some(effect) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the port reads a spell whose id is a variable");
    };
    let cdda_content::dialogue::Effect::CastSpell {
        spell, targeted, ..
    } = effect
    else {
        panic!("it reads as the cast effect");
    };
    assert!(
        matches!(spell, cdda_content::dialogue::StringSource::Variable { .. }),
        "and the id stays a variable: {spell:?}"
    );
    assert!(targeted, "with the targeted flag kept");
}
