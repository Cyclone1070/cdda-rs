//! Scent as the level holds it: one grid, marked by the terrain's own flags.
//!
//! The diffusion itself is `cdda_core_types::scent::ScentWindow`, which is the transcribed arithmetic and is pinned by its own six tests; this module is the layer that
//! decides *which squares are marked*, and the marks come from the loaded terrain flags rather than from a caller with a marker pen.
//!
//! Reference: `scent_map` (`src/scent_map.cpp`) and its two verbs - `update` (the diffusion) and `decay` (one point off every square, called once a turn from
//! `map::decay_fields_and_scent`, `src/map.cpp:3417-3420`).
//!
//! The grid is a component on the one entity [`crate::TheScent`] marks, so it is a thing in the world
//! like any other. Reading does not create it: a world with no scent entity simply has no scent.

use crate::{Map, TheScent};
use bevy_ecs::prelude::*;
use cdda_core_types::scent::ScentCell;

/// The scent standing on every square of the level.
#[derive(Debug, Clone, Default, Component)]
pub struct Scent {
    /// The level's extent, in tiles.
    pub side: usize,
    /// One cell per square, row by row with x fastest, which is the order the diffuser walks.
    pub cells: Vec<ScentCell>,
}

/// The scent grid, if the world has made one.
#[must_use]
pub fn scent_of(world: &World) -> Option<&Scent> {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<Scent>())
}

/// The scent grid, made empty if the world has none.
///
/// # Panics
///
/// Panics if the entity found does not carry the grid, which cannot happen: it is found by having
/// it, and spawned with it.
pub fn scent_mut(world: &mut World) -> Mut<'_, Scent> {
    let entity = crate::singletons::singleton::<Scent, TheScent>(world);
    match world.get_mut::<Scent>(entity) {
        Some(scent) => scent,
        None => unreachable!("the scent entity was just spawned with its grid"),
    }
}

/// Builds the grid for the level, with every square marked from its terrain.
///
/// A world with no level has no extent, so the grid is empty - which is the honest answer, not a
/// default that quietly pretends the level is one square.
#[must_use]
pub fn scent_grid(world: &mut World) -> Scent {
    let mut sides = std::collections::BTreeMap::new();
    {
        let mut query = world.query_filtered::<&Map, ()>();
        if let Some(map) = query.iter(world).next() {
            sides.insert(0, crate::bounds_of_public(&map.0));
        }
    }
    let Some(&side) = sides.get(&0) else {
        return Scent::default();
    };
    let table = crate::singletons::scent_table(world).cloned();
    let mut terrain = vec![0_u32; side * side];
    {
        let mut query = world.query_filtered::<&Map, ()>();
        if let Some(map) = query.iter(world).next() {
            for y in 0..side {
                for x in 0..side {
                    let (Ok(xi), Ok(yi)) = (i32::try_from(x), i32::try_from(y)) else {
                        continue;
                    };
                    if let (Some(code), Some(slot)) =
                        (map.0.terrain_at(xi, yi), terrain.get_mut(y * side + x))
                    {
                        *slot = code;
                    }
                }
            }
        }
    }
    let mut cells = vec![ScentCell::default(); side * side];
    for (cell, code) in cells.iter_mut().zip(terrain) {
        let (blocks, reduces) = table.as_ref().map_or((false, false), |table| table.marks(code));
        cell.blocks = blocks;
        cell.reduces = reduces;
    }
    Scent { side, cells }
}

/// The scent on a square, or zero where the level has not made one.
#[must_use]
pub fn scent_at(world: &World, square: (i32, i32)) -> i32 {
    let Some(scent) = scent_of(world) else {
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
    let Some(scent) = scent_of(world) else {
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
    let mut scent = scent_mut(world);
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

/// One turn of the reference's scent step: the character deposits, then the window diffuses.
///
/// Reference: `do_turn.cpp:642-648`, which is the first thing a turn does - `scent.set( u.pos, u.scent, type )` and then `scent.update( u.pos, m )`, both **before** the fields
/// are processed. The window is the character's own radius, because that is the square of the world the reference tracks (`scent_map` is written through the window, not
/// through the whole bubble).
///
/// The marks come from the level's terrain through [`crate::scent_table::ScentTable`], so a wall that the data flags `NO_SCENT` holds the smell in without being told.
pub fn advance_scent(world: &mut World, center: (i32, i32), deposited: i32) {
    let mut window = cdda_core_types::scent::ScentWindow::around(center);
    {
        let Some(scent) = scent_of(world) else {
            return;
        };
        // Everything the map already holds inside the window comes back in, which is what makes the scent stay put in the world rather than belong to the character.
        for y in window.min_y()..=window.max_y() {
            for x in window.min_x()..=window.max_x() {
                let Some(square) = square_index(scent, x, y) else {
                    continue;
                };
                let Some(cell) = scent.cells.get(square) else {
                    continue;
                };
                window.set(x, y, cell.value);
                if cell.blocks {
                    window.block(x, y);
                }
                if cell.reduces {
                    window.reduce(x, y);
                }
            }
        }
    }
    if deposited > 0 {
        window.set(center.0, center.1, deposited);
    }
    window.diffuse();

    let mut scent = scent_mut(world);
    for y in window.min_y()..=window.max_y() {
        for x in window.min_x()..=window.max_x() {
            let value = window.get(x, y);
            let Some(square) = square_index(&scent, x, y) else {
                continue;
            };
            if let Some(cell) = scent.cells.get_mut(square) {
                cell.value = value;
            }
        }
    }
}

/// Where a world square sits in the grid, if the level covers it.
fn square_index(scent: &Scent, x: i32, y: i32) -> Option<usize> {
    let column = usize::try_from(x).ok()?;
    let row = usize::try_from(y).ok()?;
    if column >= scent.side || row >= scent.side {
        return None;
    }
    Some(row * scent.side + column)
}

/// One point off every square that has any.
///
/// Reference: `scent_map::decay` (`scent_map.cpp:60-67`), called once a turn from `map::decay_fields_and_scent` (`map.cpp:3417-3420`).
pub fn decay_scent(world: &mut World) {
    if let Some(mut scent) = scent_of(world).cloned() {
        for cell in &mut scent.cells {
            cell.value = (cell.value - 1).max(0);
        }
        *scent_mut(world) = scent;
    }
}

/// Builds the level's scent grid and puts it in the world, which is what a turn does before it diffuses anything.
pub fn reset_scent(world: &mut World) {
    let grid = scent_grid(world);
    *scent_mut(world) = grid;
}
