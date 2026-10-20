//! Several fields sharing one tile.
//!
//! Source: `field::add_field` (`src/field.cpp:158-182`). The two things worth pinning
//! are that merging sums intensities and that reviving a dead entry starts from zero
//! rather than from the strength it died at.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::field::{FieldEntry, FieldId, FieldLayer};

fn entry(intensity: i32) -> FieldEntry {
    let mut field = FieldEntry::new(5, 10, true, false);
    field.set_intensity(intensity);
    field
}

#[test]
fn a_tile_can_hold_several_field_types() {
    let mut layer = FieldLayer::new();
    assert!(layer.add(FieldId(1), entry(2), 1));
    assert!(layer.add(FieldId(2), entry(3), 2));
    assert_eq!(layer.len(), 2);
    assert_eq!(layer.get(FieldId(1)).map(FieldEntry::intensity), Some(2));
    assert_eq!(layer.get(FieldId(2)).map(FieldEntry::intensity), Some(3));
}

#[test]
fn adding_a_type_that_is_already_there_merges_it() {
    // "The fire got stronger" is not "the tile caught fire": the second add returns
    // false and the intensities sum, clamped to the type's maximum of five.
    let mut layer = FieldLayer::new();
    assert!(layer.add(FieldId(1), entry(2), 1));
    assert!(!layer.add(FieldId(1), entry(2), 1));
    assert_eq!(layer.get(FieldId(1)).map(FieldEntry::intensity), Some(4));
    assert!(!layer.add(FieldId(1), entry(4), 1));
    assert_eq!(
        layer.get(FieldId(1)).map(FieldEntry::intensity),
        Some(5),
        "summed past the type's maximum and clamped"
    );
    assert_eq!(layer.len(), 1);
}

#[test]
fn a_dead_field_is_revived_from_zero() {
    // A spent fire still reports intensity one; reviving it must give it the new
    // strength rather than adding to the one it died with.
    let mut layer = FieldLayer::new();
    layer.add(FieldId(1), entry(1), 1);
    assert!(
        layer.add(FieldId(2), entry(1), 1),
        "a second type for the tile"
    );
    // Kill the first one where it sits, which is how decay does it.
    layer
        .get_mut(FieldId(1))
        .expect("the field is there")
        .set_intensity(0);
    assert_eq!(layer.get(FieldId(1)).map(FieldEntry::is_alive), Some(false));
    layer.add(FieldId(1), entry(3), 1);
    assert_eq!(
        layer.get(FieldId(1)).map(FieldEntry::intensity),
        Some(3),
        "revived at the new strength, not 1 + 3"
    );
}

#[test]
fn the_null_field_type_is_never_stored() {
    let mut layer = FieldLayer::new();
    assert!(!layer.add(FieldId::NULL, entry(1), 1));
    assert!(layer.is_empty());
}

#[test]
fn the_drawn_field_is_the_one_with_the_highest_priority() {
    let mut layer = FieldLayer::new();
    layer.add(FieldId(1), entry(1), 1);
    layer.add(FieldId(2), entry(1), 5);
    assert_eq!(layer.displayed(), Some(FieldId(2)));
    // A tie goes to the later arrival, which is the reference's `>=`.
    layer.add(FieldId(3), entry(1), 5);
    assert_eq!(layer.displayed(), Some(FieldId(3)));
    // Something dimmer does not take the tile's face.
    layer.add(FieldId(4), entry(1), 2);
    assert_eq!(layer.displayed(), Some(FieldId(3)));
}
