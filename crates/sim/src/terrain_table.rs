//! What each terrain costs a ray of light, taken from the loaded prototypes.
//!
//! Reference: `calc_transp` in `map::build_transparency_cache` (`src/lightmap.cpp:140-150`), which asks the terrain one question - is it `transparent` - and answers
//! `LIGHT_TRANSPARENCY_SOLID` or the open-air value. There is no middle: a transparent terrain costs nothing and an opaque one stops the ray.
//!
//! The table exists because the level stores terrain as a **code**, and the code means whatever the thing that built the level decided. So the mapping from code to
//! transparency is the caller's, and this type is where it is kept - one place, so that the light and the vision cache cannot disagree about a window.

use std::collections::BTreeMap;

use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;

/// Which codes let light through, for either layer of a square.
///
/// The reference keeps terrain and furniture in two id spaces but asks them the same question, so one type serves both: a `TerrainTable` for terrain codes and another for
/// furniture codes. Keeping them as separate instances is what stops code 3 meaning two different things.
///
/// A code the table has never been told about is treated as **open air**, which is what the sim did for every code but zero before the loader existed. An code that is
/// known to be opaque stops the ray. Codes and their prototypes are matched up by the caller: nothing here guesses that code 2 is `t_wall`.
#[derive(Debug, Clone, Default)]
pub struct TerrainTable {
    transparent: BTreeMap<u32, bool>,
    indoors: BTreeMap<u32, bool>,
}

impl TerrainTable {
    /// An empty table, in which every code is open air.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that a code is, or is not, see-through.
    pub fn insert(&mut self, code: u32, transparent: bool) {
        self.transparent.insert(code, transparent);
    }

    /// Builds a table from `(code, transparent)` pairs, which is how a loader's answer is handed over.
    pub fn from_prototypes(pairs: impl IntoIterator<Item = (u32, bool)>) -> Self {
        Self {
            transparent: pairs.into_iter().collect(),
            indoors: BTreeMap::new(),
        }
    }

    /// Records that a code is, or is not, flagged as indoors.
    ///
    /// Reference: `TFLAG_INDOORS` on either layer, read by `build_outside_cache` (`map.cpp:9291-9293`) to decide which squares are under open sky. A code the table has not
    /// been told about is **under the sky**, which is what an unflagged terrain is.
    pub fn insert_indoors(&mut self, code: u32, indoors: bool) {
        self.indoors.insert(code, indoors);
    }

    /// Whether the code carries the indoors flag.
    #[must_use]
    pub fn is_indoors(&self, code: u32) -> bool {
        self.indoors.get(&code).copied().unwrap_or(false)
    }

    /// How much light the terrain costs a ray crossing it.
    ///
    /// Reference: `calc_transp` (`lightmap.cpp:144-150`). A transparent terrain is exactly the open-air value - not a little less - and an opaque one is zero, which the
    /// cast reads as `sight_check` failing and treats as a wall.
    #[must_use]
    pub fn transparency(&self, code: u32) -> f32 {
        match self.transparent.get(&code) {
            Some(false) => 0.0,
            _ => TRANSPARENCY_OPEN_AIR,
        }
    }

    /// Whether the table has been told about this code at all.
    #[must_use]
    pub fn knows(&self, code: u32) -> bool {
        self.transparent.contains_key(&code)
    }
}
