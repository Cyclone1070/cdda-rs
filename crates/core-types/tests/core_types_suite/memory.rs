//! Map memory: what stays on the character's map after the light goes out.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::level::Level;
use cdda_core_types::memory::{MapMemory, RememberedLight, remember_seen};
use cdda_core_types::shadowcasting::FourQuadrants;
use std::collections::BTreeMap;

const OPEN: u32 = 1;
const WALL: u32 = 2;
const GRASS: u32 = 3;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, OPEN);
    level
}

// A square the world has made, with one patch of grass to tell remembered terrain apart. The Option
// is what the memory API asks for: a square with no submap has no terrain to remember.
#[allow(clippy::unnecessary_wraps)]
fn terrain(x: i32, y: i32) -> Option<u32> {
    Some(if (x, y) == (5, 6) { GRASS } else { OPEN })
}

fn blocks(terrain: u32) -> bool {
    terrain == WALL
}

fn light(squares: &[(i32, i32)], luminance: f32) -> BTreeMap<(i32, i32), FourQuadrants> {
    squares
        .iter()
        .map(|square| (*square, FourQuadrants::uniform(luminance)))
        .collect()
}

#[test]
fn what_is_seen_is_remembered_with_its_terrain() {
    let mut level = level();
    let mut memory = MapMemory::new();
    let lit = light(&[(5, 5), (5, 6)], 30.0);
    let written = remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &lit, terrain);
    assert!(written > 0);
    // The origin is not lit by its own cast - the reference's shadowcasting does not light the
    // square it starts from - so the neighbour is the square to check.
    let here = memory.at((5, 6)).expect("the neighbour is remembered");
    assert_eq!(here.terrain, GRASS, "terrain travels with the memory");
    assert_eq!(here.light, RememberedLight::Lit);
}

#[test]
fn the_memory_survives_the_light_going_out_and_stays_dim() {
    let mut level = level();
    let mut memory = MapMemory::new();
    let lit = light(&[(5, 6)], 30.0);
    remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &lit, terrain);
    assert_eq!(
        memory.at((5, 6)).map(|r| r.light),
        Some(RememberedLight::Lit)
    );

    // Next turn there is no light at all: the square is still in sight, so it is remembered, but dim.
    let dark = BTreeMap::new();
    remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &dark, terrain);
    assert_eq!(
        memory.at((5, 6)).map(|r| r.light),
        Some(RememberedLight::Dim),
        "the square is remembered, and dark now"
    );
    assert_eq!(
        memory.at((5, 6)).map(|r| r.terrain),
        Some(GRASS),
        "with its terrain intact"
    );
}

#[test]
fn what_is_behind_a_wall_never_enters_the_memory() {
    let mut level = level();
    for y in 0..12 {
        level.set_terrain(9, y, WALL, OPEN);
    }
    let mut memory = MapMemory::new();
    let lit = light(&[(10, 5)], 30.0);
    remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &lit, terrain);
    assert!(memory.at((10, 5)).is_none(), "not seen, not remembered");
    assert!(memory.at((9, 5)).is_some(), "the wall itself is on the map");
}

#[test]
fn an_empty_memory_is_empty() {
    let memory = MapMemory::new();
    assert!(memory.is_empty());
    assert_eq!(memory.len(), 0);
    assert!(memory.at((0, 0)).is_none());
}
