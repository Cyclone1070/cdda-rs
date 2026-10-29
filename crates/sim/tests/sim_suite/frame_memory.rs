//! What the frame draws for a square the character cannot see but remembers.
//!
//! Reference: `map::draw` (`src/map.cpp:6930-6960`). The draw loop asks the square's `lit_level`, turns it into a `visibility_type`, and for `HIDDEN` or `DARK` calls
//! `draw_background`, which prints the square's **memorized symbol** in brown and a blank otherwise (`map.cpp:6906-6917`). A square that is merely unlit is therefore drawn
//! from memory - which is the whole reason the memory exists - while a square that was never seen is a space.
//!
//! The other half of the rule is where the memory comes from: the draw loop memorizes what it draws (`drawsqu_params().memorize( true )`, `map.cpp:7216`), so the memory is
//! filled by looking and read back by not being able to look.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::frame::frame;
use cdda_sim::memory;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Prototypes, cast_lights_with};

const FLOOR: u32 = 1;
const LAMP: u32 = 2;
const GRASS: u32 = 3;

/// Two squares of floor, a lamp to light them, and a patch of grass further out that the first frame sees and the second does not.
fn world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, FLOOR);
    for y in 0..24 {
        for x in 0..24 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    level.set_terrain(4, 4, LAMP, FLOOR);
    for y in 3..6 {
        for x in 8..11 {
            level.set_terrain(x, y, GRASS, GRASS);
        }
    }
    cdda_sim::load_level(&mut world, level);
    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert(LAMP, true);
    terrain.insert(GRASS, true);
    terrain.insert_light(LAMP, 200.0);
    terrain.insert_symbol(FLOOR, '.');
    terrain.insert_symbol(LAMP, 'o');
    terrain.insert_symbol(GRASS, ',');
    cdda_sim::set_prototypes(&mut world, Prototypes {
        terrain,
        furniture: TerrainTable::new(),
    });
    memory(&mut world);
    world
}

#[test]
fn a_square_seen_once_is_drawn_from_memory_when_the_light_goes_out() {
    let mut world = world();
    // The first frame: the lamp lights the grass, and the draw loop memorizes what it prints.
    cast_lights_with(&mut world);
    let first = frame(&mut world, (4, 4), 12, 1.0, 3.0);
    assert_eq!(
        first.at(9, 4).expect("the grass").glyph,
        ',',
        "the first frame prints the terrain"
    );
    assert!(
        !first.at(9, 4).expect("the grass").from_memory,
        "and prints it live, not from memory"
    );

    // The second frame: no light at all, so the grass is unlit - `DARK` in the reference's terms - and the draw path reaches for the memory.
    cdda_sim::clear_light(&mut world);
    let second = frame(&mut world, (4, 4), 12, 1.0, 3.0);
    let cell = second.at(9, 4).expect("the grass");
    assert!(cell.from_memory, "the square is drawn from memory");
    assert_eq!(cell.glyph, ',', "and with the glyph it was memorized with");
}

#[test]
fn a_square_never_seen_is_a_space() {
    let mut world = world();
    cdda_sim::clear_light(&mut world);
    let dark = frame(&mut world, (4, 4), 12, 1.0, 3.0);
    let cell = dark.at(9, 4).expect("the grass");
    assert!(!cell.from_memory, "nothing was ever memorized");
    assert_eq!(cell.glyph, ' ', "so the square is a space");
}

#[test]
fn a_lit_square_is_printed_live_and_never_from_memory() {
    let mut world = world();
    cast_lights_with(&mut world);
    let drawn = frame(&mut world, (4, 4), 12, 1.0, 3.0);
    let cell = drawn.at(4, 4).expect("the lamp's own square");
    assert!(!cell.from_memory, "a square the character can see");
    assert_eq!(cell.glyph, 'o');
}
