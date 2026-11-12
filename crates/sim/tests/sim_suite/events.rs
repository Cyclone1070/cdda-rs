//! **An event is sent, and what a position means is the reference's table.**
//!
//! Reference: `f_trigger_event` (`npctalk.cpp:7894-7911`) evaluates each argument of `args` as a `str_or_var` and sends the list onto the bus; `event::make_dyn` maps position
//! `n` onto the type's `n`th field. The port keeps one queue, drained at a declared point, so a sender never re-enters the rules that sent it.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// The data's own entry, from `portal_storm.json`: a variable change with a literal name and a value read from the player.
const STORM: &str = r#"{
    "trigger_event": "u_var_changed",
    "args": [ "counter_portal_storm_counter", { "u_val": "storm_counter" } ]
}"#;

#[test]
fn an_event_is_sent_with_its_arguments_evaluated() {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::dialogue_values::set(
        &mut world,
        speaker,
        "storm_counter",
        cdda_sim::dialogue_values::Value::Number(7.0),
    );
    let value = cdda_json::parse(STORM.as_bytes()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("**the port reads `trigger_event`**, which it never did");

    let applied =
        cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the effect runs");

    let sent = cdda_sim::events::sent(&world);
    assert_eq!(sent.len(), 1, "one event was sent: {sent:?}");
    assert_eq!(sent[0].kind, "u_var_changed", "the kind is the data's own name");
    assert_eq!(
        sent[0].args,
        vec!["counter_portal_storm_counter".to_owned(), "7".to_owned()],
        "**and the arguments are positional, the variable already worked out**"
    );
    assert!(cdda_sim::events::has_pending(&world), "it waits for the drain");

    let drained = cdda_sim::events::drain(&mut world);
    assert_eq!(drained.len(), 1, "the drain takes what was waiting");
    assert!(!cdda_sim::events::has_pending(&world), "and nothing is left waiting");
    assert_eq!(cdda_sim::events::sent(&world).len(), 1, "what was sent is still on the record");
}
