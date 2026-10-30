//! A creature's body, built from content the way the reference builds it at load time.
//!
//! Sources: `corpus/anatomy-parts.json` (the game's `anatomy.json`) and `corpus/anatomy.json` (`--dump-anatomy`).
//! Reference: `anatomy::finalize` (`anatomy.cpp:78-90`) resolves an anatomy's part ids once, at load, and keeps the
//! parts that exist. The port does the same join, and refuses rather than shrinking a body when a part is missing.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_json::{Value, parse};
use cdda_sim::creature::body_named;
use std::path::Path;

fn read(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(name);
    let text = std::fs::read(path).unwrap_or_else(|_| panic!("{name} exists"));
    parse(&text).unwrap_or_else(|_| panic!("{name} is valid JSON"))
}

fn content() -> (
    Vec<cdda_content::anatomies::Anatomy>,
    Vec<cdda_content::body_parts::BodyPartType>,
) {
    let Value::Array(anatomies) = read("anatomy-parts.json") else {
        panic!("the anatomies are an array");
    };
    let Value::Object(root) = read("anatomy.json") else {
        panic!("the dump's root is an object");
    };
    let Some(Value::Array(parts)) = root.get("parts") else {
        panic!("the dump has parts");
    };
    (
        cdda_content::anatomies::load_objects(&anatomies).expect("the anatomies load"),
        cdda_content::body_parts::load_objects(parts).expect("the parts load"),
    )
}

#[test]
fn a_person_s_body_is_assembled_from_the_two_files_that_describe_it() {
    let (anatomies, parts) = content();
    let body = body_named("human_anatomy", &anatomies, &parts).expect("a person has a body");
    assert_eq!(body.parts.len(), 12, "twelve parts");
    let torso = body
        .parts
        .iter()
        .find(|part| part.name == "torso")
        .expect("a torso");
    assert_eq!(torso.hit_size, 36.0, "with the size the reference states");
    let head = body.parts.iter().find(|part| part.name == "head").expect("a head");
    assert!(head.upper, "and the head out of a short attacker's reach");
    assert!(
        body.parts.iter().any(|part| part.name == "left arm"),
        "the arms are named as messages print them, not as the ids spell them"
    );
}

#[test]
fn a_body_that_is_not_described_is_refused_rather_than_shrunk() {
    let (anatomies, parts) = content();
    assert!(
        body_named("no_such_anatomy", &anatomies, &parts).is_none(),
        "an unknown anatomy has no body"
    );
    // And an anatomy listing a part nobody describes is refused too: half a body would make every later rule
    // quietly wrong, which is worse than having none.
    let broken = vec![cdda_content::anatomies::Anatomy {
        id: "broken".to_owned(),
        parts: vec!["torso".to_owned(), "wing".to_owned()],
    }];
    assert!(
        body_named("broken", &broken, &parts).is_none(),
        "a part the content does not describe refuses the whole body"
    );
}
