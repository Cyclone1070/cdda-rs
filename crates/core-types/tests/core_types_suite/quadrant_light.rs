//! How much light reaches a **wall**, which only counts from the sides the player can see it from.
//!
//! Reference: `map::apparent_light_helper` (`src/lightmap.cpp:725-799`). For a square that light cannot pass through there is no single answer: the light map stores
//! four quadrant values, and the one that matters is the quadrant the square is being *looked at* from. So the helper asks each of the eight neighbours which
//! quadrants it sits in, keeps the ones the player can actually see, and multiplies those quadrants by the square's own light map.
//!
//! The neighbour-to-quadrant table is the whole rule, and two of its rows are worth noticing:
//!
//! | Neighbour | Quadrants it lights |
//! |---|---|
//! a cardinal neighbour lights **two** quadrants - the south one lights `SE` and `SW` |
//! a diagonal neighbour lights **one**, twice - the south-east one lights `SE` and `SE` |
//!
//! A port that gave a cardinal neighbour one quadrant, or a diagonal neighbour two, would be wrong in exactly the case the mechanism exists for: a wall seen from one
//! side, with a lamp on the other.

#![allow(clippy::expect_used)]
// The neighbour arrays are exactly eight long and every index below is a literal or a loop over that same table.
#![allow(clippy::indexing_slicing)]

use cdda_core_types::light::{
    Neighbourhood, Quadrant, Quadrants, SeenFrom, four_quadrants, visible_light,
};

/// Light is a float, so "the same" means "as close as floats get" and never `==`.
#[track_caller]
fn assert_light(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-6,
        "{what}: got {passed}, expected {expected}"
    );
}

fn open() -> SeenFrom {
    SeenFrom {
        seen: 1.0,
        camera: 1.0,
        dist: 1,
    }
}

/// Every neighbour visible, none opaque: the player can see the wall from every side.
fn all_visible() -> Neighbourhood {
    Neighbourhood {
        in_bounds: [true; 8],
        opaque: [false; 8],
        unimpaired_range: 60,
        neighbours: [open(); 8],
    }
}

#[test]
fn a_wall_seen_from_every_side_takes_the_brightest_quadrant() {
    let light = four_quadrants([0.1, 0.2, 0.3, 0.4]);
    let neighbourhood = all_visible();
    assert_light(
        visible_light(true, 1.0, &neighbourhood, &light),
        0.4,
        "every quadrant was reachable, so the brightest wins",
    );
}

#[test]
fn a_wall_seen_from_the_south_only_ignores_the_light_behind_it() {
    // Only the south neighbour is passable. The south neighbour lights SE and SW (index 1 and 2), so a lamp to the north - quadrant 0 or 3 - is invisible.
    let mut neighbourhood = all_visible();
    for index in 0..8 {
        // Order: south, north, east, south-east, north-east, west, south-west, north-west.
        if index != 0 {
            neighbourhood.opaque[index] = true;
        }
    }
    let behind = four_quadrants([1.0, 0.0, 0.0, 1.0]);
    assert_light(
        visible_light(true, 1.0, &neighbourhood, &behind),
        0.0,
        "light from the north does not reach a wall looked at from the south",
    );

    let in_front = four_quadrants([0.0, 0.5, 0.25, 0.0]);
    assert_light(
        visible_light(true, 1.0, &neighbourhood, &in_front),
        0.5,
        "and the southern quadrants do",
    );
}

#[test]
fn a_diagonal_neighbour_lights_one_quadrant_and_a_cardinal_lights_two() {
    // The diagonal at index 3 is the south-east one: it lights SE twice, and nothing else. With light only in the NE quadrant the answer is zero.
    let mut only_south_east = all_visible();
    for index in 0..8 {
        if index != 3 {
            only_south_east.opaque[index] = true;
        }
    }
    let north_east_lit = four_quadrants([1.0, 0.0, 0.0, 0.0]);
    assert_light(
        visible_light(true, 1.0, &only_south_east, &north_east_lit),
        0.0,
        "a diagonal neighbour does not light the quadrant away from it",
    );
    let south_east_lit = four_quadrants([0.0, 1.0, 0.0, 0.0]);
    assert_light(
        visible_light(true, 1.0, &only_south_east, &south_east_lit),
        1.0,
        "the south-east neighbour lights the south-east quadrant",
    );
}

#[test]
fn a_neighbour_nobody_can_see_does_not_count() {
    // Two ways to fail: the neighbour is past the player's unimpaired range with no camera, or its seen and camera values are both zero. Both skip the neighbour
    // entirely (`lightmap.cpp:779-786`).
    let mut neighbourhood = all_visible();
    for index in 0..8 {
        if index != 0 {
            neighbourhood.opaque[index] = true;
        }
    }
    neighbourhood.neighbours[0] = SeenFrom {
        seen: 0.0,
        camera: 0.0,
        dist: 1,
    };
    let lit = four_quadrants([1.0; 4]);
    assert_light(
        visible_light(true, 1.0, &neighbourhood, &lit),
        0.0,
        "nobody saw the south neighbour",
    );

    neighbourhood.neighbours[0].camera = 0.5;
    assert_light(
        visible_light(true, 1.0, &neighbourhood, &lit),
        1.0,
        "a camera sees it instead",
    );

    // The other way to fail: past the player's unimpaired range and no camera watching that neighbour.
    neighbourhood.neighbours[0] = SeenFrom {
        seen: 1.0,
        camera: 0.0,
        dist: 100,
    };
    assert_light(
        visible_light(true, 1.0, &neighbourhood, &lit),
        0.0,
        "too far to see that side of the wall",
    );
}

#[test]
fn a_wall_the_player_looks_at_from_nowhere_is_dark() {
    let mut neighbourhood = all_visible();
    neighbourhood.opaque = [true; 8];
    let lit = four_quadrants([1.0; 4]);
    assert_light(
        visible_light(true, 1.0, &neighbourhood, &lit),
        0.0,
        "every neighbour opaque: no quadrant is seen from anywhere",
    );
}

#[test]
fn a_square_light_passes_through_does_not_need_quadrants_at_all() {
    // The simple case (`lightmap.cpp:793-797`): for a non-opaque square the direction does not matter, so it is `visibility * brightest quadrant`.
    let neighbourhood = all_visible();
    let light = four_quadrants([0.1, 0.2, 0.3, 0.0]);
    assert_light(
        visible_light(false, 0.5, &neighbourhood, &light),
        0.15,
        "0.5 * 0.3",
    );
}

#[test]
fn the_quadrant_order_is_the_references() {
    // `enum class quadrant { NE, SE, SW, NW }` (`shadowcasting.h:24-30`), and `four_quadrants` indexes its array with it directly. A port that ordered these
    // clockwise from north would be a rotation of the truth, and the multiplier below would pair the wrong quadrants.
    assert_eq!(Quadrant::NorthEast as usize, 0);
    assert_eq!(Quadrant::SouthEast as usize, 1);
    assert_eq!(Quadrant::SouthWest as usize, 2);
    assert_eq!(Quadrant::NorthWest as usize, 3);
    let quadrants = Quadrants::new([1.0, 2.0, 3.0, 4.0]);
    assert_light(quadrants.get(Quadrant::SouthWest), 3.0, "the third value");
}
