//! Sight limited by light: the two halves the reference joins when it draws a frame.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::level::Level;
use cdda_core_types::shadowcasting::FourQuadrants;
use cdda_core_types::vision::{apparent_light_at, shown_at};
use std::collections::BTreeMap;

const OPEN: u32 = 1;
const WALL: u32 = 2;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, OPEN);
    level
}

fn blocks(terrain: u32) -> bool {
    terrain == WALL
}

fn lit(squares: &[(i32, i32)], luminance: f32) -> BTreeMap<(i32, i32), FourQuadrants> {
    let mut light = BTreeMap::new();
    for square in squares {
        light.insert(*square, FourQuadrants::uniform(luminance));
    }
    light
}

#[test]
fn a_square_in_sight_and_in_light_is_shown() {
    let mut level = level();
    let light = lit(&[(5, 6)], 30.0);
    assert!(shown_at(&mut level, (5, 5), 5, blocks, &light, (5, 6)));
}

#[test]
fn a_square_in_sight_but_in_the_dark_is_not_shown() {
    let mut level = level();
    let light = BTreeMap::new();
    assert!(
        !shown_at(&mut level, (5, 5), 5, blocks, &light, (5, 6)),
        "the cast reaches it, but there is nothing to see by"
    );
}

#[test]
fn a_lit_square_behind_a_wall_is_not_shown() {
    let mut level = level();
    for y in 0..12 {
        level.set_terrain(9, y, WALL, OPEN);
    }
    let light = lit(&[(10, 5)], 30.0);
    assert!(
        !shown_at(&mut level, (5, 5), 5, blocks, &light, (10, 5)),
        "light on the far side does not help"
    );
}

#[test]
fn the_brightest_quadrant_is_the_one_that_counts() {
    let mut light = BTreeMap::new();
    let mut quadrants = FourQuadrants::uniform(1.0);
    quadrants.set(cdda_core_types::shadowcasting::Quadrant::SouthEast, 42.0);
    light.insert((3, 3), quadrants);
    assert!((apparent_light_at(&light, (3, 3)) - 42.0).abs() < 1e-6);
    assert!(
        apparent_light_at(&light, (3, 4)).abs() < 1e-6,
        "an unlit square is zero"
    );
}
