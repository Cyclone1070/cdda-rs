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
        if self.dense.is_empty() {
            if value == self.uniform {
                return;
            }
            self.dense = vec![self.uniform; SIDE * SIDE];
        }
        let index = index_of(x, y);
        self.dense[index] = value;
    }

    /// Every value, row by row.
    ///
    /// This is the order the reference's dense array is stored in - `ter[y][x]` -
    /// so code that walks a page sees the same sequence it would there.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        PageIter {
            page: self,
            index: 0,
        }
    }
}

/// Where a cell lives in the dense array, row by row (`ter[y][x]`).
fn index_of(x: usize, y: usize) -> usize {
    y * SIDE + x
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
