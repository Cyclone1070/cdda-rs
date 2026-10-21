//! Professions and scenarios, against the reference's own list.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::starting_kit;
use cdda_json::parse;
use std::path::Path;

fn dump() -> starting_kit::StartingKit {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/starting_kit.json");
    let text = std::fs::read_to_string(path).expect("the starting kit dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    starting_kit::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_profession_and_scenario_the_reference_has() {
    let kit = dump();
    assert_eq!(kit.professions.len(), 320, "professions");
    assert_eq!(kit.scenarios.len(), 60, "scenarios");
}

#[test]
fn known_ids_are_present_and_no_id_repeats() {
    let kit = dump();
    assert!(
        kit.professions.iter().any(|id| id == "FBI"),
        "a known profession"
    );
    assert!(
        kit.scenarios.iter().any(|id| id == "Mansion"),
        "a known scenario"
    );
    for ids in [&kit.professions, &kit.scenarios] {
        let mut sorted = ids.clone();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "no duplicates, and the dump is sorted"
        );
    }
}

#[test]
fn a_dump_that_contradicts_itself_is_refused() {
    let value =
        parse(br#"{"professions":["a"],"profession_count":2,"scenarios":[],"scenario_count":0}"#)
            .expect("valid JSON");
    assert!(
        starting_kit::from_dump(&value).is_err(),
        "a count that disagrees is an error"
    );
}
