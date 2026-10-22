//! Submaps, and the one store that owns them.
//!
//! Reference: `src/mapbuffer.h` and `src/submap.h`. The reference owns every submap
//! in the map buffer and hands the map raw pointers into it; the port owns them in
//! one store and hands out ids. `docs/03-decisions.md` records why: a pointer is a
//! lifetime the whole simulation would have to carry, and a submap has to outlive
//! every view of it.

use crate::tiles::TilePage;

/// A submap is twelve map squares across and twelve down.
///
/// This is `Scale for Submap`'s `MAP_SQUARES_PER`, named here so map code that
/// walks a submap does not have to remember it.
pub const SUBMAP_SIDE: usize = 12;

/// A stable name for one submap.
///
/// It is an index into the store's vector, not a pointer: adding submaps may move
/// the storage, and an id keeps naming the same submap afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubmapId(usize);

impl SubmapId {
    /// Names the submap at an index.
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// The index this id names.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// The id a tile holds when it holds nothing: `t_null`, `f_null` or `tr_null`, depending on the layer.
///
/// Reference: the factories' null prototypes (`generic_factory.h` creates one per type). The port's id table assigns index zero to each type's null prototype, which
/// is why a fresh submap - one that has never been written - answers with this and not with a missing value.
pub const NULL_ID: u32 = 0;

/// One submap: five layers of per-square data, each a page that stays cheap while nothing has been written to it.
///
/// Reference: `maptile_soa` (`submap.h:62-72`). The reference stores terrain, furniture, luminance, items, fields, traps and radiation; the port has the five that
/// are plain values here and will take items and fields when something stores them.
#[derive(Debug, Clone)]
pub struct Submap {
    terrain: TilePage<u32>,
    furniture: TilePage<u32>,
    luminance: TilePage<u8>,
    traps: TilePage<u32>,
    radiation: TilePage<i32>,
}

impl Submap {
    /// Builds a submap filled with one terrain.
    #[must_use]
    pub fn new(uniform_terrain: u32) -> Self {
        Self {
            terrain: TilePage::new(uniform_terrain),
            furniture: TilePage::new(NULL_ID),
            luminance: TilePage::new(0),
            traps: TilePage::new(NULL_ID),
            radiation: TilePage::new(0),
        }
    }

    /// The terrain every cell holds while the submap is uniform.
    #[must_use]
    pub fn uniform_terrain(&self) -> u32 {
        self.terrain.get(0, 0)
    }

    /// The terrain at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    #[must_use]
    pub fn terrain(&self, x: usize, y: usize) -> u32 {
        self.terrain.get(x, y)
    }

    /// Sets the terrain at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    pub fn set_terrain(&mut self, x: usize, y: usize, terrain: u32) {
        self.terrain.set(x, y, terrain);
    }

    /// Whether nothing has been written to this submap.
    ///
    /// Reference: `is_uniform()` is `!m` (`submap.h:293-295`) - one array for all seven layers, so the first write to *any* of them materialises all of them. The
    /// port keeps a page per layer, so this has to ask every layer and not only the terrain.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        self.terrain.is_uniform()
            && self.furniture.is_uniform()
            && self.luminance.is_uniform()
            && self.traps.is_uniform()
            && self.radiation.is_uniform()
    }

    /// The furniture at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    #[must_use]
    pub fn furniture(&self, x: usize, y: usize) -> u32 {
        self.furniture.get(x, y)
    }

    /// Sets the furniture at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    pub fn set_furniture(&mut self, x: usize, y: usize, furniture: u32) {
        self.furniture.set(x, y, furniture);
    }

    /// Fills the submap with one piece of furniture.
    ///
    /// Reference: `set_all_furn` (`submap.h:119-123`). There is no `uniform_ok` flag here, unlike `set_all_ter`, so this always materialises the page.
    pub fn set_all_furniture(&mut self, furniture: u32) {
        self.furniture = filled(furniture);
    }

    /// The trap at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    #[must_use]
    pub fn trap(&self, x: usize, y: usize) -> u32 {
        self.traps.get(x, y)
    }

    /// Sets the trap at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    pub fn set_trap(&mut self, x: usize, y: usize, trap: u32) {
        self.traps.set(x, y, trap);
    }

    /// Fills the submap with one trap. As with furniture and unlike terrain, there is no way to stay uniform.
    pub fn set_all_traps(&mut self, trap: u32) {
        self.traps = filled(trap);
    }

    /// The light at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    #[must_use]
    pub fn luminance(&self, x: usize, y: usize) -> u8 {
        self.luminance.get(x, y)
    }

    /// Sets the light at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    pub fn set_luminance(&mut self, x: usize, y: usize, luminance: u8) {
        self.luminance.set(x, y, luminance);
    }

    /// Counts one item that might give off light, saturating at 255.
    ///
    /// Reference: `update_lum_add` (`submap.h:185-191`). A non-emissive item still materialises the array, because `ensure_nonuniform()` runs first.
    pub fn update_lum_add(&mut self, x: usize, y: usize, emissive: bool) {
        let current = self.luminance.get(x, y);
        self.luminance.set(x, y, current);
        if emissive && current < 255 {
            self.luminance.set(x, y, current + 1);
        }
    }

    /// Takes one item away and adjusts the light count.
    ///
    /// Reference: `submap::update_lum_rem` (`submap.cpp:364-388`). Three branches: a non-emissive item changes nothing; a count that is neither zero nor saturated
    /// just decrements; a saturated one is **recounted** from the emissive items left on the square (`emissive_left` here) and written as one fewer than that, which
    /// is the reference's own arithmetic - including the wrapping it would do at zero.
    pub fn update_lum_rem(&mut self, x: usize, y: usize, emissive: bool, emissive_left: usize) {
        let current = self.luminance.get(x, y);
        self.luminance.set(x, y, current);
        if !emissive {
            return;
        }
        if current != 0 && current < 255 {
            self.luminance.set(x, y, current - 1);
            return;
        }
        if emissive_left <= 256 {
            self.luminance.set(x, y, recount(emissive_left));
        }
    }

    /// The radiation at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    #[must_use]
    pub fn radiation(&self, x: usize, y: usize) -> i32 {
        self.radiation.get(x, y)
    }

    /// Sets the radiation at a cell.
    ///
    /// # Panics
    ///
    /// Panics outside the submap, as the page does.
    pub fn set_radiation(&mut self, x: usize, y: usize, radiation: i32) {
        self.radiation.set(x, y, radiation);
    }
}

/// Every submap, owned in one place.
#[derive(Debug, Clone, Default)]
pub struct SubmapStore {
    submaps: Vec<Submap>,
}

impl SubmapStore {
    /// Builds an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a submap filled with one terrain and returns its id.
    pub fn add(&mut self, uniform_terrain: u32) -> SubmapId {
        self.submaps.push(Submap::new(uniform_terrain));
        SubmapId::from_index(self.submaps.len() - 1)
    }

    /// The submap an id names, or nothing if it was never made.
    ///
    /// Nothing is returned rather than a default: an ungenerated submap has no
    /// terrain, and the reference's null pointer says the same thing.
    #[must_use]
    pub fn get(&self, id: SubmapId) -> Option<&Submap> {
        self.submaps.get(id.index())
    }

    /// The submap an id names, for writing.
    ///
    /// # Panics
    ///
    /// Panics if the id names nothing, because the alternative - silently doing
    /// nothing - turns a coordinate bug into a terrain that never changes.
    pub fn get_mut(&mut self, id: SubmapId) -> Option<&mut Submap> {
        self.submaps.get_mut(id.index())
    }

    /// How many submaps the store holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.submaps.len()
    }

    /// Whether the store holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.submaps.is_empty()
    }
}

/// A page that has been materialised: every cell written, so it is no longer uniform.
fn filled(value: u32) -> TilePage<u32> {
    let mut page = TilePage::new(value);
    // Writing one cell would only materialise it; writing the same value to a cell that already holds it is exactly what the reference's setters do, so this is the
    // honest way to say "materialise without changing anything".
    for x in 0..SUBMAP_SIDE {
        for y in 0..SUBMAP_SIDE {
            page.set(x, y, value);
        }
    }
    page
}

/// The reference's `static_cast<uint8_t>( count - 1 )` (`submap.cpp:384`), kept rather than corrected.
///
/// At `count == 0` the subtraction underflows and the cast takes the low byte, so the answer is 255. That looks like a bug in the reference and may be one; it is
/// unreachable through the game's own paths because the item being removed is still in the square's list when it is counted. The port reproduces it exactly and
/// says so here, because silently "fixing" it would be a divergence nobody could see.
#[allow(clippy::cast_possible_truncation)]
fn recount(emissive_left: usize) -> u8 {
    emissive_left.wrapping_sub(1) as u8
}
