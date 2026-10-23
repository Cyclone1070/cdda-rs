//! Scent as the map holds it: one grid for the level, marked by its own terrain.
//!
//! The diffusion itself is `cdda_core_types::scent::ScentWindow`, which is the transcribed arithmetic and is pinned by its own six tests; this module is the layer that
//! decides *which squares are marked*, and the marks come from the loaded terrain flags rather than from a caller with a marker pen.
//!
//! Reference: `scent_map` (`src/scent_map.cpp`) and its two verbs - `update` (the diffusion) and `decay` (one point off every square, called once a turn from
//! `map::decay_fields_and_scent`, `src/map.cpp:3417-3420`). The arithmetic is transcribed in `cdda_core_types::scent`; this module is the layer that decides which squares
//! are marked, and the marks come from the loaded terrain flags rather than from a caller with a marker pen.

use crate::scent_table::ScentTable;
use crate::Map;
use bevy_ecs::prelude::*;
use cdda_core_types::scent::ScentCell;

/// The scent standing on every square of the level.
#[derive(Debug, Clone, Default, Resource)]
pub struct Scent {
    /// The level's extent, in tiles.
    pub side: usize,
    /// One cell per square, row by row with x fastest, which is the order the diffuser walks.
    pub cells: Vec<ScentCell>,
}

/// Builds the grid for the level, with every square marked from its terrain.
#[must_use]
pub fn scent_grid(world: &World) -> Scent {
    let Some(map) = world.get_resource::<Map>() else {
        return Scent::default();
    };
    let table = world.get_resource::<ScentTable>();
    let side = crate::bounds_of_public(&map.0);
    let mut cells = vec![ScentCell::default(); side * side];
    for y in 0..side {
        for x in 0..side {
            let Ok(xi) = i32::try_from(x) else { continue };
            let Ok(yi) = i32::try_from(y) else { continue };
            let code = map.0.terrain_at(xi, yi).unwrap_or(0);
            let (blocks, reduces) = table.map_or((false, false), |table| table.marks(code));
            if let Some(cell) = cells.get_mut(y * side + x) {
                cell.blocks = blocks;
                cell.reduces = reduces;
            }
        }
    }
    Scent { side, cells }
}

/// The scent on a square, or zero where the level has not made one.
#[must_use]
pub fn scent_at(world: &World, square: (i32, i32)) -> i32 {
    let Some(scent) = world.get_resource::<Scent>() else {
        return 0;
    };
    let (Ok(x), Ok(y)) = (usize::try_from(square.0), usize::try_from(square.1)) else {
        return 0;
    };
    if x >= scent.side || y >= scent.side {
        return 0;
    }
    scent
        .cells
        .get(y * scent.side + x)
        .map_or(0, |cell| cell.value)
}

/// The two marks on a square: whether it blocks, and whether it only reduces.
#[must_use]
pub fn marks_at(world: &World, square: (i32, i32)) -> (bool, bool) {
    let Some(scent) = world.get_resource::<Scent>() else {
        return (false, false);
    };
    let (Ok(x), Ok(y)) = (usize::try_from(square.0), usize::try_from(square.1)) else {
        return (false, false);
    };
    if x >= scent.side || y >= scent.side {
        return (false, false);
    }
    scent
        .cells
        .get(y * scent.side + x)
        .map_or((false, false), |cell| (cell.blocks, cell.reduces))
}

/// Puts scent on a square, which is what a creature standing there does.
pub fn set_scent_at(world: &mut World, square: (i32, i32), value: i32) {
    let Some(mut scent) = world.get_resource_mut::<Scent>() else {
        return;
    };
    let (Ok(x), Ok(y)) = (usize::try_from(square.0), usize::try_from(square.1)) else {
        return;
    };
    if x >= scent.side || y >= scent.side {
        return;
    }
    let side = scent.side;
    if let Some(cell) = scent.cells.get_mut(y * side + x) {
        cell.value = value;
    }
}

/// One point off every square that has any.
///
/// Reference: `scent_map::decay` (`scent_map.cpp:60-67`), called once a turn from `map::decay_fields_and_scent` (`map.cpp:3417-3420`).
pub fn decay_scent(world: &mut World) {
    if let Some(mut scent) = world.get_resource_mut::<Scent>() {
        for cell in &mut scent.cells {
            cell.value = (cell.value - 1).max(0);
        }
    }
}

/// Builds the level's scent grid and puts it in the world, which is what a turn does before it diffuses anything.
pub fn reset_scent(world: &mut World) {
    let grid = scent_grid(world);
    world.insert_resource(grid);
}
