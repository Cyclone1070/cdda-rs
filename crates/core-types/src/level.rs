//! A level of the map: tiles addressed by world coordinate, held in submaps.
//!
//! Reference: `map` (`src/map.h`) over the map buffer. The port keeps the reference's
//! division of labour - submaps own the tiles, the level owns the submaps and turns a
//! world coordinate into the cell inside one - and drops the pointers.

use crate::shadowcasting::{TransparencyGrid, cast_light};
use crate::submap::{SubmapId, SubmapStore};
use std::collections::BTreeMap;

/// A submap's width as a coordinate, for the division that finds one.
/// A submap's width as a coordinate, for the division that finds one.
///
/// The port keeps this in step with [`crate::submap::SUBMAP_SIDE`] by construction; the
/// test below fails if they ever disagree.
const SUBMAP_SIDE_I32: i32 = 12;

fn side() -> i32 {
    SUBMAP_SIDE_I32
}

/// One level of the map.
///
/// Submaps are made on demand: the level knows which ones it has, not which ones could
/// exist, so reading an ungenerated coordinate answers nothing rather than inventing a
/// world.
#[derive(Debug, Clone, Default)]
pub struct Level {
    store: SubmapStore,
    /// Where each submap sits, in submap coordinates.
    placed: BTreeMap<SubmapId, (i32, i32)>,
    /// The other way round, for turning a coordinate into a submap.
    at: BTreeMap<(i32, i32), SubmapId>,
    /// The light last built, and what it was built for.
    light: Option<Vec<Vec<bool>>>,
    light_key: Option<((i32, i32), i32)>,
    /// Whether something has changed since the light was built.
    light_dirty: bool,
    light_rebuilds: u32,
}

impl Level {
    /// Builds an empty level.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the submap at a submap coordinate if it is not made, and returns its id.
    pub fn ensure(&mut self, submap_x: i32, submap_y: i32, uniform_terrain: u32) -> SubmapId {
        if let Some(id) = self.at.get(&(submap_x, submap_y)) {
            return *id;
        }
        let id = self.store.add(uniform_terrain);
        self.placed.insert(id, (submap_x, submap_y));
        self.at.insert((submap_x, submap_y), id);
        id
    }

    /// The submap holding a world tile, if it has been made.
    #[must_use]
    pub fn submap_at(&self, x: i32, y: i32) -> Option<(SubmapId, usize, usize)> {
        let submap = (x.div_euclid(side()), y.div_euclid(side()));
        let id = *self.at.get(&submap)?;
        let cell_x = usize::try_from(x.rem_euclid(side())).ok()?;
        let cell_y = usize::try_from(y.rem_euclid(side())).ok()?;
        Some((id, cell_x, cell_y))
    }

    /// The terrain at a world tile, if the tile's submap has been made.
    #[must_use]
    pub fn terrain_at(&self, x: i32, y: i32) -> Option<u32> {
        let (id, cell_x, cell_y) = self.submap_at(x, y)?;
        self.store
            .get(id)
            .map(|submap| submap.terrain(cell_x, cell_y))
    }

    /// Sets the terrain at a world tile, making its submap if needed.
    ///
    /// The terrain the new submap starts as is the one the caller passes, because a
    /// submap is made by whoever first needs it and only they know what the world is
    /// made of there.
    ///
    /// # Panics
    ///
    /// Panics if the submap was made and then could not be found, which cannot happen:
    /// `ensure` inserts it before returning.
    pub fn set_terrain(&mut self, x: i32, y: i32, terrain: u32, blank: u32) {
        let submap = (x.div_euclid(side()), y.div_euclid(side()));
        let id = self.ensure(submap.0, submap.1, blank);
        let Some((_, cell_x, cell_y)) = self.submap_at(x, y) else {
            return;
        };
        if let Some(entry) = self.store.get_mut(id) {
            entry.set_terrain(cell_x, cell_y, terrain);
        }
        // The reference marks the level dirty on the same write. Until the light is
        // rebuilt, anything read from it describes the world before this call.
        self.light_dirty = true;
    }

    /// Which world tiles an origin can see, given which terrain blocks light.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative, because a negative view distance is a caller
    /// bug rather than something to interpret.
    ///
    /// The grid handed to the caster is built from the level's tiles and covers one map
    /// square per coordinate from `origin - radius` to `origin + radius`, so shadows
    /// cross submap boundaries the way they do in play. A coordinate with no submap is
    /// solid: light does not pass through a place the world has not made.
    // The loops below run over the grid's own dimensions, so every index is inside it.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn visibility(
        &self,
        origin: (i32, i32),
        radius: i32,
        blocks_light: impl Fn(u32) -> bool,
    ) -> Vec<Vec<bool>> {
        let Ok(side) = usize::try_from(radius * 2 + 1) else {
            return Vec::new();
        };
        let mut cells = vec![0.0_f32; side * side];
        for grid_y in 0..side {
            for grid_x in 0..side {
                let Ok(offset_x) = i32::try_from(grid_x) else {
                    continue;
                };
                let Ok(offset_y) = i32::try_from(grid_y) else {
                    continue;
                };
                let (offset_x, offset_y) = (offset_x - radius, offset_y - radius);
                let world_x = origin.0 + offset_x;
                let world_y = origin.1 + offset_y;
                let transparent = self
                    .terrain_at(world_x, world_y)
                    .is_some_and(|terrain| !blocks_light(terrain));
                cells[grid_x * side + grid_y] = if transparent { 1.0 } else { 0.0 };
            }
        }
        let grid = TransparencyGrid::new(side, cells);
        let visible = cast_light(&grid, (radius, radius), radius);
        let mut rows = vec![vec![false; side]; side];
        for grid_y in 0..side {
            for grid_x in 0..side {
                rows[grid_y][grid_x] = visible[grid_x * side + grid_y];
            }
        }
        rows
    }
}

impl Level {
    /// The visibility last built, if it can still be trusted.
    ///
    /// The reference rebuilds a level's light when the level is dirty
    /// (`map::build_map_cache`), not once per query, so this keeps one result and the
    /// coordinates it belongs to.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative, as [`Level::visibility`] does.
    pub fn cached_visibility(
        &mut self,
        origin: (i32, i32),
        radius: i32,
        blocks_light: impl Fn(u32) -> bool,
    ) -> &Vec<Vec<bool>> {
        let key = (origin, radius);
        let stale = self.light_dirty || self.light.is_none() || self.light_key != Some(key);
        if stale {
            let built = self.visibility(origin, radius, blocks_light);
            self.light = Some(built);
            self.light_key = Some(key);
            self.light_dirty = false;
            self.light_rebuilds += 1;
        }
        // Both branches leave a value in place: either it was just built, or it was
        // already there and the staleness check above said so.
        match &self.light {
            Some(rows) => rows,
            None => unreachable!("the light was just built"),
        }
    }

    /// How many times light has been rebuilt; the level's own receipt.
    #[must_use]
    pub fn light_rebuilds(&self) -> u32 {
        self.light_rebuilds
    }
}
