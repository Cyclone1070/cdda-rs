//! Derived per-level values, rebuilt only when what they depend on moved.
//!
//! Reference: `map_cache` (`src/map_cache.h`, `src/map_cache.cpp`). The map keeps
//! light, transparency, visibility, floor and outside-ness per cell, derived from
//! terrain that rarely changes. Writing a tile marks the submap dirty; the next read
//! of a dirty submap rebuilds that submap and nothing else.
//!
//! The port keeps the same shape and the same promise: a mark is cheap, and a
//! rebuild happens once per submap between reads, however many tiles changed.

use crate::submap::SubmapId;
use std::collections::{BTreeMap, BTreeSet};

/// One submap's worth of a derived value, cached until something changes.
#[derive(Debug, Clone, Default)]
pub struct MapCache {
    /// Submaps whose values are stale.
    dirty: BTreeSet<SubmapId>,
    /// The values last built, keyed by submap.
    values: BTreeMap<SubmapId, u8>,
    /// How many times a value has been built; the cache's own receipt.
    rebuilds: u32,
}

impl MapCache {
    /// Builds an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a submap stale.
    ///
    /// Marking the same submap twenty times is still one rebuild, which is the
    /// whole point: twenty changed tiles in one submap should not cost twenty
    /// rebuilds, and a cache that did would be correct and useless.
    pub fn invalidate(&mut self, id: SubmapId) {
        self.dirty.insert(id);
    }

    /// Marks every submap stale, for a change that could reach anywhere - loading
    /// a save, or an option that changes visibility for the whole map.
    pub fn invalidate_all(&mut self) {
        for id in self.values.keys() {
            self.dirty.insert(*id);
        }
    }

    /// The value for a submap, building it if the submap is stale.
    ///
    /// `build` is called only when the cached value cannot be trusted, so a caller
    /// can put an expensive computation there without paying for it on every read.
    pub fn value(&mut self, id: SubmapId, build: impl FnOnce(u8) -> u8) -> u8 {
        if self.dirty.remove(&id) || !self.values.contains_key(&id) {
            // The previous value is handed to the builder so a caller can decide for
            // itself whether the new state still resembles the old one.
            let previous = self.values.get(&id).copied().unwrap_or(0);
            let fresh = build(previous);
            self.values.insert(id, fresh);
            self.rebuilds += 1;
            return fresh;
        }
        self.values.get(&id).copied().unwrap_or(0)
    }

    /// Whether a submap is waiting to be rebuilt.
    #[must_use]
    pub fn is_dirty(&self, id: SubmapId) -> bool {
        self.dirty.contains(&id)
    }

    /// How many values have been built since the cache was made.
    #[must_use]
    pub fn rebuilds(&self) -> u32 {
        self.rebuilds
    }

    /// How many submaps hold a cached value.
    #[must_use]
    pub fn cached(&self) -> usize {
        self.values.len()
    }
}

/// One z-level's derived state, and what says it is stale.
///
/// Reference: `struct level_cache` (`src/level_cache.h:20-98`) together with the four functions that mark it (`src/map.cpp:295-345`). The port keeps the marking
/// rules and the two indices that are easy to get wrong; the derived arrays themselves arrive with the systems that compute them.
///
/// Three kinds of mark live here, and they are not interchangeable:
///
/// * the **transparency** bitset, one bit per submap, because transparency is computed a submap at a time and a changed tile spoils only its own;
/// * the **seen** flag, one for the level, because visibility is a property of the whole light map;
/// * the **floor** and **outside** flags, one each, because both are read off terrain and furniture and both are cheap to recompute for a level.
// The reference's struct is four bare `bool`s in a row (`level_cache.h:31-33`) and that is what they are: named, readable, and each asked for by name. Grouping them
// into a bitflags type would be tidier and further from the thing being transcribed.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct LevelCache {
    /// One bit per submap, set when that submap's transparency needs rebuilding.
    transparency_dirty: std::collections::BTreeSet<(i32, i32)>,
    seen_dirty: bool,
    floor_dirty: bool,
    outside_dirty: bool,
    reachability_dirty: bool,
    /// A square's seen value, kept so the marking rule can ask whether it was visible at all.
    seen: std::collections::BTreeMap<(i32, i32), f32>,
}

impl LevelCache {
    /// A cache with nothing marked.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether any submap's transparency is stale.
    #[must_use]
    pub fn transparency_dirty_anywhere(&self) -> bool {
        !self.transparency_dirty.is_empty()
    }

    /// Whether one submap's transparency is stale.
    ///
    /// The submap is named by its position in the map's grid, which is what the reference's bitset is indexed by.
    #[must_use]
    pub fn transparency_dirty_at(&self, submap_x: i32, submap_y: i32) -> bool {
        self.transparency_dirty.contains(&(submap_x, submap_y))
    }

    /// Marks the submap holding a map square, and possibly the reachability cache.
    ///
    /// Reference: `map::set_transparency_cache_dirty( const tripoint_bub_ms &p, bool field )` (`map.cpp:302-311`). The bit is `sm.x() * MAPSIZE + sm.y()`, so the
    /// grid is indexed x-first; and a change that is **not** a field also invalidates reachability, because a wall changes what a creature can walk to and smoke does
    /// not.
    pub fn mark_transparency_dirty_at(&mut self, x: i32, y: i32, field: bool) {
        self.transparency_dirty.insert((
            x.div_euclid(crate::grid::SEEX),
            y.div_euclid(crate::grid::SEEY),
        ));
        if !field {
            self.reachability_dirty = true;
        }
    }

    /// Whether a mask change has invalidated the reachability cache.
    #[must_use]
    pub fn reachability_dirty(&self) -> bool {
        self.reachability_dirty
    }

    /// Marks the whole level stale, as `map::invalidate_map_cache` does (`map.cpp:397-406`).
    pub fn invalidate_all(&mut self) {
        for x in 0..crate::grid::MAPSIZE {
            for y in 0..crate::grid::MAPSIZE {
                self.transparency_dirty.insert((x, y));
            }
        }
        self.floor_dirty = true;
        self.seen_dirty = true;
        self.outside_dirty = true;
    }

    /// Whether the light map has to be recomputed.
    #[must_use]
    pub fn seen_dirty(&self) -> bool {
        self.seen_dirty
    }

    /// Whether the floor cache has to be recomputed.
    #[must_use]
    pub fn floor_dirty(&self) -> bool {
        self.floor_dirty
    }

    /// Whether the outside cache has to be recomputed.
    #[must_use]
    pub fn outside_dirty(&self) -> bool {
        self.outside_dirty
    }

    /// Records what is visible at a square, which is what the marking rule reads.
    pub fn set_seen_value(&mut self, x: i32, y: i32, seen: f32) {
        self.seen.insert((x, y), seen);
    }

    /// Marks the light map stale if this square could have been contributing to it.
    ///
    /// Reference: `map::set_seen_cache_dirty( const tripoint_bub_ms & )` (`map.cpp:313-325`). Two early-outs, and the second is the interesting one: if the square's
    /// seen value **and** its camera value are both zero, nothing was visible there, so a change there cannot change what is seen and the map is left alone.
    pub fn mark_seen_dirty_at(&mut self, x: i32, y: i32) {
        if self.seen_dirty {
            return;
        }
        if self.seen.get(&(x, y)).copied().unwrap_or(0.0) != 0.0 {
            self.seen_dirty = true;
        }
    }
}
