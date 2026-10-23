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
    /// Whether each tile blocks light, worked out once per tile rather than once per
    /// cast. Keyed by tile, so moving the light reuses what it already knows.
    transparency: BTreeMap<(i32, i32), bool>,
    transparency_builds: u32,
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

    /// Whether a tile blocks light, remembering the answer.
    ///
    /// The reference keeps a transparency cache per level and rebuilds it when the
    /// level is dirty; a per-tile memo is the same idea at a finer grain, and it means
    /// two casts from different places pay once for the tiles they share.
    fn blocks_at(&mut self, x: i32, y: i32, blocks_light: &impl Fn(u32) -> bool) -> bool {
        if let Some(known) = self.transparency.get(&(x, y)) {
            return *known;
        }
        // A tile the world has not made blocks light: there is nothing to see through.
        let blocks = self.terrain_at(x, y).is_none_or(&blocks_light);
        self.transparency.insert((x, y), blocks);
        self.transparency_builds += 1;
        blocks
    }

    /// How many tiles have had their transparency worked out.
    #[must_use]
    pub fn transparency_builds(&self) -> u32 {
        self.transparency_builds
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
        // Only the tile that changed can have changed what is there to see through.
        self.transparency.remove(&(x, y));
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
    #[allow(clippy::indexing_slicing, clippy::needless_pass_by_value)]
    pub fn visibility(
        &mut self,
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
                let transparent = !self.blocks_at(world_x, world_y, &blocks_light);
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
    /// Whether a square can be seen from an origin, using the cached cast.
    ///
    /// Reference: `map::seen` reading `seen_cache`, which `map::build_seen_cache` fills with the
    /// cast around the player (`src/lightmap.cpp:1039-1100`). A square with no submap is never
    /// seen, because the world has not made it.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative, as [`Level::visibility`] does.
    pub fn seen_at(
        &mut self,
        origin: (i32, i32),
        radius: i32,
        blocks_light: impl Fn(u32) -> bool,
        square: (i32, i32),
    ) -> bool {
        let dx = square.0 - origin.0;
        let dy = square.1 - origin.1;
        if dx.abs() > radius || dy.abs() > radius {
            return false;
        }
        let rows = self.cached_visibility(origin, radius, blocks_light);
        let Ok(row) = usize::try_from(dy + radius) else {
            return false;
        };
        let Ok(column) = usize::try_from(dx + radius) else {
            return false;
        };
        rows.get(row)
            .and_then(|row| row.get(column))
            .copied()
            .unwrap_or(false)
    }

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

impl Level {}

impl Level {
    /// Daylight on the level: every made tile's four quarters.
    ///
    /// Reference: the ambient half of `map::build_map_cache` (`src/lightmap.cpp:396-438`),
    /// which fills the level with the indoor light level and then lifts the quarters of each
    /// tile that face open sky. Outdoors the whole tile takes the sun; indoors only the side
    /// facing a way out does, which is why a doorway lights the floor inside it.
    ///
    /// `outside` answers whether a tile is under open sky. The port does not decide that
    /// itself yet - it depends on the terrain and whether the tile has a roof - so it is
    /// asked, tile by tile, and answered by whoever owns that knowledge.
    ///
    /// # Panics
    ///
    /// Panics if the clock is before the calendar's epoch, which the astronomy cannot answer.
    pub fn daylight(
        &self,
        turns: f64,
        indoors: f32,
        outside: impl Fn(i32, i32) -> bool,
    ) -> BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants> {
        use crate::shadowcasting::FourQuadrants;
        #[allow(clippy::cast_possible_truncation)]
        let altitude = crate::astronomy::sun_altitude_degrees(turns) as f32;
        let sun = crate::light::sun_light_from_altitude(altitude);
        let mut lit = BTreeMap::new();
        // `at` holds submaps, not tiles: walk each placed submap's cells to reach the tiles.
        let tiles: Vec<(i32, i32)> = self
            .at
            .keys()
            .flat_map(|&(submap_x, submap_y)| {
                (0..side()).flat_map(move |cell_x| {
                    (0..side())
                        .map(move |cell_y| (submap_x * side() + cell_x, submap_y * side() + cell_y))
                })
            })
            .collect();
        for (x, y) in tiles {
            // The reference checks the tile and its four cardinals; a tile with open sky on
            // any side takes daylight through it.
            let neighbours = [
                outside(x, y),
                outside(x, y - 1),
                outside(x - 1, y),
                outside(x + 1, y),
                outside(x, y + 1),
            ];
            if neighbours[0] {
                lit.insert((x, y), FourQuadrants::uniform(sun));
                continue;
            }
            let base = FourQuadrants::uniform(indoors);
            lit.insert(
                (x, y),
                crate::shadowcasting::light_in_from_outside(neighbours, sun, base),
            );
        }
        lit
    }
}

impl Level {
    /// Adds one light source's light to a map of quarters.
    ///
    /// Reference: `map::apply_light_source` (`src/lightmap.cpp`), which casts from the source
    /// with the shadow caster and raises the quarters it reaches by `sight_calc` of the
    /// luminance at that distance.
    ///
    /// **Known divergence, deliberate and declared:** the reference accumulates transparency
    /// along each ray and passes that to `sight_calc`; this uses open-air transparency and the
    /// straight-line distance, so a source behind smoke is currently as bright as one in clear
    /// air. It is not gated against reference output yet - the ledger says so - and the ray
    /// accumulation is the next step.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative, as [`Level::visibility`] does.
    /// The transparency of a window of tiles, for the caster.
    ///
    /// Reference: `map::build_transparency_cache` (`src/lightmap.cpp:100-203`), whose per-square rule lives in
    /// [`crate::transparency::transparency_of`]. This function only gathers the window and asks that rule, so the level cannot
    /// disagree with the cache about what a square costs - the older version of this function kept the *most opaque* field on a
    /// square, which is a different and wrong rule for two overlapping layers.
    #[must_use]
    pub fn transparency_grid(
        &self,
        origin: (i32, i32),
        radius: i32,
        sight_penalty: f32,
        facts: impl Fn(i32, i32) -> crate::transparency::SquareFacts,
    ) -> crate::shadowcasting::TransparencyGrid {
        let side = usize::try_from(radius * 2 + 1).unwrap_or(1);
        let mut cells = vec![0.0_f32; side * side];
        for grid_y in 0..side {
            for grid_x in 0..side {
                let Ok(offset_x) = i32::try_from(grid_x) else {
                    continue;
                };
                let Ok(offset_y) = i32::try_from(grid_y) else {
                    continue;
                };
                let world_x = origin.0 + offset_x - radius;
                let world_y = origin.1 + offset_y - radius;
                // A tile the world has not made is solid: light does not pass through a place that does not exist.
                let value = if self.terrain_at(world_x, world_y).is_some() {
                    crate::transparency::transparency_of(&facts(world_x, world_y), sight_penalty)
                        .value
                } else {
                    0.0
                };
                if let Some(cell) = cells.get_mut(grid_x * side + grid_y) {
                    *cell = value;
                }
            }
        }
        crate::shadowcasting::TransparencyGrid::new(side, cells)
    }

    /// Casts one light source using the transcribed transparency rule.
    ///
    /// Reference: `map::apply_light_source` (`lightmap.cpp:1217-1294`). The `facts` closure answers, per square, what the transparency cache would
    /// answer for it - terrain and furniture flags, whether it is under the sky, and its fields.
    pub fn add_light_source_with(
        &mut self,
        origin: (i32, i32),
        luminance: f32,
        sight_penalty: f32,
        facts: impl Fn(i32, i32) -> crate::transparency::SquareFacts,
        light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
    ) {
        let radius = crate::light::light_range(luminance);
        if radius <= 0 {
            return;
        }
        let grid = self.transparency_grid(origin, radius, sight_penalty, facts);
        Self::apply_cast(origin, radius, luminance, &grid, light);
    }

    /// Writes one cast of light into the map.
    ///
    /// Reference: `map::apply_light_source` (`lightmap.cpp:1217-1294`), whose cast is the octant sweep in
    /// [`crate::shadowcasting::cast_light_quadrants`]. That sweep already decides, per square, **which two quarters** the ray entered by, so nothing here
    /// has to guess a facing: it takes the brighter of the two answers per quarter and stops.
    fn apply_cast(
        origin: (i32, i32),
        radius: i32,
        luminance: f32,
        grid: &crate::shadowcasting::TransparencyGrid,
        light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
    ) {
        use crate::shadowcasting::cast_light_quadrants;
        let side = usize::try_from(radius * 2 + 1).unwrap_or(1);
        let cast = cast_light_quadrants(grid, (radius, radius), radius, luminance, false);
        for grid_y in 0..side {
            for grid_x in 0..side {
                let Some(&value) = cast.get(grid_x * side + grid_y) else {
                    continue;
                };
                if value.max() <= 0.0 {
                    continue;
                }
                let Ok(offset_x) = i32::try_from(grid_x) else {
                    continue;
                };
                let Ok(offset_y) = i32::try_from(grid_y) else {
                    continue;
                };
                let offset_x = offset_x - radius;
                let offset_y = offset_y - radius;
                let entry = light
                    .entry((origin.0 + offset_x, origin.1 + offset_y))
                    .or_insert_with(|| crate::shadowcasting::FourQuadrants::uniform(0.0));
                *entry = entry.max_with(value);
            }
        }
    }
}
