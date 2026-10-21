//! What the character can see: the map's `seen_cache` in miniature.
//!
//! Reference: `map::seen` over the cache `map::build_seen_cache` fills (`src/lightmap.cpp:1039-1100`).

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::level::Level;

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

#[test]
fn an_open_square_next_to_the_origin_is_seen() {
    let mut level = level();
    assert!(level.seen_at((5, 5), 5, blocks, (5, 6)));
    assert!(level.seen_at((5, 5), 5, blocks, (6, 6)), "diagonals too");
}

#[test]
fn a_wall_hides_what_is_behind_it() {
    let mut level = level();
    for y in 0..12 {
        level.set_terrain(9, y, WALL, OPEN);
    }
    assert!(
        level.seen_at((5, 5), 5, blocks, (8, 5)),
        "the wall itself is seen"
    );
    assert!(
        !level.seen_at((5, 5), 5, blocks, (10, 5)),
        "the square behind it is not"
    );
}

#[test]
fn nothing_beyond_the_sight_range_is_seen() {
    let mut level = level();
    assert!(
        !level.seen_at((5, 5), 5, blocks, (11, 5)),
        "six squares out of five"
    );
    assert!(
        !level.seen_at((5, 5), 5, blocks, (5, 40)),
        "and far away is definitely not"
    );
}
