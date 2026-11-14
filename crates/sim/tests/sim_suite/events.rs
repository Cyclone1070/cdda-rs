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
    let effect = cdda_content::dialogue::read_effect(&value)
        .expect("**the port reads `trigger_event`**, which it never did");

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

    let sent = cdda_sim::events::sent(&world);
    assert_eq!(sent.len(), 1, "one event was sent: {sent:?}");
    assert_eq!(
        sent[0].kind, "u_var_changed",
        "the kind is the data's own name"
    );
    assert_eq!(
        sent[0].args,
        vec!["counter_portal_storm_counter".to_owned(), "7".to_owned()],
        "**and the arguments are positional, the variable already worked out**"
    );
    assert!(
        cdda_sim::events::has_pending(&world),
        "it waits for the drain"
    );

    let drained = cdda_sim::events::drain(&mut world);
    assert_eq!(drained.len(), 1, "the drain takes what was waiting");
    assert!(
        !cdda_sim::events::has_pending(&world),
        "and nothing is left waiting"
    );
    assert_eq!(
        cdda_sim::events::sent(&world).len(),
        1,
        "what was sent is still on the record"
    );
}

/// **An event program answers the event it names, and reads its arguments by the reference's own field names.**
///
/// Reference: `eoc_events::notify` (`effect_on_condition.cpp:519-580`): only the programs whose `required_event` is this event's type run, and every field of the event goes
/// into the dialogue's **context**, so a program reads it with `context_val( .. )`. The names are the dumped table's (`corpus/events.json`), position by position.
#[test]
fn an_event_program_answers_its_own_event_with_the_arguments_bound() {
    let mut world = World::new();
    let avatar = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let table = {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/events.json");
        let text = std::fs::read_to_string(path).expect("the event dump is in the corpus");
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        cdda_content::events::EventTable::read(&value).expect("the table reads")
    };
    cdda_sim::events::install_vocabulary(&mut world, table);

    // Two programs: one answers `avatar_enters_omt`, one answers `game_begin`. Only the first may run.
    let enters = r#"{
        "id": "EOC_ENTERED",
        "eoc_type": "EVENT",
        "required_event": "avatar_enters_omt",
        "effect": [ { "u_message": { "context_val": "oter_id" } } ]
    }"#;
    let begins = r#"{
        "id": "EOC_BEGAN",
        "eoc_type": "EVENT",
        "required_event": "game_begin",
        "effect": [ { "u_message": "the game began" } ]
    }"#;
    let programs: Vec<cdda_content::dialogue::Program> = [enters, begins]
        .iter()
        .map(|text| {
            let value = cdda_json::parse(text.as_bytes()).expect("json");
            let program = cdda_content::dialogue::read_program(&value)
                .unwrap_or_else(|| panic!("**the port reads an event program**: {text}"));
            assert!(
                program.event.is_some(),
                "an event program says which event it answers"
            );
            program
        })
        .collect();
    cdda_sim::eoc_registry::set_programs(&mut world, &programs);

    cdda_sim::events::send(
        &mut world,
        cdda_sim::events::Event::new(
            "avatar_enters_omt",
            vec!["(3,4,0)".to_owned(), "house".to_owned()],
        ),
    );
    let ran = cdda_sim::events::dispatch(&mut world, avatar);
    assert_eq!(
        ran, 1,
        "**one program answers, and it is the one that names this event**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("house")),
        "**and it read the second argument through its field name**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert!(
        !cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("the game began")),
        "the program for another event never ran"
    );
    assert!(
        !cdda_sim::events::has_pending(&world),
        "the queue is empty after a dispatch"
    );
}
