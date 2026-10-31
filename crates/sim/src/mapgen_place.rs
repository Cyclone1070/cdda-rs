//! Putting a drawn mapgen into the world.
//!
//! Reference: `mapgen::draw_map` (`mapgen.cpp`): a mapgen draws a chunk of symbols into a grid of terrain ids, and the map then has that terrain
//! where the chunk was placed. The content crate draws (`cdda_content::mapgen::draw_chunk`); this module writes the result into the one store, so a
//! chunk of map is entities carrying `Terrain` rather than an array beside them.

use bevy_ecs::prelude::*;

/// Writes a drawn chunk into the world at an origin.
///
/// A square the chunk leaves empty keeps whatever the world already had there: the reference's `fill_ter` covers the chunk's own squares, and
/// nothing is written over a neighbour's. Answers how many squares were written, so a caller can tell an empty chunk from a full one.
pub fn place_drawn(
    world: &mut World,
    drawn: &[Vec<Option<String>>],
    origin: (i32, i32, i32),
    code_of: &dyn Fn(&str) -> Option<u32>,
) -> usize {
    let mut written = 0;
    for (row, line) in drawn.iter().enumerate() {
        for (column, terrain) in line.iter().enumerate() {
            let Some(terrain) = terrain else {
                continue;
            };
            let Some(code) = code_of(terrain) else {
                continue;
            };
            let (Some(column), Some(row)) = (offset(column), offset(row)) else {
                continue;
            };
            crate::square::set_terrain(world, (origin.0 + column, origin.1 + row, origin.2), code);
            written += 1;
        }
    }
    written
}

/// A chunk's column or row as a world offset, or nothing when it is too large to be one.
fn offset(index: usize) -> Option<i32> {
    i32::try_from(index).ok()
}

/// A drawn chunk's size, which is the data's business rather than the port's.
#[must_use]
pub fn drawn_extent(drawn: &[Vec<Option<String>>]) -> (usize, usize) {
    let width = drawn.iter().map(Vec::len).max().unwrap_or(0);
    (width, drawn.len())
}
