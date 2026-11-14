//! The event vocabulary is the reference's own table, dumped rather than transcribed.
//!
//! Reference: `trigger_event` (`npctalk.cpp:7894-7911`) gives its arguments **positionally**, and what position *n* means is `event_spec<Type>::fields[n]` (`event.h:173-990`).
//! `cata::event::get_fields` returns an unordered map, so the order comes from `--dump-events`, which the harness writes by walking each type's own array.

#![allow(clippy::expect_used, clippy::indexing_slicing)]

use std::path::PathBuf;

fn the_table() -> cdda_content::events::EventTable {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/events.json");
    let text = std::fs::read_to_string(&path).expect("the reference's event dump is in the corpus");
    let value = cdda_json::parse(text.as_bytes()).expect("the dump parses");
    cdda_content::events::EventTable::read(&value).expect("the port reads the event dump")
}

#[test]
fn every_event_type_has_its_argument_names_in_order() {
    let table = the_table();
    assert_eq!(
        table.len(),
        107,
        "the reference declares a hundred and seven event types"
    );

    // The two the data's own programs use, whose order is what a `trigger_event` argument list fills.
    assert_eq!(
        table.fields_of("u_var_changed"),
        Some(["var".to_owned(), "value".to_owned()].as_slice()),
        "a variable change names the variable first and the value second"
    );
    assert_eq!(
        table.fields_of("avatar_enters_omt"),
        Some(["pos".to_owned(), "oter_id".to_owned()].as_slice()),
        "and an overmap entry names where, then what terrain"
    );

    // A type the reference has and a name it does not are different answers, and the port must not confuse them.
    assert_eq!(
        table.fields_of("no_such_event"),
        None,
        "an unknown type is not answered"
    );
    assert_eq!(
        table.fields_of("game_begin"),
        Some(["cdda_version".to_owned()].as_slice()),
        "a type with one field keeps it"
    );
}
