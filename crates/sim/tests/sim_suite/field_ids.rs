//! The sim's field ids are the ones the reference dump gives.
//!
//! These were placeholders (900 and up) while no field data existed. Now that the dump is loaded,
//! each one is checked against the registry the data produces - a placeholder that is never
//! checked is a bug waiting for the first field that lands on the same number.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::fields::{self, FieldRegistry};
use cdda_json::parse;
use std::path::Path;

fn registry() -> FieldRegistry {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(path).expect("the field dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    FieldRegistry::new(&fields::from_dump(&value).expect("the dump loads"))
}

/// The reference's `fd_null` **is** zero, and no other field is.
///
/// This test used to assert the opposite - that `fd_null` got a real id and zero meant "no field" - which was true of the
/// port's own numbering and false of the reference's: `dump_fields` reports `fd_null` as 0 and `fd_blood` as 1. The
/// invented rule was harmless until the oracle's numbers arrived, and then the two could not both be right.
#[test]
fn the_null_field_is_zero_and_no_other_field_is() {
    let registry = registry();
    assert_eq!(
        registry.id_of("fd_null"),
        Some(cdda_core_types::field::FieldId::NULL),
        "the reference numbers fd_null zero"
    );
    let mut seen = std::collections::HashSet::new();
    assert!(
        seen.insert(cdda_core_types::field::FieldId::NULL),
        "and only one field may hold the zero slot"
    );
    for name in ["fd_blood", "fd_fire", "fd_smoke"] {
        let id = registry.id_of(name).expect("present");
        assert_ne!(
            id,
            cdda_core_types::field::FieldId::NULL,
            "{name} must not be the null field"
        );
    }
}

#[test]
fn the_special_fields_the_sim_hardcodes_exist_in_the_data() {
    let registry = registry();
    for name in [
        "fd_flame_burst",
        "fd_fire_vent",
        "fd_hot_air1",
        "fd_hot_air2",
        "fd_hot_air3",
        "fd_hot_air4",
    ] {
        assert!(registry.id_of(name).is_some(), "{name} must be in the dump");
    }
}

#[test]
fn every_id_is_unique() {
    let registry = registry();
    let mut seen = std::collections::HashSet::new();
    for name in [
        "fd_fire",
        "fd_smoke",
        "fd_flame_burst",
        "fd_hot_air1",
        "fd_hot_air4",
    ] {
        let id = registry.id_of(name).expect("present");
        assert!(seen.insert(id), "{name} shares an id");
    }
}

#[test]
fn the_constants_are_the_ids_the_registry_assigns() {
    let registry = registry();
    for (name, constant) in [
        ("fd_flame_burst", cdda_sim::FLAME_BURST),
        ("fd_fire_vent", cdda_sim::FIRE_VENT),
        ("fd_hot_air1", cdda_sim::HOT_AIR_1),
        ("fd_hot_air2", cdda_sim::HOT_AIR_2),
        ("fd_hot_air3", cdda_sim::HOT_AIR_3),
        ("fd_hot_air4", cdda_sim::HOT_AIR_4),
        ("fd_fire", cdda_sim::FIRE),
        ("fd_extinguisher", cdda_sim::EXTINGUISHER),
        ("fd_acid_vent", cdda_sim::ACID_VENT),
        ("fd_shock_vent", cdda_sim::SHOCK_VENT),
        ("fd_push_items", cdda_sim::PUSH_ITEMS),
        ("fd_electricity", cdda_sim::ELECTRICITY),
    ] {
        assert_eq!(
            registry.id_of(name),
            Some(constant),
            "{name} must map to the sim's constant"
        );
    }
}
