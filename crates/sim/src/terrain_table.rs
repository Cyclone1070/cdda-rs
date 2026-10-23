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
    light: BTreeMap<u32, f32>,
    symbols: BTreeMap<u32, char>,
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
            light: BTreeMap::new(),
            symbols: BTreeMap::new(),
        }
    }

    /// Records that a code is, or is not, flagged as indoors.
    ///
    /// Reference: `TFLAG_INDOORS` on either layer, read by `build_outside_cache` (`map.cpp:9291-9293`) to decide which squares are under open sky. A code the table has not
    /// been told about is **under the sky**, which is what an unflagged terrain is.
    pub fn insert_indoors(&mut self, code: u32, indoors: bool) {
        self.indoors.insert(code, indoors);
    }

    /// Records the glyph the code is drawn with.
    ///
    /// Reference: `map_data_common_t::sym` (`src/mapdata.h`), loaded by [`crate::terrain_table::TerrainTable::from_terrains`]. A code the table has not been told about is a
    /// space, which is what the reference draws for open air.
    pub fn insert_symbol(&mut self, code: u32, symbol: char) {
        self.symbols.insert(code, symbol);
    }

    /// The glyph a code is drawn with.
    #[must_use]
    pub fn symbol(&self, code: u32) -> char {
        self.symbols.get(&code).copied().unwrap_or(' ')
    }

    /// Marks every code that either layer calls opaque as opaque.
    ///
    /// Reference: `map::calc_transp` (`lightmap.cpp:146-149`) multiplies the two layers' transparencies per square, so a square is opaque when **either** layer is. Merging by code
    /// is the same statement for a frame that draws one level: the numbers are per code, and the more absorbing layer decides.
    pub fn merge_absorbing_from(&mut self, other: &Self) {
        let opaque: Vec<u32> = other
            .transparent
            .iter()
            .filter(|(_, transparent)| !**transparent)
            .map(|(code, _)| *code)
            .collect();
        for code in opaque {
            self.transparent.insert(code, false);
        }
    }

    /// Records how much light the code emits.
    ///
    /// Reference: `map_data_common_t::light_emitted` (`src/mapdata.h`), which `map::generate_lightmap` turns into a light source on every square of that terrain
    /// (`lightmap.cpp:527-533`). Twenty-two terrains in this corpus emit light, all of them at 120.
    pub fn insert_light(&mut self, code: u32, light: f32) {
        self.light.insert(code, light);
    }

    /// The light a code emits, zero when it emits none.
    #[must_use]
    pub fn light(&self, code: u32) -> f32 {
        self.light.get(&code).copied().unwrap_or(0.0)
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
