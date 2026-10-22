//! A page of tile values that costs nothing while it is all one value.
//!
//! Reference: `submap` (`src/submap.h:74-130`). The submap keeps its dense array
//! in a pointer that is null while the whole page is one value, and
//! `ensure_nonuniform` allocates and fills it on the first write that needs it.
//! Generated maps are overwhelmingly runs of the same tile, so this is the
//! difference between paying for the terrain of a city and paying for the terrain
//! of the cells that were actually decided.

/// One map square's worth of a single value type, held sparsely until it diverges.
///
/// The dimensions are a submap's: twelve map squares across, twelve down
/// (`Scale for Submap`, `MAP_SQUARES_PER = 12`).
#[derive(Debug, Clone)]
pub struct TilePage<T: Copy> {
    uniform: T,
    /// Empty while the page is uniform; `SIDE * SIDE` once it is not.
    dense: Vec<T>,
}

/// A submap is twelve map squares across.
pub const SIDE: usize = 12;

impl<T: Copy + PartialEq> TilePage<T> {
    /// Builds a page where every cell holds `uniform`, allocating nothing.
    #[must_use]
    pub fn new(uniform: T) -> Self {
        Self {
            uniform,
            dense: Vec::new(),
        }
    }

    /// Whether every cell still holds the one value.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        self.dense.is_empty()
    }

    /// How many values are stored densely; zero while uniform.
    #[must_use]
    pub fn dense_len(&self) -> usize {
        self.dense.len()
    }

    /// The value at a coordinate.
    ///
    /// # Panics
    ///
    /// Panics if the coordinate is outside the page. The reference indexes its
    /// arrays the same way, and a coordinate outside a submap is a bug in the
    /// caller rather than something to clamp.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn get(&self, x: usize, y: usize) -> T {
        assert!(x < SIDE && y < SIDE, "cell {x},{y} is outside the page");
        if self.dense.is_empty() {
            self.uniform
        } else {
            self.dense[index_of(x, y)]
        }
    }

    /// Sets one cell, materialising the page if this is the first difference.
    ///
    /// Writing the uniform value back does **not** collapse the page again: the
    /// reference's `ensure_nonuniform` is one-way, and a structure that collapsed
    /// would have different worst-case behaviour from the one being ported.
    ///
    /// # Panics
    ///
    /// Panics if the coordinate is outside the page, as [`get`](Self::get) does.
    #[allow(clippy::indexing_slicing)]
    pub fn set(&mut self, x: usize, y: usize, value: T) {
        assert!(x < SIDE && y < SIDE, "cell {x},{y} is outside the page");
        // Reference: every setter calls `ensure_nonuniform()` first (`submap.h:152-158`). Writing the value a uniform page already holds therefore allocates the
        // whole page - the reference's rule is "has anything been written", not "do the squares differ". Skipping the allocation here would be a nicer data
        // structure and a different one, and the difference is invisible until something counts allocated pages or saves a submap.
        if self.dense.is_empty() {
            self.dense = vec![self.uniform; SIDE * SIDE];
        }
        let index = index_of(x, y);
        self.dense[index] = value;
    }

    /// Every value in the reference's dense order.
    ///
    /// `maptile_soa` holds `mdarray<T, point_sm_ms>`, and `mdarray_impl_2d` is a `std::array` of columns indexed by **x** (`mdarray.h:45,64`), so the flat order is
    /// `x * SIDE + y`: every `y` for `x = 0`, then every `y` for `x = 1`. Anything that saves, digests or replays a page has to see that sequence.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        PageIter {
            page: self,
            index: 0,
        }
    }
}

/// Where a cell lives in the dense array: `x` outer, `y` inner, as `mdarray` stores it.
fn index_of(x: usize, y: usize) -> usize {
    x * SIDE + y
}

/// Walks a page, answering from the uniform value until the page is dense.
struct PageIter<'a, T: Copy + PartialEq> {
    page: &'a TilePage<T>,
    index: usize,
}

impl<T: Copy + PartialEq> Iterator for PageIter<'_, T> {
    type Item = T;

    #[allow(clippy::indexing_slicing)]
    fn next(&mut self) -> Option<T> {
        if self.index >= SIDE * SIDE {
            return None;
        }
        let index = self.index;
        self.index += 1;
        Some(if self.page.dense.is_empty() {
            self.page.uniform
        } else {
            self.page.dense[index]
        })
    }
}
