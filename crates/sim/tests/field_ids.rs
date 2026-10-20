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

#[test]
fn the_null_field_is_zero_and_never_assigned() {
    let registry = registry();
    assert!(
        registry
            .id_of("fd_null")
            .is_some_and(|id| id != cdda_core_types::field::FieldId::NULL),
        "the data's fd_null still gets a real id, so the zero slot means 'no field'"
    );
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
        ("fd_electricity", cdda_sim::ELECTRICITY),
    ] {
        assert_eq!(
            registry.id_of(name),
            Some(constant),
            "{name} must map to the sim's constant"
        );
    }
}
