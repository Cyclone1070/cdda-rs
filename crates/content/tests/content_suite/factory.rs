//! The registry machinery: what a factory does with an object before any loader
//! has looked at its fields.
//!
//! The reference's `generic_factory` has four jobs worth copying exactly: it
//! keeps abstracts apart from concrete prototypes, it defers an object whose
//! `copy-from` base has not been loaded yet and retries it later
//! (`generic_factory.h:295-313`), it treats an abstract and a concrete prototype
//! as one namespace for `copy-from`, and it keeps the position of an id that is
//! loaded twice.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::factory::{Factory, InsertOutcome};
use cdda_json::{Object, Value, parse};

fn object(text: &str) -> Object {
    let value = parse(text.as_bytes()).expect("fixture parses");
    let Value::Object(object) = value else {
        panic!("fixture is not an object")
    };
    object
}

#[test]
fn a_concrete_object_is_listed_by_its_id() {
    let mut factory = Factory::new("furniture");
    let outcome = factory
        .insert(object(r#"{"id": "f_desk", "type": "furniture"}"#), "core")
        .expect("accepted");
    assert_eq!(outcome, InsertOutcome::Inserted);
    assert_eq!(factory.ids(), vec!["f_desk"]);
    assert!(factory.is_valid("f_desk"));
}

#[test]
fn an_abstract_is_kept_out_of_the_id_list_but_usable_as_a_base() {
    let mut factory = Factory::new("furniture");
    assert_eq!(
        factory
            .insert(
                object(r#"{"abstract": "base_desk", "type": "furniture"}"#),
                "core"
            )
            .expect("accepted"),
        InsertOutcome::Abstract
    );
    assert_eq!(
        factory
            .insert(
                object(r#"{"id": "f_desk", "copy-from": "base_desk", "type": "furniture"}"#),
                "core"
            )
            .expect("accepted"),
        InsertOutcome::Inserted
    );
    assert_eq!(factory.ids(), vec!["f_desk"]);
}

#[test]
fn an_object_whose_base_comes_later_is_deferred_until_it_can_be_resolved() {
    let mut factory = Factory::new("furniture");
    assert_eq!(
        factory
            .insert(
                object(r#"{"id": "f_desk", "copy-from": "base_desk", "type": "furniture"}"#),
                "core"
            )
            .expect("accepted"),
        InsertOutcome::Deferred
    );
    // The base arrives in the next file: the deferred object is retried at
    // finalisation, not before.
    assert_eq!(
        factory
            .insert(
                object(r#"{"abstract": "base_desk", "type": "furniture"}"#),
                "core"
            )
            .expect("accepted"),
        InsertOutcome::Abstract
    );
    assert!(factory.finalize().expect("resolves").is_empty());
    assert_eq!(factory.ids(), vec!["f_desk"]);
}

#[test]
fn a_cycle_is_reported_with_the_ids_in_it() {
    let mut factory = Factory::new("furniture");
    factory
        .insert(
            object(r#"{"id": "a", "copy-from": "b", "type": "furniture"}"#),
            "core",
        )
        .expect("accepted");
    factory
        .insert(
            object(r#"{"id": "b", "copy-from": "a", "type": "furniture"}"#),
            "core",
        )
        .expect("accepted");
    let unresolved = factory
        .finalize()
        .expect("finalize reports instead of failing");
    assert_eq!(unresolved.len(), 2, "{unresolved:?}");
    assert!(unresolved.iter().any(|id| id == "a"));
    assert!(unresolved.iter().any(|id| id == "b"));
}

#[test]
fn loading_the_same_id_twice_keeps_its_position() {
    let mut factory = Factory::new("furniture");
    factory
        .insert(object(r#"{"id": "f_a", "type": "furniture"}"#), "core")
        .expect("accepted");
    factory
        .insert(object(r#"{"id": "f_b", "type": "furniture"}"#), "core")
        .expect("accepted");
    let outcome = factory
        .insert(object(r#"{"id": "f_a", "type": "furniture"}"#), "mod")
        .expect("accepted");
    assert_eq!(outcome, InsertOutcome::Replaced);
    assert_eq!(factory.ids(), vec!["f_a", "f_b"]);
}

#[test]
fn two_definitions_from_the_same_source_are_an_error() {
    // Reference: `mod_tracker::check_duplicate_entries` (`src/mod_tracker.h:75-82`). A second definition of an id from the **same** mod is not an override, it is
    // a mistake in the data, and the reference throws: "'<id>' (<type>) has two definitions from the same source (<src>)!". The port replaced it silently, which
    // is the one outcome that would let a broken data file load happily.
    let mut factory = cdda_content::factory::Factory::new("trap");
    let first = object(r#"{"type": "trap", "id": "tr_dup", "name": "first"}"#);
    let second = object(r#"{"type": "trap", "id": "tr_dup", "name": "second"}"#);
    factory.insert(first, "dda").expect("the first definition");
    let error = factory
        .insert(second.clone(), "dda")
        .expect_err("the same source twice is an error");
    assert!(error.to_string().contains("tr_dup"), "the id is named");
    assert!(error.to_string().contains("dda"), "the source is named");

    // The same object from a *different* source is an override, which is how a mod replaces a core prototype: no error, and the later definition wins.
    factory.insert(second, "mod_override").expect("an override");
    let stored = factory.get("tr_dup").expect("the prototype");
    assert_eq!(stored.src, "mod_override", "the later source wins");
}

#[test]
fn the_shipped_data_has_no_two_definitions_from_one_source() {
    // The reference throws on the first same-source duplicate it sees, so a data tree it loads at all is a data tree with none. Walking every file in load order and
    // inserting into a real factory is what makes that claim about the content rather than about a comment: if the port's own rule is too strict, this test is
    // where it shows up.
    let mut factory = cdda_content::factory::Factory::new("trap");
    let mut duplicates = Vec::new();
    for root in crate::corpus::dirs() {
        let Ok(files) = cdda_content::json_files_in_load_order(&root) else {
            continue;
        };
        for path in files {
            let Some(value) = cdda_content::read_json(&path) else {
                continue;
            };
            let entries = match &*value {
                cdda_json::Value::Array(entries) => entries.clone(),
                other => vec![other.clone()],
            };
            for entry in entries {
                let cdda_json::Value::Object(object) = entry else {
                    continue;
                };
                if object.get("type") != Some(&cdda_json::Value::Str("trap".to_owned())) {
                    continue;
                }
                if let Err(duplicate) = factory.insert(object, "dda") {
                    duplicates.push(duplicate.to_string());
                }
            }
        }
    }
    assert!(
        duplicates.is_empty(),
        "the reference would refuse to load: {duplicates:?}"
    );
}
