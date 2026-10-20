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

/// One submap's terrain, as a page that stays cheap while it is uniform.
#[derive(Debug, Clone)]
pub struct Submap {
    terrain: TilePage<u32>,
}

impl Submap {
    /// Builds a submap filled with one terrain.
    #[must_use]
    pub fn new(uniform_terrain: u32) -> Self {
        Self {
            terrain: TilePage::new(uniform_terrain),
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

    /// Whether every cell still holds one terrain.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        self.terrain.is_uniform()
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
