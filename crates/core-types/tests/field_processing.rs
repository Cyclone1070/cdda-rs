//! A turn of field processing on a level.
//!
//! Reference: `map::process_fields_in_submap` (`src/map_field.cpp:470-500`): age every
//! field on every tile, then take off the ones that died. Removal happening in one place
//! is the point - a dead field left lying around is a state nothing else should see.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;

fn rolls() -> Rolls {
    Rolls::new(7)
}

fn field(intensity: i32, half_life: i64) -> FieldEntry {
    let mut entry = FieldEntry::new(5, half_life, true, false);
    entry.set_intensity(intensity);
    entry
}

#[test]
fn every_field_on_the_level_is_aged_once_per_turn() {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    level.add_field(1, 1, FieldId(1), field(3, 100), 1);
    level.add_field(2, 1, FieldId(1), field(2, 100), 1);
    level.add_field(2, 1, FieldId(2), field(1, 100), 2);
    assert_eq!(level.fielded_tiles(), 2);

    let receipt = level.process_fields(50, &mut rolls());
    assert_eq!(receipt.ticked, 3, "three fields, three ticks");
    let ages: Vec<i64> = [
        level
            .fields_at(1, 1)
            .expect("fields")
            .get(FieldId(1))
            .expect("field")
            .age(),
        level
            .fields_at(2, 1)
            .expect("fields")
            .get(FieldId(1))
            .expect("field")
            .age(),
        level
            .fields_at(2, 1)
            .expect("fields")
            .get(FieldId(2))
            .expect("field")
            .age(),
    ]
    .to_vec();
    assert_eq!(
        ages,
        vec![1, 1, 1],
        "a field's age counts turns at its intensity"
    );
}

#[test]
fn a_field_that_dies_is_taken_off_the_tile() {
    // Half-life one: the first tick drops it from one to dead, and processing removes it.
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    level.add_field(3, 3, FieldId(1), field(1, 1), 1);
    let receipt = level.process_fields(10, &mut rolls());
    assert_eq!(receipt.removed, 1);
    assert_eq!(level.fielded_tiles(), 0, "the tile has no fields left");
    assert!(level.fields_at(3, 3).is_none());
}

#[test]
fn a_tile_that_loses_one_field_keeps_the_others() {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    level.add_field(4, 4, FieldId(1), field(1, 1), 1);
    level.add_field(4, 4, FieldId(2), field(2, 100), 2);
    let receipt = level.process_fields(10, &mut rolls());
    assert_eq!(receipt.removed, 1);
    let layer = level.fields_at(4, 4).expect("one field is left");
    assert_eq!(layer.len(), 1);
    assert!(layer.get(FieldId(2)).is_some());
    assert_eq!(
        layer.displayed(),
        Some(FieldId(2)),
        "the drawn type moves to what is left"
    );
}

#[test]
fn an_empty_level_costs_nothing_to_process() {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    let receipt = level.process_fields(1, &mut rolls());
    assert_eq!(receipt, cdda_core_types::level::FieldProcessing::default());
    assert_eq!(level.fielded_tiles(), 0);
}
