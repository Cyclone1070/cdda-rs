//! Four light values per tile.
//!
//! Source: `four_quadrants` and `quadrant` (`src/shadowcasting.h:24-90`). The index order
//! and the two combine operations are the parts other code depends on.

// Exact comparisons are the point here: every value in these tests is a small integer or
// a half, all exactly representable, and a tolerance would hide a wrong quarter.
#![allow(clippy::panic, clippy::float_cmp)]

use cdda_core_types::shadowcasting::{FourQuadrants, Quadrant};

#[test]
fn a_quadrant_keeps_the_reference_index_order() {
    // NE, SE, SW, NW - the order the reference's enum declares and every array uses.
    assert_eq!(Quadrant::NorthEast.index(), 0);
    assert_eq!(Quadrant::SouthEast.index(), 1);
    assert_eq!(Quadrant::SouthWest.index(), 2);
    assert_eq!(Quadrant::NorthWest.index(), 3);
}

#[test]
fn a_tile_is_as_bright_as_its_brightest_quarter() {
    let mut light = FourQuadrants::uniform(0.0);
    assert_eq!(light.max(), 0.0);
    light.set(Quadrant::SouthWest, 8.0);
    assert_eq!(light.max(), 8.0, "the brightest corner is the tile's light");
    assert_eq!(light.get(Quadrant::SouthWest), 8.0);
    assert_eq!(
        light.get(Quadrant::NorthEast),
        0.0,
        "the other corners are untouched"
    );
}

#[test]
fn two_lights_take_the_brighter_quarter_each() {
    let mut torch = FourQuadrants::uniform(3.0);
    torch.set(Quadrant::NorthEast, 9.0);
    let lamp = FourQuadrants::uniform(5.0);
    let both = torch.max_with(lamp);
    assert_eq!(
        both.get(Quadrant::NorthEast),
        9.0,
        "the torch wins its corner"
    );
    assert_eq!(both.get(Quadrant::SouthWest), 5.0, "the lamp wins the rest");
}

#[test]
fn combining_with_a_value_raises_the_dim_quarters_only() {
    // This is `elementwise_max( l, r )` with a scalar, used to lift a whole tile to an
    // ambient level without dimming the corner a lamp already lit.
    let mut light = FourQuadrants::uniform(1.0);
    light.set(Quadrant::NorthWest, 12.0);
    let lifted = light.max_with_value(3.5);
    assert_eq!(
        lifted.get(Quadrant::NorthWest),
        12.0,
        "a bright corner is not dimmed"
    );
    assert_eq!(lifted.get(Quadrant::NorthEast), 3.5);
    assert_eq!(lifted.max(), 12.0);
}

#[test]
fn multiplying_two_tiles_works_quarter_by_quarter() {
    // Light multiplied by transparency: what reaches a corner is what arrived times what
    // the corner let through.
    let mut light = FourQuadrants::uniform(2.0);
    light.set(Quadrant::SouthEast, 4.0);
    let mut transparency = FourQuadrants::uniform(0.5);
    transparency.set(Quadrant::SouthEast, 0.25);
    let through = light.multiplied(transparency);
    assert_eq!(through.get(Quadrant::NorthEast), 1.0);
    assert_eq!(through.get(Quadrant::SouthEast), 1.0);
}
