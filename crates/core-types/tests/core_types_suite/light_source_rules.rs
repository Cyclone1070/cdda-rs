//! Lighting one source, and the trick that keeps a burning building from melting the frame rate.
//!
//! Reference: `map::apply_light_source` (`src/lightmap.cpp:1217-1294`) and `map::add_light_source` (`:666-670`). Every light in the world - a lamp, a fire, lava, a
//! player holding a torch - arrives here, and three things happen in a fixed order:
//!
//! 1. the source's **own square** is lit, in all four quarters, to at least one unit; its raw luminance also goes into the `sm` cache that `apparent_light_at` reads;
//! 2. a luminance of one or two is **quantised** - one and below casts nothing, two and below becomes `1.49`;
//! 3. the cast happens **only in directions where a buffered source is not already at least as bright**.
//!
//! The third is the burning-hospital optimisation, and it is the part worth testing: a row of equally bright fires does not cast one full set of rays each, because every
//! fire can see from the buffer that the fire beside it is at least as bright and skips that direction.

#![allow(clippy::expect_used)]
// A light map is a grid and the tests name squares in it by index: that is what makes them readable as maps.
#![allow(clippy::indexing_slicing)]

use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;
use cdda_core_types::shadowcasting::{
    AMBIENT_OPEN, LightMap, Quadrant, TransparencyGrid, sight_calc,
};

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-4,
        "{what}: got {passed}, expected {expected}"
    );
}

/// A field of open air with nothing buffered and nothing lit.
fn field(side: usize) -> LightMap {
    LightMap::new(
        TransparencyGrid::new(side, vec![AMBIENT_OPEN; side * side]),
        false,
    )
}

#[test]
fn a_dim_source_lights_only_its_own_square_and_casts_nothing() {
    // `std::max( float( lit_level::LOW ), luminance )` puts at least one unit of light in the square itself - a candle you are standing beside is visible even when it
    // cannot light anything else - and then `luminance <= lit_level::LOW` returns before any cast.
    let mut lit = field(9);
    lit.add_source((4, 4), 0.5);
    let cell = lit.light_at(4, 4);
    for quarter in [
        Quadrant::NorthEast,
        Quadrant::SouthEast,
        Quadrant::SouthWest,
        Quadrant::NorthWest,
    ] {
        assert_close(cell.get(quarter), 1.0, "all four quarters, at least one");
    }
    assert_close(lit.source_at(4, 4), 0.5, "the raw value, not the floor");
    assert_close(lit.light_at(5, 4).max(), 0.0, "and nothing was cast");
}

#[test]
fn a_luminance_of_two_is_raised_to_one_point_four_nine() {
    // `else if( luminance <= lit_level::BRIGHT_ONLY ) luminance = 1.49f;` - a cast of two units would decay below the point where a square counts as lit at all after one
    // step of open air, so the reference raises it just enough to survive that step.
    let mut lit = field(9);
    lit.add_source((4, 4), 2.0);
    let next = lit.light_at(5, 4).max();
    assert!(next > 0.0, "it does cast");
    assert!(next < 1.49, "and decays from 1.49");

    let mut brighter = field(9);
    brighter.add_source((4, 4), 3.0);
    assert!(
        brighter.light_at(5, 4).max() > next,
        "a luminance of three casts further than one of two"
    );
}

#[test]
fn a_bright_source_lights_its_own_square_and_casts_in_every_direction() {
    let mut lit = field(9);
    lit.add_source((4, 4), 10.0);
    assert_close(lit.source_at(4, 4), 10.0, "the sm cache");
    for (x, y, what) in [
        (5, 4, "east"),
        (3, 4, "west"),
        (4, 5, "south"),
        (4, 3, "north"),
    ] {
        assert_close(
            lit.light_at(x, y).max(),
            sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
            what,
        );
    }
}

#[test]
fn a_neighbour_that_is_already_at_least_as_bright_stops_the_cast_it_guards() {
    // `light_source_buffer[ neighbour ] < luminance` (`lightmap.cpp:1257-1260`), and it is **strict**: an equal neighbour is enough to skip. The buffer at `y - 1` gates
    // the pair of octants that cover `y + 1` - the reference's own inversion of the axis naming, reproduced here on purpose and recorded in the ledger. So a bright
    // source to the north stops the cast that goes *south*.
    let mut lit = field(9);
    lit.buffer_source((4, 3), 10.0);
    lit.add_source((4, 4), 10.0);
    assert_close(
        lit.light_at(4, 5).max(),
        0.0,
        "the cast it guards was skipped",
    );
    assert_close(
        lit.light_at(5, 4).max(),
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "the x directions are not affected, and are not inverted",
    );
}

#[test]
fn a_dimmer_neighbour_does_not_stop_the_cast() {
    // The other side of the same comparison: a neighbour with *less* light than this source is not a reason to skip, because this source reaches further.
    let mut lit = field(9);
    lit.buffer_source((4, 3), 9.0);
    lit.add_source((4, 4), 10.0);
    assert!(
        lit.light_at(4, 5).max() > 0.0,
        "the guarded direction is cast after all"
    );
}

#[test]
fn a_source_on_an_edge_cannot_read_outside_the_buffer() {
    // Each of the four tests guards one edge (`p2.y() != 0` and friends), so the corner reads nothing outside the buffer. The consequence of the inverted y pairing is
    // visible here: at `y == 0` the test that would cast towards `y + 1` is switched off, so a source in the top-left corner lights eastwards and not downwards.
    let mut lit = field(9);
    lit.add_source((0, 0), 10.0);
    assert_close(
        lit.light_at(1, 0).max(),
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "east is cast",
    );
    assert_close(
        lit.light_at(0, 1).max(),
        0.0,
        "and the guarded direction is not, because its buffer test is switched off at this edge",
    );
}
