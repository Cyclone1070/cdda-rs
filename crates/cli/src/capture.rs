//! Drawing the view to a picture.
//!
//! The interface does not exist yet, so what a capture draws is the map view: one cell per square of
//! the frame the simulation already computes, tinted by what stands there and shaded by the light.
//! It is deliberately not a glyph renderer - the text art is `Frame::text()`, and a bitmap font here
//! would be a second thing to keep in step with the data for no gain while there is no screen.
//!
//! A capture has to be *checkable*, so the rules are plain: every square is a cell of the same size,
//! the colour comes from the square's own answers, and the same world always produces the same PNG.

use crate::png::Image;

/// How big one square is drawn, in pixels.
pub const CELL: u32 = 8;

/// What a capture was drawn from, so a caller can say how much it looked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// The picture.
    pub image: Image,
    /// How many squares across the view was.
    pub squares_across: u32,
    /// How many of them had anything but darkness in them.
    pub lit_squares: u32,
}

/// Draws the view around `origin`, one cell per square.
#[must_use]
pub fn draw(world: &mut bevy_ecs::prelude::World, origin: (i32, i32), radius: i32) -> Capture {
    // **The picture is drawn by the world's own light.** `frame` takes the ambient light at the character and the character's vision threshold
    // (`frame.rs:155-159`); passing zero for both drew every square as dark, which made every capture a black square that diffs equal to every other capture.
    // The light comes from the sun at the world's own turn (`astronomy::sun_altitude_degrees` over the calendar), and the threshold from a character of ordinary
    // perception, which is what the reference's draw loop asks of the avatar.
    let turns = cdda_sim::calendar::of(world).map_or(0, |calendar| calendar.turn);
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let altitude = cdda_core_types::astronomy::sun_altitude_degrees(turns as f64) as f32;
    let ambient = cdda_core_types::light::sun_light_from_altitude(altitude);
    let senses = cdda_core_types::light::Senses {
        perception: 10.0,
        night_vision: 0.0,
        bird_eye: false,
        gnv: false,
        limb_night_vis: 1.0,
        enchantment_night_vis: 1.0,
    };
    let threshold = cdda_core_types::light::vision_threshold(ambient, &senses);
    // The light map has to be cast before it can be read: the reference rebuilds the map's caches once per frame and draws from them (`map::build_map_cache`), so the daylight is put on the
    // squares here rather than assumed to be there. Without this every square is dark however bright the sun is, which is how the first version of this capture came out black.
    #[allow(clippy::cast_precision_loss)]
    cdda_sim::cast_sunlight_with(world, turns as f64);
    let frame = cdda_sim::frame::frame(world, origin, radius, ambient, threshold);
    let across = u32::try_from(radius * 2 + 1).unwrap_or(1);
    let mut image = Image::blank(across * CELL, across * CELL, [0, 0, 0, 255]);
    let mut lit_squares = 0;
    for row in 0..=radius * 2 {
        for column in 0..=radius * 2 {
            let Some(cell) = frame.at(origin.0 + column - radius, origin.1 + row - radius) else {
                continue;
            };
            let (cells_x, cells_y) = (
                u32::try_from(column).unwrap_or(0),
                u32::try_from(row).unwrap_or(0),
            );
            let shade = shade_of(cell);
            let colour = [shade[0], shade[1], shade[2], 255];
            image.fill(cells_x * CELL, cells_y * CELL, CELL, CELL, colour);
            if cell.lit != cdda_core_types::light::LitLevel::Blank {
                lit_squares += 1;
            }
            // A remembered square is drawn with a notch in the corner, so the picture says which
            // parts of it the character is not looking at.
            if cell.from_memory {
                image.fill(cells_x * CELL, cells_y * CELL, 2, 2, [90, 40, 90, 255]);
            }
        }
    }
    Capture {
        image,
        squares_across: across,
        lit_squares,
    }
}

/// The colour a square is drawn in: its glyph's own tint, dimmed by how lit it is.
fn shade_of(cell: &cdda_sim::frame::FrameCell) -> [u8; 3] {
    let base = tint_of(cell.glyph);
    let (numerator, denominator) = match cell.lit {
        cdda_core_types::light::LitLevel::Bright => (10_u16, 10_u16),
        cdda_core_types::light::LitLevel::Lit => (8, 10),
        cdda_core_types::light::LitLevel::BrightOnly => (6, 10),
        cdda_core_types::light::LitLevel::Low => (4, 10),
        cdda_core_types::light::LitLevel::Memorized => (3, 10),
        cdda_core_types::light::LitLevel::Dark | cdda_core_types::light::LitLevel::Blank => (0, 10),
    };
    let dim = |channel: u8| {
        let scaled = u16::from(channel) * numerator / denominator;
        u8::try_from(scaled.min(255)).unwrap_or(255)
    };
    [dim(base[0]), dim(base[1]), dim(base[2])]
}

/// A glyph's colour, from the character rather than from a table: walls are grey, floor is brown,
/// water is blue, plants are green, and everything else keeps a tint of its own.
fn tint_of(glyph: char) -> [u8; 3] {
    match glyph {
        '#' => [150, 150, 150],
        '.' | ',' => [120, 90, 60],
        '~' => [40, 90, 180],
        '*' | 'w' | 't' => [40, 140, 60],
        '|' | '-' => [170, 170, 170],
        ' ' => [20, 20, 24],
        other => {
            let code = u32::from(other) & 0xff;
            [
                u8::try_from(60 + (code * 37) % 120).unwrap_or(120),
                u8::try_from(60 + (code * 71) % 120).unwrap_or(120),
                u8::try_from(60 + (code * 113) % 120).unwrap_or(120),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use super::*;
    use bevy_ecs::prelude::*;

    fn world_of(marks: &[(i32, i32, u32)]) -> World {
        let mut world = World::new();
        let mut level = cdda_core_types::level::Level::new();
        for y in 0..12 {
            for x in 0..12 {
                level.set_terrain(x, y, 1, 0);
            }
        }
        for (x, y, code) in marks {
            level.set_terrain(*x, *y, *code, 0);
        }
        cdda_sim::load_level(&mut world, &level);
        world
    }

    #[test]
    fn a_capture_is_one_cell_per_square() {
        let mut world = world_of(&[]);

        let capture = draw(&mut world, (5, 5), 2);

        assert_eq!(capture.squares_across, 5, "a radius of two is five squares across");
        assert_eq!(capture.image.width, 5 * CELL, "and each square is a cell");
        assert_eq!(capture.image.height, 5 * CELL);
    }

    #[test]
    fn the_same_world_draws_the_same_picture() {
        let mut first = world_of(&[(3, 3, 2)]);
        let mut second = world_of(&[(3, 3, 2)]);

        let one = draw(&mut first, (5, 5), 3);
        let two = draw(&mut second, (5, 5), 3);

        assert_eq!(one.image.png(), two.image.png(), "a capture is comparable byte for byte");
    }
}

#[cfg(test)]
mod picture_tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use super::*;
    use bevy_ecs::prelude::*;

    /// A world whose terrain table knows its glyphs: floor is `.` and wall is `#`.
    fn a_room() -> World {
        let mut world = World::new();
        // A capture is of a **game**, and a game is played in daylight: the reference's scenario starts at eight in the morning (`scenario.cpp:138`).
        cdda_sim::calendar::set_turn(&mut world, 8 * 60 * 60);
        let mut level = cdda_core_types::level::Level::new();
        for y in 0..12 {
            for x in 0..12 {
                let edge = x == 0 || y == 0 || x == 11 || y == 11;
                level.set_terrain(x, y, if edge { 2 } else { 1 }, 0);
            }
        }
        let mut terrain = cdda_sim::terrain_table::TerrainTable::default();
        terrain.insert_symbol(1, '.');
        terrain.insert_symbol(2, '#');
        cdda_sim::singletons::set_prototypes(
            &mut world,
            cdda_sim::Prototypes {
                terrain,
                furniture: cdda_sim::terrain_table::TerrainTable::default(),
            },
        );
        cdda_sim::load_level(&mut world, &level);
        world
    }

    /// **A picture of a room shows a room.** The capture used to come out black because the world had no terrain glyphs to draw, and a black picture diffs equal to every other black picture,
    /// which is worse than no picture at all: it looks like a passing screen check.
    #[test]
    fn a_capture_of_a_room_is_not_one_flat_colour() {
        let mut world = a_room();

        let capture = draw(&mut world, (5, 5), 5);

        let mut seen: std::collections::BTreeSet<[u8; 3]> = std::collections::BTreeSet::new();
        for y in 0..capture.image.height {
            for x in 0..capture.image.width {
                let index = usize::try_from((y * capture.image.width + x) * 4).unwrap_or(0);
                if let Some(colour) = capture.image.rgba.get(index..index + 4) {
                    seen.insert([colour[0], colour[1], colour[2]]);
                }
            }
        }
        assert!(
            seen.len() > 1,
            "**the frame shows more than one tint**, and it shows {:?}",
            seen
        );
        assert!(
            seen.iter().any(|colour| colour[0] > 100),
            "and at least one of them is a lit glyph rather than the near-black of an empty one"
        );
    }
}

