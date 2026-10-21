//! Vehicle parts and prototypes, against the reference's own list.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::vehicles;
use cdda_json::parse;
use std::path::Path;

fn dump() -> vehicles::Vehicles {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicles.json");
    let text = std::fs::read_to_string(path).expect("the vehicle dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    vehicles::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_vehicle_part_and_prototype() {
    let dump = dump();
    assert_eq!(dump.parts.len(), 701, "parts");
    assert_eq!(dump.prototypes.len(), 249, "prototypes");
}

#[test]
fn known_ids_are_present_and_no_id_repeats() {
    let dump = dump();
    assert!(dump.parts.iter().any(|id| id == "seat"), "a known part");
    assert!(
        dump.prototypes.iter().any(|id| id == "2seater2"),
        "a known vehicle"
    );
    for ids in [&dump.parts, &dump.prototypes] {
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
    let value = parse(br#"{"parts":["a"],"part_count":2,"prototypes":[],"prototype_count":0}"#)
        .expect("valid JSON");
    assert!(
        vehicles::from_dump(&value).is_err(),
        "a count that disagrees is an error"
    );
}
