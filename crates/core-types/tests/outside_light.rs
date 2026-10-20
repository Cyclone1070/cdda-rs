//! Daylight reaching inside, quarter by quarter.
//!
//! Source: `dir_quadrants` and the ambient pass (`src/lightmap.cpp:384-438`). The table is
//! copied from the reference in its own order: the tile itself, then north, west, east and
//! south.

#![allow(clippy::panic, clippy::float_cmp)]

use cdda_core_types::shadowcasting::{FACING, FourQuadrants, Quadrant, light_in_from_outside};

#[test]
fn each_direction_lights_the_quarters_that_face_it() {
    assert_eq!(
        FACING[0],
        [Quadrant::NorthEast, Quadrant::NorthWest],
        "the tile itself"
    );
    assert_eq!(
        FACING[1],
        [Quadrant::NorthEast, Quadrant::NorthWest],
        "north"
    );
    assert_eq!(
        FACING[2],
        [Quadrant::SouthWest, Quadrant::NorthWest],
        "west"
    );
    assert_eq!(
        FACING[3],
        [Quadrant::SouthEast, Quadrant::NorthEast],
        "east"
    );
    assert_eq!(
        FACING[4],
        [Quadrant::SouthEast, Quadrant::SouthWest],
        "south"
    );
}

#[test]
fn a_doorway_lights_the_floor_inside_it_and_not_the_rest() {
    // Open sky to the south of an indoor tile: the two quarters on the south side take the
    // daylight, the two on the north side stay dark. This is why standing in a doorway
    // leaves the back of the room dim.
    let dark = FourQuadrants::uniform(0.0);
    let lit = light_in_from_outside([false, false, false, false, true], 10.0, dark);
    assert_eq!(lit.get(Quadrant::SouthEast), 10.0);
    assert_eq!(lit.get(Quadrant::SouthWest), 10.0);
    assert_eq!(lit.get(Quadrant::NorthEast), 0.0);
    assert_eq!(lit.get(Quadrant::NorthWest), 0.0);
    assert_eq!(
        lit.max(),
        10.0,
        "the tile is as bright as its brightest quarter"
    );
}

#[test]
fn two_open_sides_light_two_pairs_and_can_share_a_quarter() {
    // East and south are both open: east lights SE and NE, south lights SE and SW, so
    // three quarters are lit and the north-west stays dark.
    let lit = light_in_from_outside(
        [false, false, false, true, true],
        10.0,
        FourQuadrants::uniform(0.0),
    );
    assert_eq!(lit.get(Quadrant::SouthEast), 10.0);
    assert_eq!(lit.get(Quadrant::NorthEast), 10.0);
    assert_eq!(lit.get(Quadrant::SouthWest), 10.0);
    assert_eq!(lit.get(Quadrant::NorthWest), 0.0);
}

#[test]
fn daylight_never_dims_a_corner_a_lamp_already_lit() {
    // The pass raises quarters to the daylight level; it does not set them to it. A tile
    // with a lamp in one corner keeps that corner when the sun comes up.
    let mut lit = FourQuadrants::uniform(0.0);
    lit.set(Quadrant::NorthWest, 30.0);
    let after = light_in_from_outside([false, false, false, false, true], 10.0, lit);
    assert_eq!(
        after.get(Quadrant::NorthWest),
        30.0,
        "the lamp is not dimmed"
    );
    assert_eq!(
        after.get(Quadrant::SouthWest),
        10.0,
        "the open side still lights"
    );
}
