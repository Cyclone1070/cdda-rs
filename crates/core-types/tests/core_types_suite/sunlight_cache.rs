//! How daylight reaches a level, and the three regimes it passes through on the way down.
//!
//! Reference: `map::build_sunlight_cache( pzlev )` (`src/lightmap.cpp:306-437`). Sunlight is parallel, so the reference does not cast it - it walks the z-levels
//! **top to bottom** and lets each square take its light from the square above and the four beside it. Two flags carry the state across levels, and they are what keep
//! the common cases cheap:
//!
//! ```text
//! fully_outside = true,  fully_inside = false   ->  nothing has blocked the sun yet: fill the whole level
//!      |
//!      v  (an obstacle appears)
//! fully_outside = false, fully_inside = false   ->  per-square quadrant work
//!      |
//!      v  (everything below is opaque or floored)
//! fully_outside = false, fully_inside = true    ->  no light gets here at all: fill with the inside level
//! ```
//!
//! The middle regime has one rule worth reading twice: a square takes its light from **directly above** first and only falls back to the four cardinals, and a
//! cardinal lights just the two quarters facing it - which is how a shaft of daylight through a hole in a ceiling lands as a bright patch with a gradient.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use cdda_core_types::light::{AMBIENT_DIM, AMBIENT_LOW, SOURCE_BRIGHT, TRANSPARENCY_OPEN_AIR};
use cdda_core_types::shadowcasting::Quadrant;
use cdda_core_types::sunlight_cache::{AboveLayer, SunLayer, Sunlight, inside_light_level};

const SIDE: usize = 5;

/// A level of open air: nothing opaque, no floors, everything under the sky.
fn open_level() -> SunLayer {
    SunLayer {
        side: SIDE,
        transparency: vec![TRANSPARENCY_OPEN_AIR; SIDE * SIDE],
        floor: vec![false; SIDE * SIDE],
        outside: vec![true; SIDE * SIDE],
    }
}

/// A level with solid rock above every square and floors under them.
fn roofed_level() -> SunLayer {
    SunLayer {
        side: SIDE,
        transparency: vec![0.0; SIDE * SIDE],
        floor: vec![true; SIDE * SIDE],
        outside: vec![true; SIDE * SIDE],
    }
}

/// An open sky above the level: open air, no floors, and lit at the given value everywhere.
fn sky(light: f32) -> AboveLayer {
    AboveLayer {
        transparency: vec![TRANSPARENCY_OPEN_AIR; SIDE * SIDE],
        floor: vec![false; SIDE * SIDE],
        light: vec![light; SIDE * SIDE],
    }
}

fn open_at(level: &mut SunLayer, x: usize, y: usize) {
    level.transparency[x * SIDE + y] = TRANSPARENCY_OPEN_AIR;
    level.floor[x * SIDE + y] = false;
}

/// A pass that has already descended past an open sky and a partly roofed level, so that it is in the middle regime - the only regime where per-square work happens.
///
/// The order matters and is the reference's: a level of open air keeps `fully_outside` set and is filled wholesale; the partly roofed level below it is also filled
/// wholesale, and only then do the flags say "neither all outside nor all inside", which is what makes the level below the roof the first one to be computed.
fn primed() -> Sunlight {
    let mut sun = Sunlight::new();
    let _ = sun.step(&open_level(), 20.0, 4.0, 1.0);
    let mut partly = roofed_level();
    open_at(&mut partly, 2, 2);
    let _ = sun.step(&partly, 20.0, 4.0, 1.0);
    assert!(
        !sun.fully_outside() && !sun.fully_inside(),
        "the middle regime"
    );
    sun
}

#[test]
fn the_inside_light_level_depends_on_the_sky_and_not_on_the_depth() {
    // `( zlev >= 0 && outside_light_level > LIGHT_SOURCE_BRIGHT ) ? LIGHT_AMBIENT_DIM * 0.8 : LIGHT_AMBIENT_LOW` (`lightmap.cpp:338-340`). A bright sky makes indoors
    // dim-but-lit; a dark sky makes it dark everywhere, and below ground it is always the dark value.
    assert!((inside_light_level(0, 20.0) - AMBIENT_DIM * 0.8).abs() < 1e-6);
    assert!((inside_light_level(0, SOURCE_BRIGHT - 0.1) - AMBIENT_LOW).abs() < 1e-6);
    assert!((inside_light_level(-1, 20.0) - AMBIENT_LOW).abs() < 1e-6);
}

#[test]
fn an_unobstructed_level_takes_the_full_sky_and_stays_outside() {
    // The first regime: nothing has blocked the sun, so the level is filled with the outside light without any per-square work.
    let mut sun = Sunlight::new();
    let lit = sun.step(&open_level(), 20.0, 4.0, 1.0);
    assert!(lit.iter().all(|cell| (cell.max() - 20.0).abs() < 1e-6));
    assert!(sun.fully_outside(), "still no obstacle");
    assert!(!sun.fully_inside(), "so light still reaches here");
}

#[test]
fn a_roof_ends_the_outside_regime_and_the_level_below_is_skipped() {
    // A level opaque or floored everywhere is still *filled* with the outside light - the reference fills first and inspects after - but the regime moves on, and the
    // level beneath it is filled with the inside value and skipped entirely from then on.
    let mut sun = Sunlight::new();
    let lit = sun.step(&roofed_level(), 20.0, 4.0, 1.0);
    assert!(lit.iter().all(|cell| (cell.max() - 20.0).abs() < 1e-6));
    assert!(!sun.fully_outside(), "the roof ended that");
    assert!(sun.fully_inside(), "and nothing gets through it");

    let below = sun.step(&open_level(), 20.0, 4.0, 1.0);
    assert!(
        below.iter().all(|cell| (cell.max() - 4.0).abs() < 1e-6),
        "the level below the roof is filled with the inside light, open air or not"
    );
}

#[test]
fn a_shaft_in_a_ceiling_lands_as_a_bright_patch() {
    // The middle regime. One square of ceiling is open and lit from above; that square takes the light through its centre, so all four quarters, and the square beside
    // it takes the light from the cardinal between them, so only the pair of quarters facing the shaft.
    let mut sun = primed();
    let mut level = roofed_level();
    open_at(&mut level, 2, 2);
    // One lit column above the shaft and dark sky everywhere else, which is what a hole in a ceiling looks like from below.
    let mut above = sky(0.0);
    above.light[2 * SIDE + 2] = 20.0;
    let lit = sun.step_with_above(&level, &above, 20.0, 4.0, 1.0);

    let hole = lit[2 * SIDE + 2];
    assert!(
        (hole.get(Quadrant::NorthEast) - 20.0).abs() < 1e-5,
        "straight down through the shaft, so every quarter: {hole:?}"
    );
    assert!(
        (hole.get(Quadrant::SouthWest) - 20.0).abs() < 1e-5,
        "including the one facing away"
    );

    // The square south of the shaft: its north cardinal is the shaft, so its *northern* quarters catch the light - and its southern ones stay at the indoors value, which is
    // the gradient.
    let beside = lit[2 * SIDE + 3];
    assert!(
        beside.get(Quadrant::NorthEast) > 4.0,
        "the quarter facing the shaft: {beside:?}"
    );
    assert!(
        (beside.get(Quadrant::SouthWest) - 4.0).abs() < 1e-6,
        "and the quarter away from it is left at the inside light: {beside:?}"
    );
}

#[test]
fn light_through_thicker_air_is_dimmed_and_the_clamp_holds_it_below_the_level_above() {
    // `clamp( prev_light_max * LIGHT_TRANSPARENCY_OPEN_AIR / prev_transparency, inside, prev_light_max )` (`lightmap.cpp:419-420`), where the transparency is the
    // **level above's** - so this is a property of the sky, not of the room. Open air passes everything, twice the absorption halves it, and thinner-than-air can never raise
    // a square above the light above it because of the clamp.
    let mut sun = primed();
    let mut open = roofed_level();
    open_at(&mut open, 2, 2);
    let clear = sun.step_with_above(&open, &sky(20.0), 20.0, 4.0, 1.0)[2 * SIDE + 2].max();
    assert!(
        (clear - 20.0).abs() < 1e-5,
        "open air passes the sky: {clear}"
    );

    let mut sun = primed();
    let mut smoky_sky = sky(20.0);
    smoky_sky.transparency[2 * SIDE + 2] = TRANSPARENCY_OPEN_AIR * 2.0;
    let mut room = roofed_level();
    open_at(&mut room, 2, 2);
    let dimmed = sun.step_with_above(&room, &smoky_sky, 20.0, 4.0, 1.0)[2 * SIDE + 2].max();
    assert!(
        (dimmed - 10.0).abs() < 1e-4,
        "twice the absorption halves it: {dimmed}"
    );

    let mut sun = primed();
    let mut thin_sky = sky(20.0);
    thin_sky.transparency[2 * SIDE + 2] = TRANSPARENCY_OPEN_AIR / 2.0;
    let mut thin_room = roofed_level();
    open_at(&mut thin_room, 2, 2);
    let clamped = sun.step_with_above(&thin_room, &thin_sky, 20.0, 4.0, 1.0)[2 * SIDE + 2].max();
    assert!(
        (clamped - 20.0).abs() < 1e-5,
        "and nothing is brighter than the sky above it: {clamped}"
    );
}

#[test]
fn a_square_dark_above_is_lit_from_the_side_instead() {
    // The five-square scan is the square itself first, then north, west, east, south (`lightmap.cpp:384-388`). A square with nothing above it falls back to the cardinals,
    // which is how light spills sideways under a ceiling.
    let mut sun = primed();
    let mut level = roofed_level();
    // Two open squares side by side, so that light can pass from one to the other; the western one is under a dark column and the eastern one under the sky.
    open_at(&mut level, 2, 2);
    open_at(&mut level, 3, 2);
    let mut above = sky(0.0);
    above.light[3 * SIDE + 2] = 20.0;
    let lit = sun.step_with_above(&level, &above, 20.0, 4.0, 1.0);

    assert!(
        (lit[3 * SIDE + 2].max() - 20.0).abs() < 1e-5,
        "the square under the sky takes it straight down"
    );
    let sideways = lit[2 * SIDE + 2];
    assert!(
        sideways.get(Quadrant::SouthEast) > 4.0,
        "and the dark one takes it from the east: {sideways:?}"
    );
    assert!(
        (sideways.get(Quadrant::NorthWest) - 4.0).abs() < 1e-6,
        "leaving the far quarters at the indoors value"
    );
}

#[test]
fn the_square_above_decides_whether_its_light_gets_through() {
    // The reference reads `prev_transparency_cache` and `prev_floor_cache` - the level **above** - for every square it tests (`lightmap.cpp:406-419`), and only
    // `outside_cache` from the level being lit. A port that read the current level's transparency instead would pour daylight through a roof, because the roof is a
    // property of the level above and the floor under it belongs to that level too.
    let mut sun = primed();
    // The level being lit: open air everywhere, nothing translucent.
    let mut level = open_level();
    level.outside = vec![true; SIDE * SIDE];
    open_at(&mut level, 2, 2);
    // The level above: one lit square, and **that square is opaque** on the level above.
    let mut above = AboveLayer {
        transparency: vec![0.0; SIDE * SIDE],
        floor: vec![false; SIDE * SIDE],
        light: vec![20.0; SIDE * SIDE],
    };
    above.transparency[2 * SIDE + 2] = TRANSPARENCY_OPEN_AIR;
    above.floor[2 * SIDE + 2] = true; // a floor above means the sky does not reach the square below

    let lit = sun.step_with_above(&level, &above, 20.0, 4.0, 1.0);
    assert!(
        (lit[2 * SIDE + 2].max() - 4.0).abs() < 1e-6,
        "a roof above the square keeps the sun out: {:?}",
        lit[2 * SIDE + 2]
    );

    // A fresh pass: the first call latched `fully_inside`, which is the reference's own state machine and not something to reset by hand.
    above.floor[2 * SIDE + 2] = false;
    let mut sun = primed();
    let lit = sun.step_with_above(&level, &above, 20.0, 4.0, 1.0);
    assert!(
        lit[2 * SIDE + 2].max() > 4.0,
        "and with the floor above removed the sun gets through: {:?}",
        lit[2 * SIDE + 2]
    );
}
