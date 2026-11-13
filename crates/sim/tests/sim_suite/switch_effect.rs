//! **`switch` runs the effect list of the last case its value has reached.**
//!
//! Reference: `f_switch` (`npctalk.cpp:6998-7021`): the member is a number, every case is a number and an effect list, and the loop keeps the **last** case whose number is
//! `<=` the switch value - so a case written later wins even if it is smaller, and a value below every case runs nothing at all.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// Runs one `switch` entry and answers which messages it left behind.
fn run(text: &str) -> Vec<String> {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("**the port reads `switch`**");
    let applied = cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the switch runs");
    cdda_sim::messages::lines(&world)
}

#[test]
fn the_last_case_the_value_has_reached_is_the_one_that_runs() {
    let lines = run(
        r#"{ "switch": 5, "cases": [
             { "case": 1, "effect": [ { "u_message": "one" } ] },
             { "case": 3, "effect": [ { "u_message": "three" } ] },
             { "case": 10, "effect": [ { "u_message": "ten" } ] }
           ] }"#,
    );
    assert_eq!(
        lines,
        vec!["three".to_owned()],
        "**five has reached three and not ten, so only three's list runs**"
    );

    // The rule is the **last** match, not the greatest: a later case with a smaller number still wins.
    let lines = run(
        r#"{ "switch": 5, "cases": [
             { "case": 3, "effect": [ { "u_message": "three" } ] },
             { "case": 1, "effect": [ { "u_message": "one" } ] }
           ] }"#,
    );
    assert_eq!(
        lines,
        vec!["one".to_owned()],
        "**the later case wins even though its number is smaller**, because the loop keeps the last match rather than the best one"
    );

    // Below every case, nothing runs - and that is not a refusal.
    let lines = run(
        r#"{ "switch": 0, "cases": [
             { "case": 1, "effect": [ { "u_message": "one" } ] }
           ] }"#,
    );
    assert!(
        lines.is_empty(),
        "**a value no case has reached leaves nothing behind**: {lines:?}"
    );
}
