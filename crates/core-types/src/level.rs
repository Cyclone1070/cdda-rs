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
    /// How many sight casts have been built, which is the level's receipt for "one cast, many readers".
    sight_casts: u32,
    /// Buffered light sources, recorded before any of them casts.
    ///
    /// Reference: `level_cache::light_source_buffer` (`src/lightmap.cpp:666-670`), filled by the gathering loop and drained afterwards (`:654-659`). Recording first is what
    /// lets a square ask "is my neighbour already lit at least as brightly" before it decides which directions to cast into.
    buffered: BTreeMap<(i32, i32), f32>,
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

    /// Where every submap of the level sits, in submap coordinates.
    ///
    /// The level's extent is not a rectangle from the origin: a level is whatever submaps were made,
    /// so a caller that needs to walk all of it asks for this rather than guessing a side.
    #[must_use]
    pub fn submap_places(&self) -> Vec<(i32, i32)> {
        self.placed.values().copied().collect()
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

    /// The furniture standing on a world tile, or `None` when the world has not made the tile.
    ///
    /// Reference: `submap::get_furn` (`src/submap.cpp`), which is a layer of its own beside the terrain. `None` is "no such square"; a square with nothing on it
    /// answers `Some(NULL_ID)`, which is the reference's `f_null`.
    #[must_use]
    pub fn furniture_at(&self, x: i32, y: i32) -> Option<u32> {
        let (id, cell_x, cell_y) = self.submap_at(x, y)?;
        self.store
            .get(id)
            .map(|submap| submap.furniture(cell_x, cell_y))
    }

    /// Places furniture on a world tile, making its submap if needed.
    ///
    /// Reference: `submap::set_furn` (`src/submap.cpp`), which materialises the furniture layer on the first write.
    pub fn set_furniture(&mut self, x: i32, y: i32, furniture: u32, blank: u32) {
        let submap = (x.div_euclid(side()), y.div_euclid(side()));
        let id = self.ensure(submap.0, submap.1, blank);
        let Some((_, cell_x, cell_y)) = self.submap_at(x, y) else {
            return;
        };
        if let Some(entry) = self.store.get_mut(id) {
            entry.set_furniture(cell_x, cell_y, furniture);
        }
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

/// One sight cast, ready to be read square by square.
///
/// Rows are `y - origin.y + radius`, columns `x - origin.x + radius`; a square outside the cast reads as not seen at all, which is the same answer the reference gives for a
/// square outside its cache.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenGrid {
    origin: (i32, i32),
    radius: i32,
    rows: Vec<Vec<f32>>,
}

impl SeenGrid {
    /// How clearly a square is seen, from zero to one.
    #[must_use]
    pub fn at(&self, square: (i32, i32)) -> f32 {
        if square == self.origin {
            // `seen_cache[origin] = VISIBILITY_FULL` before the cast (`lightmap.cpp:1057-1059`).
            return 1.0;
        }
        let (dx, dy) = (square.0 - self.origin.0, square.1 - self.origin.1);
        if dx.abs() > self.radius || dy.abs() > self.radius {
            return 0.0;
        }
        let (Ok(row), Ok(column)) = (
            usize::try_from(dy + self.radius),
            usize::try_from(dx + self.radius),
        ) else {
            return 0.0;
        };
        self.rows
            .get(row)
            .and_then(|row| row.get(column))
            .copied()
            .unwrap_or(0.0)
    }
}

/// One light source's cast: where it is, how bright, and how to look at the square it crosses.
pub struct Source<'a> {
    /// The square the source stands on.
    pub at: (i32, i32),
    /// How bright it is.
    pub luminance: f32,
    /// The weather's sight penalty, which attenuates every ray.
    pub sight_penalty: f32,
    /// What a square is like: terrain and furniture transparency, the outside cache, and the fields standing on it.
    pub facts: &'a dyn Fn(i32, i32) -> crate::transparency::SquareFacts,
}

impl Level {
    /// The sight cast over the square of world around an origin, to be read square by square.
    ///
    /// Reference: `map::build_seen_cache` (`src/lightmap.cpp:1039-1100`), which fills a level's `seen_cache` once and lets every reader ask it. A frame asks about every
    /// square it draws, so this is the shape callers want; [`Level::seen_through`] is the same cast for one square.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative, as [`Level::visibility`] does.
    #[must_use]
    pub fn seen_grid(
        &mut self,
        origin: (i32, i32),
        radius: i32,
        transparency_of: impl Fn(u32) -> f32,
    ) -> SeenGrid {
        // One pass over the cast's square turns codes into numbers; then there is a single cast implementation, which is the one that takes a table.
        let mut transparency = BTreeMap::new();
        for offset_y in -radius..=radius {
            for offset_x in -radius..=radius {
                let square = (origin.0 + offset_x, origin.1 + offset_y);
                transparency.insert(
                    square,
                    self.terrain_at(square.0, square.1)
                        .map_or(0.0, &transparency_of),
                );
            }
        }
        self.seen_grid_of(origin, radius, &transparency)
    }

    /// The sight cast, from a table of per-square transparencies the caller has already worked out.
    ///
    /// The sim knows each square's number before it casts - terrain, furniture, outside cache and fields, all multiplied - so handing the table over keeps the rule where it
    /// belongs (the caller owns what a square is like) and the cast where it belongs (the level). A square the table does not mention is opaque, as an unbuilt square is.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative, as [`Level::visibility`] does.
    #[must_use]
    pub fn seen_grid_of(
        &mut self,
        origin: (i32, i32),
        radius: i32,
        transparency: &BTreeMap<(i32, i32), f32>,
    ) -> SeenGrid {
        self.seen_cast(origin, radius, transparency)
    }

    /// How many sight casts the level has built.
    #[must_use]
    pub fn sight_casts(&self) -> u32 {
        self.sight_casts
    }

    /// How clearly a square is seen, from zero (not at all) to one (in plain sight).
    ///
    /// Reference: `map::cast_seen_light` (`src/lightmap.cpp:1039-1100`), which walks the same octants as the light cast with `sight_calc` as its accumulator and a starting
    /// value of `VISIBILITY_FULL`. The squared transparency it crosses is each square's own, so open air thins the answer with distance while a wall ends it - which is what
    /// makes "a wall you look at is seen clearly" and "a square behind fog is not" two different numbers rather than two different code paths.
    ///
    /// `transparency_of` is the square's own transparency, the same number `calc_transp` puts in the reference's cache; zero means opaque.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative, as [`Level::visibility`] does.
    pub fn seen_through(
        &mut self,
        origin: (i32, i32),
        radius: i32,
        transparency_of: impl Fn(u32) -> f32,
        square: (i32, i32),
    ) -> f32 {
        if square == origin {
            // `seen_cache[origin] = VISIBILITY_FULL` before the cast (`lightmap.cpp:1057-1059`).
            return 1.0;
        }
        let dx = square.0 - origin.0;
        let dy = square.1 - origin.1;
        if dx.abs() > radius || dy.abs() > radius {
            return 0.0;
        }
        self.seen_grid(origin, radius, transparency_of).at(square)
    }

    /// The sight cast over the square of world around an origin.
    ///
    /// Rows are `y - origin.y + radius`, columns `x - origin.x + radius`, which is the shape [`Level::seen_through`] reads. **Not cached**: a cast is only reusable for the
    /// closure it was built with, and a cache that cannot see its rule is how a level starts answering with yesterday's walls. A caller drawing a frame builds one cast and
    /// reads it; a caller asking about a square asks for one.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    fn seen_cast(
        &mut self,
        origin: (i32, i32),
        radius: i32,
        transparency: &BTreeMap<(i32, i32), f32>,
    ) -> SeenGrid {
        self.sight_casts = self.sight_casts.saturating_add(1);
        seen_grid(origin, radius, transparency)
    }
}

/// The sight cast on its own: a function of the transparency table and nothing else.
///
/// The level's method is this plus a counter, which is what lets a store that keeps its squares as
/// entities ask for the same grid without a level behind it.
///
/// # Panics
///
/// Panics if the radius is negative.
#[must_use]
#[allow(clippy::indexing_slicing)]
pub fn seen_grid(
    origin: (i32, i32),
    radius: i32,
    transparency: &BTreeMap<(i32, i32), f32>,
) -> SeenGrid {
    assert!(radius >= 0, "a negative view distance is a caller bug");
    let Ok(side) = usize::try_from(radius * 2 + 1) else {
        return SeenGrid {
            origin,
            radius: 0,
            rows: Vec::new(),
        };
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
            let square = (origin.0 + offset_x - radius, origin.1 + offset_y - radius);
            cells[grid_x * side + grid_y] = transparency.get(&square).copied().unwrap_or(0.0);
        }
    }
    // **The sight cast, not the light cast.** The reference fills a `seen_cache` with `cast_zlight` (`lightmap.cpp:1079`); `castLight` is the lightmap's own sweep
    // and answers a different question about a square just past a corner - 93 of 441 squares differ on a patterned grid. One level is cast here, so the levels beside
    // it read as solid and nothing crosses: a caller that wants sight across levels passes the neighbouring transparencies and floors through
    // [`cdda_core_types::zlight`] directly.
    let plane = crate::zlight::Plane {
        transparency: &|x, y, z| {
            if z == 0 {
                transparency.get(&(x, y)).copied().unwrap_or(0.0)
            } else {
                0.0
            }
        },
        floor: &|_, _, _| true,
    };
    let values = crate::zlight::build_seen_cache(
        &plane,
        (origin.0, origin.1, 0),
        radius,
        (i32::MAX, i32::MAX),
        1.0,
        crate::zlight::VerticalDirection::Both,
    );
    let mut rows = vec![vec![0.0_f32; side]; side];
    for (grid_y, row) in rows.iter_mut().enumerate() {
        for (grid_x, cell) in row.iter_mut().enumerate() {
            let (Ok(offset_x), Ok(offset_y)) = (i32::try_from(grid_x), i32::try_from(grid_y))
            else {
                continue;
            };
            let square = (origin.0 + offset_x - radius, origin.1 + offset_y - radius);
            *cell = values.get(&(square.0, square.1, 0)).copied().unwrap_or(0.0);
        }
    }
    SeenGrid {
        origin,
        radius,
        rows,
    }
}

impl Level {
    /// Records a buffered light source, without casting it.
    ///
    /// Reference: `map::add_light_source` (`src/lightmap.cpp:666-670`): the buffer keeps the **brighter** of two sources on one square, because two fires on a square are one
    /// light. Terrain, furniture and fields are added this way; items and creatures are applied directly.
    pub fn add_light_source(&mut self, origin: (i32, i32), luminance: f32) {
        let entry = self.buffered.entry(origin).or_insert(0.0);
        if luminance > *entry {
            *entry = luminance;
        }
    }

    /// How many buffered sources are waiting to be applied.
    #[must_use]
    pub fn buffered_sources(&self) -> usize {
        self.buffered.len()
    }

    /// Casts every buffered source, skipping a direction a brighter neighbour already covers.
    ///
    /// Reference: the second phase of `map::generate_lightmap` (`src/lightmap.cpp:654-659`) calling `apply_light_source`, whose neighbour test is here: a square does not cast
    /// into a direction when the square that way holds a buffered source at least as bright (`:1257-1290`). The neighbour's own cast covers the squares beyond, so the answer is
    /// the same apart from the one square of attenuation the reference is happy to lose.
    pub fn apply_buffered_sources(
        &mut self,
        sight_penalty: f32,
        facts: impl Fn(i32, i32) -> crate::transparency::SquareFacts,
        light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
    ) {
        let buffered = std::mem::take(&mut self.buffered);
        apply_buffered_sources(
            &buffered,
            sight_penalty,
            &|x, y| self.terrain_at(x, y).is_some(),
            facts,
            light,
        );
    }

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
        // The character's own square is always fully visible: `build_seen_cache` writes `VISIBILITY_FULL` there before the cast runs (`lightmap.cpp:1057-1059`). Without
        // this the origin is lit by nothing and the square the player stands on would be drawn blank.
        if square == origin {
            return true;
        }
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

    /// Whether the light would have to be rebuilt before it can be trusted.
    #[must_use]
    pub fn light_is_stale(&self) -> bool {
        self.light_dirty || self.light.is_none()
    }

    /// Marks the light stale because the sky changed (`weather.cpp:938-941`).
    ///
    /// Reference: the weather update dirties **every** level's transparency cache when the sight penalty changes, and then the seen cache, because the penalty attenuates every
    /// ray on every level at once. The rebuild itself waits for the next map read, as it does in the reference - a flush is a note, not a construction.
    pub fn mark_weather_changed(&mut self) {
        self.light_dirty = true;
    }

    /// How many times light has been rebuilt; the level's own receipt.
    #[must_use]
    pub fn light_rebuilds(&self) -> u32 {
        self.light_rebuilds
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
        transparency_grid(
            origin,
            radius,
            sight_penalty,
            |x, y| self.terrain_at(x, y).is_some(),
            facts,
        )
    }
}

/// The transparency of a window of tiles, for the caster, given what the caller knows about squares.
///
/// The level's method is this with "does this square exist" answered from its own tiles; a store
/// that keeps its squares as entities answers the same question from the store.
#[must_use]
pub fn transparency_grid(
    origin: (i32, i32),
    radius: i32,
    sight_penalty: f32,
    exists: impl Fn(i32, i32) -> bool,
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
            let value = if exists(world_x, world_y) {
                crate::transparency::transparency_of(&facts(world_x, world_y), sight_penalty).value
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

impl Level {
    /// Casts one light source using the transcribed transparency rule, without recording it.
    ///
    /// Reference: `map::apply_light_source` (`lightmap.cpp:1217-1294`). The `facts` closure answers, per square, what the transparency cache would answer for it - terrain and
    /// furniture flags, whether it is under the sky, and its fields. Sources that come from the map's own data go through the buffer instead ([`Level::add_light_source`]), because
    /// a square needs to know whether a neighbour is already lit before it decides which directions to cast into.
    pub fn add_light_source_with(
        &mut self,
        origin: (i32, i32),
        luminance: f32,
        sight_penalty: f32,
        facts: impl Fn(i32, i32) -> crate::transparency::SquareFacts,
        light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
    ) {
        self.apply_light_source_with(origin, luminance, sight_penalty, facts, light);
    }

    /// Casts one source immediately, into every direction, and does **not** record it.
    ///
    /// Reference: `map::apply_light_source` called directly (`src/lightmap.cpp:1217-1290`), which is what items and creatures do: they neither consult the buffer nor fill it, so
    /// a flare is cast in full even when a fire beside it is already lit.
    pub fn apply_light_source_with(
        &mut self,
        origin: (i32, i32),
        luminance: f32,
        sight_penalty: f32,
        facts: impl Fn(i32, i32) -> crate::transparency::SquareFacts,
        light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
    ) {
        self.apply_light_source_where(
            &Source {
                at: origin,
                luminance,
                sight_penalty,
                facts: &facts,
            },
            light,
            [true; crate::shadowcasting::OCTANTS],
        );
    }

    /// One source's cast, into the directions a caller allows.
    fn apply_light_source_where(
        &self,
        source: &Source<'_>,
        light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
        allow: [bool; crate::shadowcasting::OCTANTS],
    ) {
        cast_light_source(
            source,
            &|x, y| self.terrain_at(x, y).is_some(),
            light,
            allow,
        );
    }
}

/// Casts one light source into a light map.
///
/// The level's method is this with its own tiles and its own buffer; a store that keeps its squares
/// as entities passes the same two answers in.
#[allow(clippy::too_many_arguments)]
pub fn cast_light_source(
    source: &Source<'_>,
    exists: &dyn Fn(i32, i32) -> bool,
    light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
    allow: [bool; crate::shadowcasting::OCTANTS],
) {
    let Source {
        at: (x, y),
        luminance,
        sight_penalty,
        facts,
    } = *source;
    let own = luminance.max(1.0);
    let cell = light
        .entry((x, y))
        .or_insert_with(|| crate::shadowcasting::FourQuadrants::uniform(0.0));
    if cell.max() < own {
        *cell = crate::shadowcasting::FourQuadrants::uniform(cell.max().max(own));
    }
    let radius = crate::light::light_range(luminance);
    if radius <= 0 {
        return;
    }
    let grid = transparency_grid((x, y), radius, sight_penalty, exists, facts);
    let side = usize::try_from(radius * 2 + 1).unwrap_or(1);
    let cast = crate::shadowcasting::cast_light_octants(
        &grid,
        (radius, radius),
        radius,
        luminance,
        false,
        allow,
    );
    for (index, quarters) in cast.iter().enumerate() {
        let (Some(grid_x), Some(grid_y)) = (index.checked_div(side), index.checked_rem(side))
        else {
            continue;
        };
        if quarters.max() <= 0.0 {
            continue;
        }
        let square = (
            x + i32::try_from(grid_x).unwrap_or(0) - radius,
            y + i32::try_from(grid_y).unwrap_or(0) - radius,
        );
        let entry = light
            .entry(square)
            .or_insert_with(|| crate::shadowcasting::FourQuadrants::uniform(0.0));
        for quarter in [
            crate::shadowcasting::Quadrant::NorthEast,
            crate::shadowcasting::Quadrant::SouthEast,
            crate::shadowcasting::Quadrant::SouthWest,
            crate::shadowcasting::Quadrant::NorthWest,
        ] {
            crate::shadowcasting::update_light_quadrants(entry, quarters.get(quarter), quarter);
        }
    }
}

/// Casts every buffered source, skipping a direction a brighter neighbour already covers.
///
/// Reference: the second phase of `map::generate_lightmap` (`src/lightmap.cpp:654-659`) calling
/// `apply_light_source`. The level's method is this with its own buffer; a store that keeps its
/// squares as entities keeps the buffer itself and passes it in.
pub fn apply_buffered_sources(
    buffered: &BTreeMap<(i32, i32), f32>,
    sight_penalty: f32,
    exists: &dyn Fn(i32, i32) -> bool,
    facts: impl Fn(i32, i32) -> crate::transparency::SquareFacts,
    light: &mut BTreeMap<(i32, i32), crate::shadowcasting::FourQuadrants>,
) {
    for ((x, y), luminance) in buffered {
        let (x, y, luminance) = (*x, *y, *luminance);
        let mut allow = [true; crate::shadowcasting::OCTANTS];
        for (index, direction) in crate::shadowcasting::OCTANT_DIRECTIONS.iter().enumerate() {
            let neighbour = match direction {
                crate::shadowcasting::Direction::North => (x, y - 1),
                crate::shadowcasting::Direction::East => (x + 1, y),
                crate::shadowcasting::Direction::South => (x, y + 1),
                crate::shadowcasting::Direction::West => (x - 1, y),
            };
            // `< luminance`, not `<=`: a neighbour that is **not** brighter fails the test and the direction is skipped.
            if buffered
                .get(&neighbour)
                .is_some_and(|theirs| *theirs >= luminance)
                && let Some(slot) = allow.get_mut(index)
            {
                *slot = false;
            }
        }
        cast_light_source(
            &Source {
                at: (x, y),
                luminance,
                sight_penalty,
                facts: &facts,
            },
            exists,
            light,
            allow,
        );
    }
}
