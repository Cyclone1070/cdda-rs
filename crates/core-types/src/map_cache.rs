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
