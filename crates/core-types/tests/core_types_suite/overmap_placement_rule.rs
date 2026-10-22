//! Whether a shape fits somewhere, and why not when it does not.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{Direction, Overmap, Placement, can_place_shape};

fn wooded_or_open(terrain: &str) -> bool {
    matches!(terrain, "field" | "forest" | "forest_thick")
}

#[test]
fn a_shape_of_open_ground_fits_on_open_ground() {
    let overmap = Overmap::filled("field");
    let verdict = can_place_shape(
        &overmap,
        &[(0, 0), (1, 0)],
        (10, 10),
        Direction::North,
        &wooded_or_open,
    );
    assert_eq!(verdict, Placement::Fits);
    assert!(verdict.is_fits());
}

#[test]
fn a_shape_that_hangs_off_the_map_does_not_fit() {
    let overmap = Overmap::filled("field");
    // The map's own edge, whichever size it was built at: the shape is two tiles wide and reaches past it.
    let edge = (overmap.width() - 1, overmap.height() / 2);
    let at_edge = can_place_shape(
        &overmap,
        &[(0, 0), (1, 0)],
        edge,
        Direction::North,
        &wooded_or_open,
    );
    assert_eq!(at_edge, Placement::Outside);
    // The origin rotates to itself, so the corner case needs a tile that is *not* the origin: one step east, turned
    // west, lands one step north of the corner - off the top of the map.
    let off_the_corner = can_place_shape(
        &overmap,
        &[(0, 0), (1, 0)],
        (0, 0),
        Direction::West,
        &wooded_or_open,
    );
    assert_eq!(off_the_corner, Placement::Outside);
}

#[test]
fn ground_the_special_does_not_allow_refuses_the_placement() {
    let mut overmap = Overmap::filled("field");
    overmap.set_terrain(11, 10, "lake_surface");
    let verdict = can_place_shape(
        &overmap,
        &[(0, 0), (1, 0)],
        (10, 10),
        Direction::North,
        &wooded_or_open,
    );
    assert_eq!(verdict, Placement::WrongGround, "the second tile is water");
}

#[test]
fn the_rotation_turns_the_whole_shape() {
    let mut overmap = Overmap::filled("field");
    overmap.set_terrain(11, 10, "lake_surface");
    // North: the shape reaches east into the water. East: it reaches south, onto dry ground.
    let north = can_place_shape(
        &overmap,
        &[(0, 0), (1, 0)],
        (10, 10),
        Direction::North,
        &wooded_or_open,
    );
    let east = can_place_shape(
        &overmap,
        &[(0, 0), (1, 0)],
        (10, 10),
        Direction::East,
        &wooded_or_open,
    );
    assert_eq!(north, Placement::WrongGround);
    assert_eq!(east, Placement::Fits);
}

#[test]
fn one_step_of_rotation_turns_north_into_east() {
    assert_eq!(Direction::North.rotate_point((0, -1)), (0, -1));
    assert_eq!(Direction::East.rotate_point((0, -1)), (1, 0));
    assert_eq!(Direction::South.rotate_point((0, -1)), (0, 1));
    assert_eq!(Direction::West.rotate_point((0, -1)), (-1, 0));
    assert_eq!(Direction::West.rotate_point((1, 0)), (0, -1));
}

#[test]
fn an_empty_shape_fits_anywhere_inside_the_map() {
    let overmap = Overmap::filled("field");
    assert_eq!(
        can_place_shape(&overmap, &[], (10, 10), Direction::North, &wooded_or_open),
        Placement::Fits
    );
}
