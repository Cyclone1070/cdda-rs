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
    let outcome = factory.insert(object(r#"{"id": "f_desk", "type": "furniture"}"#), "core");
    assert_eq!(outcome, InsertOutcome::Inserted);
    assert_eq!(factory.ids(), vec!["f_desk"]);
    assert!(factory.is_valid("f_desk"));
}

#[test]
fn an_abstract_is_kept_out_of_the_id_list_but_usable_as_a_base() {
    let mut factory = Factory::new("furniture");
    assert_eq!(
        factory.insert(
            object(r#"{"abstract": "base_desk", "type": "furniture"}"#),
            "core"
        ),
        InsertOutcome::Abstract
    );
    assert_eq!(
        factory.insert(
            object(r#"{"id": "f_desk", "copy-from": "base_desk", "type": "furniture"}"#),
            "core"
        ),
        InsertOutcome::Inserted
    );
    assert_eq!(factory.ids(), vec!["f_desk"]);
}

#[test]
fn an_object_whose_base_comes_later_is_deferred_until_it_can_be_resolved() {
    let mut factory = Factory::new("furniture");
    assert_eq!(
        factory.insert(
            object(r#"{"id": "f_desk", "copy-from": "base_desk", "type": "furniture"}"#),
            "core"
        ),
        InsertOutcome::Deferred
    );
    // The base arrives in the next file: the deferred object is retried at
    // finalisation, not before.
    assert_eq!(
        factory.insert(
            object(r#"{"abstract": "base_desk", "type": "furniture"}"#),
            "core"
        ),
        InsertOutcome::Abstract
    );
    assert!(factory.finalize().expect("resolves").is_empty());
    assert_eq!(factory.ids(), vec!["f_desk"]);
}

#[test]
fn a_cycle_is_reported_with_the_ids_in_it() {
    let mut factory = Factory::new("furniture");
    factory.insert(
        object(r#"{"id": "a", "copy-from": "b", "type": "furniture"}"#),
        "core",
    );
    factory.insert(
        object(r#"{"id": "b", "copy-from": "a", "type": "furniture"}"#),
        "core",
    );
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
    factory.insert(object(r#"{"id": "f_a", "type": "furniture"}"#), "core");
    factory.insert(object(r#"{"id": "f_b", "type": "furniture"}"#), "core");
    let outcome = factory.insert(object(r#"{"id": "f_a", "type": "furniture"}"#), "mod");
    assert_eq!(outcome, InsertOutcome::Replaced);
    assert_eq!(factory.ids(), vec!["f_a", "f_b"]);
}
