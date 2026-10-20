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

#[test]
fn the_order_fields_were_added_does_not_change_the_turn() {
    // The reference walks a submap x outer, y inner (`src/map_field.cpp:168-170`), so the
    // order fields are visited is a property of the map, not of the order things were
    // built. Two levels with the same fields, added in opposite orders, must produce the
    // same turn - including the same draws from the shared stream.
    let positions = [(0, 0), (5, 1), (2, 9), (11, 4), (7, 7)];
    let mut ascending = Level::new();
    let mut descending = Level::new();
    ascending.ensure(0, 0, 1);
    descending.ensure(0, 0, 1);
    for (x, y) in positions {
        ascending.add_field(x, y, FieldId(1), field(2, 40), 1);
    }
    for (x, y) in positions.iter().rev() {
        descending.add_field(*x, *y, FieldId(1), field(2, 40), 1);
    }

    let mut first = Rolls::new(99);
    let mut second = Rolls::new(99);
    let a = ascending.process_fields(1_000, &mut first);
    let b = descending.process_fields(1_000, &mut second);
    assert_eq!(a, b, "same fields, same turn");

    // The stream has to be in the same state afterwards, which is the part a comparison
    // of intensities would miss if two visits happened to draw the same values.
    assert_eq!(
        first.engine().next_u32(),
        second.engine().next_u32(),
        "the draws were taken in the same order"
    );
}

#[test]
fn a_turn_is_one_entry_point_and_repeats_exactly() {
    // The same level, the same seed, the same turn: the same result. This is the
    // property every later processor inherits by being called from here rather than
    // from wherever its caller happens to be.
    let build = || {
        let mut level = Level::new();
        level.ensure(0, 0, 1);
        for x in 0..4 {
            level.add_field(x, 2, FieldId(1), field(2, 5), 1);
        }
        level
    };
    let mut one = build();
    let mut two = build();
    let mut first = Rolls::new(4242);
    let mut second = Rolls::new(4242);
    for turn in 0..25 {
        assert_eq!(
            one.tick(turn, &mut first),
            two.tick(turn, &mut second),
            "turn {turn} differed between two identical levels"
        );
    }
    assert_eq!(
        one.fielded_tiles(),
        two.fielded_tiles(),
        "the levels ended up the same shape"
    );
}

#[test]
fn a_tick_does_not_rebuild_light_that_nobody_asks_for() {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    level.add_field(1, 1, FieldId(1), field(2, 5), 1);
    let _ = level.tick(0, &mut rolls());
    assert_eq!(
        level.light_rebuilds(),
        0,
        "a turn nobody looks at costs no light map"
    );
    let _ = level.cached_visibility((5, 5), 3, |_| false);
    assert_eq!(level.light_rebuilds(), 1, "the first look pays for it");
}
