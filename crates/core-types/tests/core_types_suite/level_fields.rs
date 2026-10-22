//! A level holding fields: where fire, smoke and the rest live between turns.
//!
//! Reference: a submap keeps a `field` per square, a map from field type to its entry, and the per-turn sweep walks those squares
//! in a fixed order. The sweep's order is x first and y inside (`map_field.cpp:445-449`), so the level has to be able to hand its
//! populated squares back in that order rather than in whatever order a hash map feels like.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::field::FieldEntry;
use cdda_core_types::level::Level;

fn field() -> FieldEntry {
    let mut entry = FieldEntry::new(3, 100, false, false);
    entry.set_age(2);
    entry
}

#[test]
fn a_level_with_nothing_burning_has_no_fields() {
    let level = Level::new();
    assert_eq!(level.field_squares(), Vec::new());
    assert!(level.field_at(0, 0).is_none());
}

#[test]
fn a_field_can_be_placed_read_and_taken_away() {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    level.add_field(5, 7, field());
    assert_eq!(level.field_at(5, 7).map(FieldEntry::age), Some(2));
    assert_eq!(level.field_squares(), vec![(5, 7)]);
    let taken = level.take_field(5, 7).expect("the field");
    assert_eq!(taken.age(), 2);
    assert!(
        level.field_at(5, 7).is_none(),
        "taking it leaves nothing behind"
    );
    assert_eq!(level.field_squares(), Vec::new());
}

#[test]
fn the_squares_come_back_column_by_column() {
    // The order the sweep will use, so a caller never has to sort: x first, y inside.
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for square in [(3, 9), (0, 0), (3, 2), (0, 11)] {
        level.add_field(square.0, square.1, field());
    }
    assert_eq!(level.field_squares(), vec![(0, 0), (0, 11), (3, 2), (3, 9)]);
}

#[test]
fn placing_a_field_twice_replaces_it() {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    level.add_field(1, 1, field());
    let mut stronger = FieldEntry::new(3, 100, false, false);
    stronger.set_intensity(3);
    level.add_field(1, 1, stronger);
    assert_eq!(level.field_at(1, 1).map(FieldEntry::intensity), Some(3));
    assert_eq!(level.field_squares().len(), 1);
}
