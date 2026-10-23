//! The frame: what the port would draw for one view, square by square.
//!
//! Reference: the draw path's per-cell work (`map::draw`, `cata_tiles`/curses), which asks three things about a square - what is there, how brightly it is lit, and whether
//! the character can see it - and prints the glyph only when the answer is not blank.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::light::LitLevel;
use cdda_sim::frame::frame;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Light, Map, Prototypes, cast_lights_with};

const FLOOR: u32 = 1;
const WALL: u32 = 2;
const LAMP: u32 = 3;

fn world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, FLOOR);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    for y in 0..12 {
        level.set_terrain(6, y, WALL, FLOOR);
    }
    level.set_terrain(4, 4, LAMP, FLOOR);
    world.insert_resource(Map(level));

    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert(WALL, false);
    terrain.insert(LAMP, true);
    terrain.insert_light(LAMP, 120.0);
    terrain.insert_symbol(FLOOR, '.');
    terrain.insert_symbol(WALL, '#');
    terrain.insert_symbol(LAMP, 'o');
    world.insert_resource(Prototypes {
        terrain,
        furniture: TerrainTable::new(),
    });
    world
}

#[test]
fn a_frame_prints_the_glyph_of_every_square_it_can_see() {
    let mut world = world();
    cast_lights_with(&mut world);
    let drawn = frame(&mut world, (4, 5), 6, 10.0, 3.0);

    assert_eq!(drawn.at(4, 4).expect("the lamp").glyph, 'o');
    assert_eq!(drawn.at(5, 5).expect("a floor").glyph, '.');
    assert_eq!(drawn.at(6, 5).expect("a wall").glyph, '#');
}

#[test]
fn a_square_out_of_sight_is_blank_however_lit_it_is() {
    let mut world = world();
    cast_lights_with(&mut world);
    let drawn = frame(&mut world, (4, 5), 6, 10.0, 3.0);

    let wall = drawn.at(6, 5).expect("the wall itself");
    assert_ne!(wall.lit, LitLevel::Blank, "the wall is seen as a wall");
    let behind = drawn.at(8, 5).expect("behind the wall");
    assert_eq!(behind.lit, LitLevel::Blank, "nothing behind it is shown");
    assert!(behind.seen == 0.0, "and the sight cast agrees");
}

#[test]
fn the_lamp_lights_the_room_the_frame_draws() {
    let mut world = world();
    cast_lights_with(&mut world);
    let drawn = frame(&mut world, (4, 5), 6, 10.0, 3.0);

    assert_eq!(
        drawn.at(4, 4).expect("the lamp").lit,
        LitLevel::Bright,
        "a source is bright at its own square"
    );
    assert!(
        matches!(
            drawn.at(3, 5).expect("beside the lamp").lit,
            LitLevel::Bright | LitLevel::Lit
        ),
        "and the room beside it is lit: {:?}",
        drawn.at(3, 5).map(|cell| cell.lit)
    );
}

#[test]
fn a_frame_is_one_sight_cast() {
    let mut world = world();
    cast_lights_with(&mut world);
    let before = world.resource::<Map>().0.sight_casts();
    let _ = frame(&mut world, (4, 5), 6, 10.0, 3.0);
    // The map resource is taken and put back by the frame, so the count is read through the world again.
    assert_eq!(
        world.resource::<Map>().0.sight_casts() - before,
        1,
        "one cast for the whole view"
    );
}

#[test]
fn the_frame_prints_rows_of_glyphs() {
    let mut world = world();
    cast_lights_with(&mut world);
    let drawn = frame(&mut world, (4, 5), 2, 10.0, 3.0);
    let text = drawn.levels();
    assert_eq!(text.lines().count(), 5, "five rows for a radius of two");
    assert!(
        text.lines().all(|row| row.chars().count() == 5),
        "five columns: {text}"
    );
    let _ = world.resource::<Light>();
}

#[test]
fn the_level_string_shows_a_lit_room_and_the_shadow_behind_the_wall() {
    // The picture a frame gate compares, spelled out square by square: the lamp's own square is bright, the room beside it is lit, and the square behind the wall is blank
    // because the sight cast does not reach it. This is what "the port draws the same world" means when the comparison is text rather than pixels.
    let mut world = world();
    cast_lights_with(&mut world);
    let drawn = frame(&mut world, (4, 5), 4, 10.0, 3.0);
    let levels = drawn.levels();

    let rows: Vec<&str> = levels.lines().collect();
    assert_eq!(rows.len(), 9);
    // Row 1 is y = 2, column 3 is x = 3: the lamp stands at (4, 4), which is row 2, column 4.
    assert_eq!(
        rows[2].chars().nth(4),
        Some('B'),
        "the lamp's own square:\n{levels}"
    );
    // **A lamp of 120 is bright, not merely lit.** The measured answer is that every square the lamp reaches clears `SOURCE_BRIGHT`, so the room reads `B` all the way to the
    // wall - the expectation this test first wrote down (a floor "lit" rather than bright) was wrong about the data, not about the code.
    assert_eq!(
        rows[4].chars().nth(6),
        Some('B'),
        "the wall the lamp reaches is seen and lit:\n{levels}"
    );
    assert_eq!(
        rows[4].chars().nth(7),
        Some('.'),
        "and nothing behind it is shown:\n{levels}"
    );
}

#[test]
fn the_glyph_string_prints_what_is_there_rather_than_how_lit_it_is() {
    // `Frame::text` is the other half of the picture: `levels` says what the character can tell about a square, `text` says what the square is. A gate can diff either, and the
    // one that catches a wrong glyph is this one.
    let mut world = world();
    cast_lights_with(&mut world);
    let drawn = frame(&mut world, (4, 5), 4, 10.0, 3.0);
    let text = drawn.text();
    let rows: Vec<&str> = text.lines().collect();
    assert_eq!(rows.len(), 9);
    assert_eq!(rows[2].chars().nth(4), Some('o'), "the lamp:\n{text}");
    assert_eq!(rows[4].chars().nth(5), Some('.'), "a floor:\n{text}");
    assert_eq!(rows[4].chars().nth(2), Some('#'), "the wall two squares west:\n{text}");
}
