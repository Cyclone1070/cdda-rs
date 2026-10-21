//! The overmap's grid.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{OMAPX, Overmap};

#[test]
fn an_overmap_is_the_size_the_reference_says() {
    let overmap = Overmap::filled("field");
    assert_eq!(overmap.width(), 180);
    assert_eq!(overmap.height(), 180);
    assert_eq!((overmap.width(), overmap.height()), (OMAPX, OMAPX));
}

#[test]
fn a_new_overmap_is_all_field() {
    let overmap = Overmap::filled("field");
    assert_eq!(overmap.count("field"), 180 * 180);
    assert_eq!(overmap.terrain(0, 0), Some("field"));
    assert_eq!(overmap.terrain(179, 179), Some("field"));
}

#[test]
fn a_terrain_can_be_put_down_and_read_back() {
    let mut overmap = Overmap::filled("field");
    overmap.set_terrain(10, 20, "house");
    assert_eq!(overmap.terrain(10, 20), Some("house"));
    assert_eq!(overmap.terrain(10, 21), Some("field"), "and only that tile");
    assert_eq!(overmap.count("house"), 1);
    assert_eq!(overmap.count("field"), 180 * 180 - 1);
}

#[test]
fn the_edges_are_inside_and_one_past_them_is_not() {
    let overmap = Overmap::filled("field");
    assert!(overmap.contains(0, 0));
    assert!(overmap.contains(179, 179));
    assert!(!overmap.contains(180, 0));
    assert!(!overmap.contains(0, 180));
    assert!(!overmap.contains(-1, 0), "there is no wrapping");
    assert_eq!(overmap.terrain(-1, 0), None);
    assert_eq!(overmap.terrain(180, 0), None);
}

#[test]
fn putting_a_terrain_outside_the_grid_does_nothing() {
    let mut overmap = Overmap::filled("field");
    overmap.set_terrain(500, 500, "house");
    assert_eq!(overmap.count("house"), 0);
    assert_eq!(overmap.count("field"), 180 * 180);
}

#[test]
fn names_are_shared_so_the_same_terrain_is_the_same_number() {
    let mut overmap = Overmap::filled("field");
    overmap.set_terrain(1, 1, "house");
    overmap.set_terrain(2, 2, "house");
    assert_eq!(overmap.terrain_id(1, 1), overmap.terrain_id(2, 2));
    assert_ne!(overmap.terrain_id(1, 1), overmap.terrain_id(0, 0));
}

#[test]
fn a_terrain_nobody_used_counts_zero_rather_than_being_invented() {
    let overmap = Overmap::filled("field");
    assert_eq!(overmap.count("swamp"), 0);
}
