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

/// The per-cell work a cast does: grid coordinate, distance, and the transparency
/// accumulated along the ray that reached it.
type Visit<'a> = &'a mut dyn FnMut(i32, i32, i32, f32);

#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
fn cast_segment(
    visit: Visit<'_>,
    grid: &TransparencyGrid,
    transform: [i32; 4],
    offset: (i32, i32),
    offset_distance: i32,
    row: i32,
    start: f32,
    end: f32,
    radius: i32,
    cumulative: f32,
) {
    let [xx, xy, yx, yy] = transform;
    let mut new_start = 0.0_f32;
    let limit = (radius - offset_distance) as f32;
    if start < end {
        return;
    }
    let mut blocked = false;
    let mut start = start;
    // The ray's running average, carried across the cells of a row as well as into the
    // recursion: the reference updates it in place as it walks, which is what makes a band of
    // smoke dim the cells beyond it and not merely the cells it recursed from.
    let mut cumulative = cumulative;
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
            // The reference accumulates the ray's transparency as it travels and gives each
            // cell the running average, so what arrives there is `sight_calc` of that
            // average rather than of open air.
            let here = accumulate_transparency(cumulative, grid.at(cx, cy), distance);
            cumulative = here;
            if inside && dx.abs().max(dy.abs()) <= radius {
                visit(cx, cy, distance, here);
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
                    visit,
                    grid,
                    transform,
                    offset,
                    offset_distance,
                    distance + 1,
                    start,
                    left_slope,
                    radius,
                    here,
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
    let side = grid.side;
    let mut visible = vec![false; side * side];
    {
        #[allow(clippy::indexing_slicing)]
        let mut visit = |cx: i32, cy: i32, _distance: i32, _transparency: f32| {
            if let (Ok(x), Ok(y)) = (usize::try_from(cx), usize::try_from(cy)) {
                visible[x * side + y] = true;
            }
        };
        for transform in TRANSFORMS {
            cast_segment(
                &mut visit, grid, transform, origin, 0, 1, 1.0, 0.0, radius, 1.0,
            );
        }
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

/// How much light survives a stretch of air.
///
/// Reference: `sight_calc` (`src/shadowcasting.h:96-99`), which is the Beer-Lambert law
/// with the absorption coefficients merged into a running average: what arrives is the
/// light divided by `e^(transparency * distance)`.
#[must_use]
pub fn sight_calc(numerator: f32, transparency: f32, distance: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let distance = distance as f32;
    numerator / (transparency * distance).exp()
}

/// Whether light goes through a cell at all.
///
/// Reference: `sight_check` (`src/shadowcasting.h:100-103`). Solid is the only thing that
/// stops light outright; everything else merely dims it, however much.
#[must_use]
pub fn sight_check(transparency: f32) -> bool {
    transparency > SOLID
}

/// Folds one cell's transparency into the running average a ray carries.
///
/// Reference: `accumulate_transparency` (`src/shadowcasting.h:112-116`). The average is
/// weighted by distance travelled, so a ray crossing one dense cell is dimmed less than a
/// ray crossing ten of them.
#[must_use]
pub fn accumulate_transparency(cumulative: f32, current: f32, distance: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let distance = distance as f32;
    ((distance - 1.0) * cumulative + current) / distance
}

/// Raises one quarter of a tile's light to at least `value`.
///
/// Reference: `update_light_quadrants` (`src/shadowcasting.h:108-111`). Two rays reaching
/// the same corner do not add: the brighter one wins, because the light from a source is
/// the same light however many ways it is computed.
pub fn update_light_quadrants(update: &mut FourQuadrants, value: f32, quadrant: Quadrant) {
    if value > update.get(quadrant) {
        update.set(quadrant, value);
    }
}

/// Which quarters of a tile face the tile to its north, west, east or south.
///
/// Reference: `dir_quadrants` (`src/lightmap.cpp:384-394`). The five rows are the tile
/// itself, then north, west, east and south, in the order the reference walks them. A
/// quarter is named by the corner it sits in, so facing north means lighting the two
/// quarters on the north side.
pub const FACING: [[Quadrant; 2]; 5] = [
    [Quadrant::NorthEast, Quadrant::NorthWest],
    [Quadrant::NorthEast, Quadrant::NorthWest],
    [Quadrant::SouthWest, Quadrant::NorthWest],
    [Quadrant::SouthEast, Quadrant::NorthEast],
    [Quadrant::SouthEast, Quadrant::SouthWest],
];

/// Lights the quarters of a tile that face the outdoors.
///
/// Reference: the ambient pass in `map::build_map_cache` (`src/lightmap.cpp:401-438`),
/// which walks each tile and its four cardinal neighbours and, where it finds open sky,
/// raises the quarters facing it to the daylight level. That is how a doorway lightens the
/// floor inside it and how a window does not light the whole room.
///
/// `neighbours` is north, west, east, south - the reference's order, which decides nothing
/// here but is kept so a future reader comparing the two does not have to guess.
#[must_use]
pub fn light_in_from_outside(
    outside: [bool; 5],
    level: f32,
    mut tile: FourQuadrants,
) -> FourQuadrants {
    for (index, open) in outside.iter().enumerate() {
        if !open {
            continue;
        }
        // `outside` and `FACING` are both five long; the enumerate cannot outrun the table.
        let Some(facing) = FACING.get(index) else {
            continue;
        };
        for quadrant in *facing {
            if level > tile.get(quadrant) {
                tile.set(quadrant, level);
            }
        }
    }
    tile
}

/// How brightly a source of `luminance` lights each cell it reaches.
///
/// Reference: `cast_zlight` with `sight_calc` as its accumulator (`src/shadowcasting.h:96-99`,
/// `src/shadowcasting.cpp:530-580`). The transparency a ray crosses is averaged as it travels,
/// so smoke dims what is behind it instead of being treated as if it were not there - which is
/// the difference between a lamp in a fog and a lamp in a void.
#[must_use]
pub fn cast_light_values(
    grid: &TransparencyGrid,
    origin: (i32, i32),
    radius: i32,
    luminance: f32,
) -> Vec<f32> {
    let side = grid.side;
    let mut values = vec![0.0_f32; side * side];
    {
        #[allow(clippy::indexing_slicing, clippy::cast_precision_loss)]
        let mut visit = |cx: i32, cy: i32, distance: i32, transparency: f32| {
            if distance == 0 {
                return;
            }
            let value = sight_calc(luminance, transparency, distance);
            if let (Ok(x), Ok(y)) = (usize::try_from(cx), usize::try_from(cy)) {
                let index = x * side + y;
                if value > values[index] {
                    values[index] = value;
                }
            }
        };
        for transform in TRANSFORMS {
            cast_segment(
                &mut visit, grid, transform, origin, 0, 1, 1.0, 0.0, radius, 1.0,
            );
        }
    }
    values
}
