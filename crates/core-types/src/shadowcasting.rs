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
