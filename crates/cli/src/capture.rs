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
    let frame = cdda_sim::frame::frame(world, origin, radius, 0.0, 0.0);
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
