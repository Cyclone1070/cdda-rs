//! What a light can see, cast in the reference's eight segments.
//!
//! Reference: `tests/shadowcasting_test.cpp:33-90` (`oldCastLight`), the copy the
//! reference keeps of its own caster so it can check the newer one against it, driven
//! by the eight segment transforms from `src/shadowcasting.cpp:545-580`.

/// The transparency value of a cell that blocks everything.
pub const SOLID: f32 = 0.0;

/// A square grid of transparency values, stored `[x][y]` as the reference's caches
/// are (`output_cache[current.x][current.y]`).
#[derive(Debug, Clone)]
pub struct TransparencyGrid {
    side: usize,
    cells: Vec<f32>,
}

impl TransparencyGrid {
    /// Builds a grid from `[x][y]` values.
    ///
    /// # Panics
    ///
    /// Panics if the values are not a square.
    #[must_use]
    pub fn new(side: usize, cells: Vec<f32>) -> Self {
        assert_eq!(cells.len(), side * side, "a grid is square");
        Self { side, cells }
    }

    /// The transparency at a cell, or solid outside the grid.
    // The bounds are checked immediately above, so the index is inside the array;
    // clippy cannot see through the two-step conversion.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn at(&self, x: i32, y: i32) -> f32 {
        // Outside the grid is solid, which is what the reference's own loops assume:
        // they never leave the buffer, so nothing there reads as transparent.
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return SOLID;
        };
        if x >= self.side || y >= self.side {
            return SOLID;
        }
        self.cells[x * self.side + y]
    }

    /// How wide the grid is.
    #[must_use]
    pub fn side(&self) -> usize {
        self.side
    }
}

/// The reference's eight segments, in its order (`src/shadowcasting.cpp:545-580`).
///
/// Four cover one level and four the mirror of it. Using four leaves half the field
/// dark - the mistake the fixtures caught.
const TRANSFORMS: [[i32; 4]; 8] = [
    [0, 1, 1, 0],
    [1, 0, 0, 1],
    [0, -1, 1, 0],
    [-1, 0, 0, 1],
    [0, 1, -1, 0],
    [1, 0, 0, -1],
    [0, -1, -1, 0],
    [-1, 0, 0, -1],
];

#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
#[allow(clippy::indexing_slicing)]
fn cast_segment(
    visible: &mut [bool],
    grid: &TransparencyGrid,
    transform: [i32; 4],
    offset: (i32, i32),
    offset_distance: i32,
    row: i32,
    start: f32,
    end: f32,
    radius: i32,
) {
    let [xx, xy, yx, yy] = transform;
    let mut new_start = 0.0_f32;
    let limit = (radius - offset_distance) as f32;
    if start < end {
        return;
    }
    let mut blocked = false;
    let mut start = start;
    let mut distance = row;
    while (distance as f32) <= limit && !blocked {
        let dy = -distance;
        let mut dx = -distance;
        while dx <= 0 {
            let left_slope = (dx as f32 - 0.5) / (dy as f32 + 0.5);
            let right_slope = (dx as f32 + 0.5) / (dy as f32 - 0.5);
            if start < right_slope {
                dx += 1;
                continue;
            } else if end > left_slope {
                break;
            }
            let cx = offset.0 + dx * xx + dy * xy;
            let cy = offset.1 + dx * yx + dy * yy;
            // `try_from` rather than a cast: a negative coordinate is outside the
            // grid, not a large index.
            let inside = usize::try_from(cx).is_ok_and(|x| x < grid.side)
                && usize::try_from(cy).is_ok_and(|y| y < grid.side);
            if inside
                && dx.abs().max(dy.abs()) <= radius
                && let (Ok(x), Ok(y)) = (usize::try_from(cx), usize::try_from(cy))
            {
                visible[x * grid.side + y] = true;
            }
            if blocked {
                if grid.at(cx, cy) == SOLID {
                    new_start = right_slope;
                } else {
                    blocked = false;
                    start = new_start;
                }
            } else if grid.at(cx, cy) == SOLID && distance < radius {
                blocked = true;
                cast_segment(
                    visible,
                    grid,
                    transform,
                    offset,
                    offset_distance,
                    distance + 1,
                    start,
                    left_slope,
                    radius,
                );
                new_start = right_slope;
            }
            dx += 1;
        }
        distance += 1;
    }
}

/// Which cells an origin can see.
///
/// The origin does not light itself, which is the reference's behaviour rather than an
/// omission: `oldCastLight` marks cells the walk reaches, and the walk starts one step
/// out.
#[must_use]
pub fn cast_light(grid: &TransparencyGrid, origin: (i32, i32), radius: i32) -> Vec<bool> {
    let mut visible = vec![false; grid.side * grid.side];
    for transform in TRANSFORMS {
        cast_segment(
            &mut visible,
            grid,
            transform,
            origin,
            0,
            1,
            1.0,
            0.0,
            radius,
        );
    }
    visible
}

/// Which quarter of a tile a light value belongs to.
///
/// Reference: `enum class quadrant` (`src/shadowcasting.h:24-30`), in its order: north-east,
/// south-east, south-west, north-west. The order is load-bearing - values are indexed by it
/// everywhere - so it is written down here rather than left to whatever an array happens to
/// be filled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Quadrant {
    /// North-east of the tile's centre.
    NorthEast,
    /// South-east.
    SouthEast,
    /// South-west.
    SouthWest,
    /// North-west.
    NorthWest,
}

impl Quadrant {
    /// The index this quadrant is stored at.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::NorthEast => 0,
            Self::SouthEast => 1,
            Self::SouthWest => 2,
            Self::NorthWest => 3,
        }
    }
}

/// Four light values per tile, one per quarter.
///
/// Reference: `four_quadrants` (`src/shadowcasting.h:38-90`). A tile's light is not one
/// number: a torch in a corner lights one quarter of the floor much more than the opposite
/// one, and the reference keeps all four so that drawing and seeing agree.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FourQuadrants {
    values: [f32; 4],
}

impl FourQuadrants {
    /// Four copies of one value.
    #[must_use]
    pub const fn uniform(value: f32) -> Self {
        Self { values: [value; 4] }
    }

    /// Fills all four with one value.
    pub fn fill(&mut self, value: f32) {
        self.values = [value; 4];
    }

    // The index comes from the quadrant enum, whose four values are 0..=3 and which is
    // the array's own length; clippy cannot see that through the method call.
    /// The value for one quarter.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn get(&self, quadrant: Quadrant) -> f32 {
        self.values[quadrant.index()]
    }

    /// Sets one quarter.
    #[allow(clippy::indexing_slicing)]
    pub fn set(&mut self, quadrant: Quadrant, value: f32) {
        self.values[quadrant.index()] = value;
    }

    /// The brightest quarter.
    ///
    /// This is what `lit_level` reads: a tile is as bright as its brightest corner, which
    /// is why a lamp around a corner still lights the floor you can see.
    #[must_use]
    pub fn max(&self) -> f32 {
        self.values
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
    }

    /// Every quarter multiplied by the same quarter of another tile.
    #[must_use]
    pub fn multiplied(self, other: Self) -> Self {
        Self {
            values: [
                self.values[0] * other.values[0],
                self.values[1] * other.values[1],
                self.values[2] * other.values[2],
                self.values[3] * other.values[3],
            ],
        }
    }

    /// The brighter of two tiles, quarter by quarter.
    ///
    /// Two lights in the same room add where they overlap and do not where they do not;
    /// this is the per-quarter half of that, and it is why a corridor can be bright on one
    /// side of the corner.
    #[must_use]
    pub fn max_with(self, other: Self) -> Self {
        Self {
            values: [
                self.values[0].max(other.values[0]),
                self.values[1].max(other.values[1]),
                self.values[2].max(other.values[2]),
                self.values[3].max(other.values[3]),
            ],
        }
    }

    /// The brighter of each quarter and a value.
    #[must_use]
    pub fn max_with_value(self, value: f32) -> Self {
        Self {
            values: self.values.map(|own| own.max(value)),
        }
    }
}
