//! Which terrain marks a square for scent, read from the loaded prototypes.
//!
//! Reference: `TFLAG_NO_SCENT` and `TFLAG_REDUCE_SCENT` (`src/mapdata.h:181-193`), read by `scent_map::update` (`src/scent_map.cpp:208-258`). The port used to make the
//! caller mark squares by hand, which let a wall's flag and the mark disagree; the flags are in the terrain dump, so the marks can come from the data like every other
//! terrain answer.

use std::collections::BTreeMap;

/// The two scent flags of each terrain code.
///
/// A component on the one entity [`TheScentTable`] marks: a table the world loaded is a thing the
/// world holds, not a side channel beside the store.
#[derive(Debug, Clone, Default, bevy_ecs::prelude::Component)]
pub struct ScentTable {
    by_code: BTreeMap<u32, (bool, bool)>,
}

impl ScentTable {
    /// An empty table: no square is marked, which is what a world with no loaded terrain deserves.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records whether a code blocks scent or only reduces it.
    pub fn insert(&mut self, code: u32, blocks: bool, reduces: bool) {
        self.by_code.insert(code, (blocks, reduces));
    }

    /// The marks for a code, as the diffuser wants them.
    ///
    /// A code the table has not been told about is plain air, which is the reference's default: only the two flags change anything.
    #[must_use]
    pub fn marks(&self, code: u32) -> (bool, bool) {
        self.by_code.get(&code).copied().unwrap_or((false, false))
    }

    /// Whether the code is known at all.
    #[must_use]
    pub fn knows(&self, code: u32) -> bool {
        self.by_code.contains_key(&code)
    }
}
