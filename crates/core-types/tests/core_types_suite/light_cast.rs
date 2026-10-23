//! Casting a lamp's light onto the four-quadrant light map.
//!
//! Reference: `castLight` (`src/lightmap.cpp:881-980`) and `castLightAll` (`:986-1012`), which is the same octant walk eight times, once per transform. The walk is a
//! recursive span sweep: at each distance it covers a row of cells, and where the opacity changes it recurses into the part of the row that is different.
//!
//! Three facts the tests pin, because each one is a way a port quietly differs:
//!
//! * **the lamp's own square is not lit by the cast** - the caller lights it, because the walk starts one square out;
//! * **an opaque square is still given light, in the quadrant the light came from** - which is what makes a wall's near face bright and its far face dark;
//! * **a transparent square takes all of it in the default quadrant**, because for light passing through, the direction does not matter.

#![allow(clippy::expect_used)]
// A light map is a grid and the tests name squares in it by index: that is what makes them readable as maps.
#![allow(clippy::indexing_slicing)]

use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;
use cdda_core_types::shadowcasting::sight_calc;
use cdda_core_types::shadowcasting::{
    AMBIENT_OPEN, Quadrant, TransparencyGrid, cast_light_quadrants,
};

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-4,
        "{what}: got {passed}, expected {expected}"
    );
}

/// A square room of open air with walls around the edge.
fn room(side: usize) -> TransparencyGrid {
    let mut cells = vec![AMBIENT_OPEN; side * side];
    for x in 0..side {
        for y in 0..side {
            if x == 0 || y == 0 || x == side - 1 || y == side - 1 {
                cells[x * side + y] = 0.0;
            }
        }
    }
    TransparencyGrid::new(side, cells)
}

#[test]
fn light_falls_off_with_distance_the_way_open_air_does() {
    let grid = room(9);
    let lit = cast_light_quadrants(&grid, (4, 4), 4, 10.0, false);
    let side = 9;
    // One square east: `sight_calc( 10, open air, 1 )`. Three squares east is much dimmer.
    let near = lit[5 * side + 4].max();
    let far = lit[7 * side + 4].max();
    assert_close(
        near,
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "one square",
    );
    assert_close(
        far,
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 3),
        "three squares",
    );
    assert!(far < near, "and it falls off");
}

#[test]
fn the_lamp_does_not_light_its_own_square() {
    // The walk starts at distance 1. `apply_light_source` lights the source's own square separately; a port that lit it twice would make a lamp appear twice as bright.
    let grid = room(9);
    let lit = cast_light_quadrants(&grid, (4, 4), 4, 10.0, false);
    assert_close(lit[4 * 9 + 4].max(), 0.0, "the origin is untouched");
}

#[test]
fn a_wall_stops_the_light_and_takes_it_on_the_side_it_faces() {
    // A wall two squares east of the lamp: everything east of it is dark, and the wall itself is lit - in some quadrant, because the light reached one of its sides.
    let mut cells = vec![AMBIENT_OPEN; 81];
    cells[6 * 9 + 4] = 0.0;
    let grid = TransparencyGrid::new(9, cells);
    let lit = cast_light_quadrants(&grid, (4, 4), 6, 10.0, false);

    let wall = lit[6 * 9 + 4];
    assert!(wall.max() > 0.0, "the near face of the wall is lit");
    assert_close(
        lit[8 * 9 + 4].max(),
        0.0,
        "and nothing gets past it to the far side",
    );
}

#[test]
fn light_passing_through_a_square_all_lands_in_one_quarter() {
    // For a square light passes through, direction does not matter, so the reference writes the default quadrant and leaves the other three at zero.
    let grid = room(9);
    let lit = cast_light_quadrants(&grid, (4, 4), 4, 10.0, false);
    let east = lit[5 * 9 + 4];
    assert!(east.get(Quadrant::NorthEast) > 0.0, "the default quarter");
    for quarter in [
        Quadrant::SouthEast,
        Quadrant::SouthWest,
        Quadrant::NorthWest,
    ] {
        assert_close(east.get(quarter), 0.0, "and the other three are empty");
    }
}

#[test]
fn circles_on_makes_a_diagonal_cost_more_than_a_step() {
    // `last_intensity = calc( numerator, cumulative_transparency, dist )` where `dist` is `rl_dist` of the sweep's own offset (`lightmap.cpp:936-937`). With the
    // "circular distances" option off that is the row number; with it on, a diagonal is further than a step and so dimmer.
    let grid = room(9);
    let square = cast_light_quadrants(&grid, (4, 4), 4, 10.0, false);
    let round = cast_light_quadrants(&grid, (4, 4), 4, 10.0, true);
    let orthogonal = 5 * 9 + 4;
    let diagonal = 5 * 9 + 5;
    assert_close(
        square[orthogonal].max(),
        round[orthogonal].max(),
        "a straight step is the same either way",
    );
    assert_close(
        square[diagonal].max(),
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "square: a diagonal is one square away",
    );
    // The reference's `rl_dist` answers with an `int`, so a one-square diagonal is *still* one square with circles on - the option changes nothing at this range, and a
    // test that expected it to would be testing an imagined rule rather than the real one.
    assert_close(
        round[diagonal].max(),
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "round: sqrt(2) truncated to one",
    );
}

#[test]
fn light_does_not_leave_the_map() {
    // The cast runs in a corner: half its octants leave the grid on the first row and must not index past it.
    let grid = room(9);
    let lit = cast_light_quadrants(&grid, (1, 1), 4, 10.0, false);
    assert_close(
        lit[9 + 2].max(),
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "the square it does reach",
    );
    assert_eq!(lit.len(), 81, "and the map is still a map");
}
