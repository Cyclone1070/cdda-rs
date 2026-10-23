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

/// Keeps the brighter of two light values.
///
/// Reference: `update_light` (`src/shadowcasting.h:105-108`), which is a maximum written as an `if` for a compiler reason the reference explains; a port can call the
/// maximum directly.
#[must_use]
pub fn update_light(existing: f32, candidate: f32) -> f32 {
    existing.max(candidate)
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

/// An angle, kept as a rise over a run so comparisons stay exact.
///
/// Reference: `struct slope` (`src/shadowcasting.cpp:19-44`). The reference stores both fields as `int_least8_t` and notes that "a shadowcasting radius of 60 readily
/// fits within that space" while "a radius of 120 definitely will not". The port stores `i32`, which is exact for every radius the game uses and also for the ones the
/// reference would truncate; the truncation is a limit of the reference's storage, not a rule worth reproducing.
#[derive(Debug, Clone, Copy)]
pub struct Slope {
    rise: i32,
    run: i32,
}

impl Slope {
    /// Builds an angle, keeping the run positive so that comparisons need no sign handling.
    ///
    /// Reference: the constructor (`shadowcasting.cpp:20-29`) takes `abs( run )` and negates the rise when the run came in negative - which is the same angle, written
    /// the way the comparison wants it.
    #[must_use]
    pub fn new(rise: i32, run: i32) -> Self {
        if run < 0 {
            Self {
                rise: -rise,
                run: -run,
            }
        } else {
            Self { rise, run }
        }
    }

    /// The rise.
    #[must_use]
    pub fn rise(self) -> i32 {
        self.rise
    }

    /// The run, always positive.
    #[must_use]
    pub fn run(self) -> i32 {
        self.run
    }
}

impl PartialEq for Slope {
    /// Equal angles, not equal pairs: one in two is two in four (`operator==`, `shadowcasting.cpp:39-43`).
    fn eq(&self, other: &Self) -> bool {
        self.rise * other.run == other.rise * self.run
    }
}

impl Eq for Slope {}

impl PartialOrd for Slope {
    /// Compares by cross-multiplication, as `operator<` does (`shadowcasting.cpp:34-38`): `a/b < c/d` is `a*d < c*b`, which is valid because both runs are positive.
    ///
    /// Comparing the rises first would make one in three look smaller than two in five, which it is not - and the sweep's spans would then be cut at the wrong angles.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Slope {
    /// A total order, which the rationals with positive denominators are: the sweep asks for the higher and lower of two angles, so this has to be `Ord` and not
    /// merely `PartialOrd`.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.rise * other.run).cmp(&(other.rise * self.run))
    }
}

/// One frustum of the sweep: the squares between two angles, at one accumulated opacity.
///
/// Reference: `struct span` (`src/shadowcasting.cpp:49-63`). `major` is the axis being swept at this distance and `minor` the axis across it; the two `skip` flags stop a
/// new span re-checking the row or column that caused it to be cut off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    /// The leading edge along the sweep direction.
    pub start_major: Slope,
    /// The trailing edge along the sweep direction.
    pub end_major: Slope,
    /// The leading edge across the sweep.
    pub start_minor: Slope,
    /// The trailing edge across the sweep.
    pub end_minor: Slope,
    /// The running mean transparency of everything this span has covered.
    pub cumulative_value: f32,
    /// Set on a span cut off by a change on the current row, so it does not re-check that row.
    pub skip_first_row: bool,
    /// Set on a span cut off by a change in the current column, so it does not re-check that column.
    pub skip_first_column: bool,
}

impl Span {
    /// Builds a span with neither skip flag set.
    #[must_use]
    pub fn new(
        start_major: Slope,
        end_major: Slope,
        start_minor: Slope,
        end_minor: Slope,
        cumulative_value: f32,
    ) -> Self {
        Self {
            start_major,
            end_major,
            start_minor,
            end_minor,
            cumulative_value,
            skip_first_row: false,
            skip_first_column: false,
        }
    }
}

/// Cuts the span at `index` where the sweep met a square of a different opacity.
///
/// Reference: `split_span` (`src/shadowcasting.cpp:69-159`). The span is cut into up to four pieces, and which of them exist depends on where the edges fall:
///
/// * `A` - rows already processed, old opacity - and `B` - the processed part of this row - are kept **only when the square just left was transparent**, because an
///   opaque square's span stops there;
/// * `D` - everything past the current row - is cut off whenever the leading edge has not reached the span's end, and starts on the **next** row;
/// * the span being processed is then truncated to the current row and takes the new opacity.
///
/// Returns the index of the span that was being processed. It can move when pieces are inserted before it, and the caller must use the answer rather than the index it
/// passed in.
// The reference's signature, transplanted: eleven arguments, three of them out-parameters, because the sweep hands the current square's edges in rather than a struct
// that would have to be built and thrown away at every square. And every index below is either the caller's own span index or one this function just created.
#[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
pub fn split_span(
    spans: &mut Vec<Span>,
    index: usize,
    current_transparency: &mut f32,
    new_transparency: f32,
    distance: i32,
    new_start_minor: &mut Slope,
    trailing_edge_major: Slope,
    leading_edge_major: Slope,
    trailing_edge_minor: Slope,
    leading_edge_minor: Slope,
) -> usize {
    let span = spans[index];
    let next_cumulative =
        accumulate_transparency(span.cumulative_value, *current_transparency, distance);
    // The square just left was transparent, so the rows and columns already covered keep their own span. If it was opaque, they are gone.
    let old_was_transparent = sight_check(*current_transparency);
    let mut inserted_before = 0;
    if old_was_transparent {
        // `A`: the rows above this one.
        if trailing_edge_major > span.start_major {
            spans.insert(
                index,
                Span {
                    start_major: span.start_major,
                    end_major: trailing_edge_major,
                    start_minor: span.start_minor,
                    end_minor: span.end_minor,
                    cumulative_value: next_cumulative,
                    skip_first_row: false,
                    skip_first_column: span.skip_first_column,
                },
            );
            inserted_before += 1;
        }
        // `B`: the part of this row already covered.
        if trailing_edge_minor > span.start_minor {
            spans.insert(
                index + inserted_before,
                Span {
                    start_major: span.start_major.max(trailing_edge_major),
                    end_major: span.end_major.min(leading_edge_major),
                    start_minor: span.start_minor,
                    end_minor: trailing_edge_minor,
                    cumulative_value: next_cumulative,
                    skip_first_row: false,
                    skip_first_column: span.skip_first_column,
                },
            );
            inserted_before += 1;
        }
        // The previous square was transparent, so the new span starts at its trailing edge rather than at the previous one's.
        *new_start_minor = trailing_edge_minor;
    }

    let span = spans[index + inserted_before];
    // `D`: the rows still to come, which keep the old opacity and must not re-check this row.
    if leading_edge_major < span.end_major {
        spans.insert(
            index + inserted_before + 1,
            Span {
                start_major: leading_edge_major,
                end_major: span.end_major,
                start_minor: span.start_minor,
                end_minor: span.end_minor,
                cumulative_value: span.cumulative_value,
                skip_first_row: true,
                skip_first_column: span.skip_first_column,
            },
        );
    }

    let span = &mut spans[index + inserted_before];
    // Two transparent squares of different transparency: the cut is between them, so the column that caused it must not be checked twice.
    span.skip_first_column = sight_check(*current_transparency) && sight_check(new_transparency);
    span.start_major = span.start_major.max(trailing_edge_major);
    span.end_major = span.end_major.min(leading_edge_major);
    span.start_minor = *new_start_minor;
    *new_start_minor = leading_edge_minor;
    *current_transparency = new_transparency;
    index + inserted_before
}

/// The transparency of open air, which is what the sweep starts with.
pub const AMBIENT_OPEN: f32 = crate::light::TRANSPARENCY_OPEN_AIR;

/// Which quadrant light arriving from one direction belongs in.
///
/// Reference: `quadrant_from_x_y` (`lightmap.cpp:874-879`). The mapping is not the obvious one - a vector with a positive x is `NW` for a positive y and `SW` for a
/// negative one, and a negative x gives `NE` and `SE` - because the axes here are the sweep's own, not the screen's.
#[must_use]
pub fn quadrant_from_x_y(x: i32, y: i32) -> Quadrant {
    if x > 0 {
        if y > 0 {
            Quadrant::NorthWest
        } else {
            Quadrant::SouthWest
        }
    } else if y > 0 {
        Quadrant::NorthEast
    } else {
        Quadrant::SouthEast
    }
}

/// Casts one light source's light onto a level's four-quadrant light map.
///
/// Reference: `castLightAll` (`lightmap.cpp:986-1012`) - the same `castLight` eight times, once per transform, so between them the eight cover every square on the level.
/// Each square takes the **brightest** light reaching it, not the sum: two rays arriving at one square are two chances to see it, not twice the brightness.
///
/// The source's own square is **not** lit here. `map::apply_light_source` lights it before casting, because the sweep starts one square out and a lamp that had to be lit
/// by its own light would be dark.
#[must_use]
pub fn cast_light_quadrants(
    grid: &TransparencyGrid,
    origin: (i32, i32),
    radius: i32,
    numerator: f32,
    circular: bool,
) -> Vec<FourQuadrants> {
    cast_light_octants(
        grid,
        origin,
        radius,
        numerator,
        circular,
        [true; OCTANTS],
    )
}

/// How many octants a quadrant cast runs, one pair per direction.
pub const OCTANTS: usize = 8;

/// The direction each octant casts into, in [`TRANSFORMS`] order.
///
/// The reference's `apply_light_source` calls its caster with eight transform pairs, grouped by direction (`src/lightmap.cpp:1260-1290`); the port keeps the same eight in the
/// same order, so a caller that wants to skip a direction can name it here and let the table do the mapping.
pub const OCTANT_DIRECTIONS: [Direction; OCTANTS] = [
    Direction::West,
    Direction::South,
    Direction::East,
    Direction::South,
    Direction::West,
    Direction::North,
    Direction::East,
    Direction::North,
];

/// Which way a pair of octants casts, for [`OCTANT_DIRECTIONS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Towards smaller y.
    North,
    /// Towards larger x.
    East,
    /// Towards larger y.
    South,
    /// Towards smaller x.
    West,
}

/// The quadrant cast, running only the octants a caller allows.
///
/// Reference: `map::apply_light_source` (`src/lightmap.cpp:1217-1290`), which skips a direction when a neighbour holds a buffered source at least as bright - that neighbour's
/// own cast covers the squares beyond, so casting both would only spend time.
#[must_use]
pub fn cast_light_octants(
    grid: &TransparencyGrid,
    origin: (i32, i32),
    radius: i32,
    numerator: f32,
    circular: bool,
    allow: [bool; OCTANTS],
) -> Vec<FourQuadrants> {
    let mut lit = vec![FourQuadrants::uniform(0.0); grid.side * grid.side];
    for (index, transform) in TRANSFORMS.into_iter().enumerate() {
        if !allow.get(index).copied().unwrap_or(false) {
            continue;
        }
        let quad = quadrant_from_x_y(-transform[0] - transform[1], -transform[2] - transform[3]);
        let mut cast = Cast {
            lit: &mut lit,
            grid,
            transform,
            origin,
            quad,
            numerator,
            radius,
            circular,
        };
        cast.run(1, 1.0, 0.0, crate::light::TRANSPARENCY_OPEN_AIR);
    }
    lit
}

/// One octant's sweep, carrying the reference's own recursion state.
///
/// Reference: `castLight`'s body (`lightmap.cpp:893-980`). It is a recursive call with nine arguments in the reference; here the invariant parts live in the struct so
/// the recursion passes only what changes - the row, the span and the running transparency.
struct Cast<'a> {
    lit: &'a mut Vec<FourQuadrants>,
    grid: &'a TransparencyGrid,
    transform: [i32; 4],
    origin: (i32, i32),
    quad: Quadrant,
    numerator: f32,
    radius: i32,
    /// Whether the "circular distances" option is on, which makes a diagonal cost more than a step.
    circular: bool,
}

// A radius is a map-square count, far inside what an `f32` holds exactly; the reference converts the same values to `float` for the span arithmetic.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
impl Cast<'_> {
    fn run(&mut self, row: i32, start: f32, end: f32, cumulative: f32) {
        if start < end {
            return;
        }
        let [xx, xy, yx, yy] = self.transform;
        let radius = self.radius as f32;
        let mut start = start;
        let mut new_start;
        let mut cumulative = cumulative;
        let mut distance = row;
        while (distance as f32) <= radius {
            let dy = -distance;
            let mut started_row = false;
            let mut current_transparency = 0.0;
            // The reference starts x where the leading edge of the span is, so that the `start < leadingEdge`
            // test it never performs would never have failed.
            let away = start - (-(distance as f32) + 0.5) / (-(distance as f32) - 0.5);
            let adjustment = (away * (-(distance as f32) - 0.5)).ceil();
            let skip = if adjustment > 0.0 {
                adjustment as i32
            } else {
                0
            };
            let mut dx = -distance + skip;
            new_start = 0.0;
            while dx <= 0 {
                let cx = self.origin.0 + dx * xx + dy * xy;
                let cy = self.origin.1 + dx * yx + dy * yy;
                #[allow(clippy::cast_precision_loss)]
                let trailing = (dx as f32 - 0.5) / (dy as f32 + 0.5);
                #[allow(clippy::cast_precision_loss)]
                let leading = (dx as f32 + 0.5) / (dy as f32 - 0.5);
                let side = i32::try_from(self.grid.side).unwrap_or(i32::MAX);
                if cx < 0 || cy < 0 || cx >= side || cy >= side {
                    dx += 1;
                    continue;
                }
                if end > trailing {
                    break;
                }
                if !started_row {
                    started_row = true;
                    current_transparency = self.grid.at(cx, cy);
                }
                // With circles off this is the row number itself; with circles on it is the true distance, which rounds the light's reach.
                let dist = crate::light::rl_dist((0, 0), (dx, dy), self.circular);
                let last_intensity = sight_calc(self.numerator, cumulative, dist);
                let new_transparency = self.grid.at(cx, cy);
                let quarter = if sight_check(new_transparency) {
                    Quadrant::NorthEast
                } else {
                    self.quad
                };
                self.update(cx, cy, last_intensity, quarter);

                // Both are values read out of the same grid, so this is an exact comparison of identical bits rather than a tolerance question.
                #[allow(clippy::float_cmp)]
                if new_transparency == current_transparency {
                    new_start = leading;
                    dx += 1;
                    continue;
                }
                if sight_check(current_transparency) {
                    self.run(
                        distance + 1,
                        start,
                        trailing,
                        accumulate_transparency(cumulative, current_transparency, distance),
                    );
                }
                // The reference writes this as `if( !check(...) ) start = newStart; else start = trailingEdge;` - opaque squares restart the span from the new start,
                // transparent ones only from the trailing edge.
                start = if sight_check(current_transparency) {
                    trailing
                } else {
                    new_start
                };
                if start < end {
                    return;
                }
                current_transparency = new_transparency;
                new_start = leading;
                dx += 1;
            }
            if !sight_check(current_transparency) {
                break;
            }
            cumulative = accumulate_transparency(cumulative, current_transparency, distance);
            distance += 1;
        }
    }

    fn update(&mut self, x: i32, y: i32, value: f32, quarter: Quadrant) {
        if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
            #[allow(clippy::indexing_slicing)]
            let cell = &mut self.lit[x * self.grid.side + y];
            if cell.get(quarter) < value {
                cell.set(quarter, value);
            }
        }
    }
}

/// One level's light bookkeeping: the four-quadrant light map, the light source cache, and the buffer of sources still to be cast.
///
/// Reference: the three arrays `map::generate_lightmap` fills - `cache.lm`, `cache.sm` and `cache.light_source_buffer` (`lightmap.cpp:1085-1102`) - kept together
/// because the rule that lights a source reads all three and the caller must not be able to pass one without the others.
#[derive(Debug, Clone)]
pub struct LightMap {
    grid: TransparencyGrid,
    light: Vec<FourQuadrants>,
    source: Vec<f32>,
    buffer: Vec<f32>,
    circular: bool,
}

impl LightMap {
    /// A map with nothing lit, over a transparency grid.
    #[must_use]
    pub fn new(grid: TransparencyGrid, circular: bool) -> Self {
        let cells = grid.side * grid.side;
        Self {
            grid,
            light: vec![FourQuadrants::uniform(0.0); cells],
            source: vec![0.0; cells],
            buffer: vec![0.0; cells],
            circular,
        }
    }

    /// The side of the map, in squares.
    #[must_use]
    pub fn side(&self) -> usize {
        self.grid.side
    }

    /// The light at a square.
    #[must_use]
    pub fn light_at(&self, x: i32, y: i32) -> FourQuadrants {
        self.index(x, y)
            .and_then(|index| self.light.get(index).copied())
            .unwrap_or_else(|| FourQuadrants::uniform(0.0))
    }

    /// The light source sitting in a square, the reference's `sm`.
    #[must_use]
    pub fn source_at(&self, x: i32, y: i32) -> f32 {
        self.index(x, y)
            .and_then(|index| self.source.get(index).copied())
            .unwrap_or(0.0)
    }

    /// Records a source in the buffer without casting it.
    ///
    /// Reference: `map::add_light_source` (`lightmap.cpp:666-670`): the buffer keeps the **brightest** source seen at each square, because a square with two lamps is not
    /// twice as bright to the rule that reads the buffer.
    pub fn buffer_source(&mut self, at: (i32, i32), luminance: f32) {
        if let Some(index) = self.index(at.0, at.1)
            && let Some(slot) = self.buffer.get_mut(index)
        {
            *slot = slot.max(luminance);
        }
    }

    /// Lights one source: its own square, then a cast in every direction not already covered.
    ///
    /// Reference: `map::apply_light_source` (`lightmap.cpp:1217-1294`), in its order.
    #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
    pub fn add_source(&mut self, at: (i32, i32), luminance: f32) {
        let side = self.grid.side;
        let Some(index) = self.index(at.0, at.1) else {
            return;
        };
        if let (Some(light), Some(source)) = (
            self.light.get_mut(index).copied(),
            self.source.get_mut(index),
        ) {
            // The square holding a source is lit in **all four** quarters to at least one unit: a candle beside you is visible even though it lights nothing else.
            let floor = 1.0_f32.max(luminance);
            let lit = light.max_with_value(floor);
            if let Some(slot) = self.light.get_mut(index) {
                *slot = lit;
            }
            *source = source.max(luminance);
        }
        // One unit and below casts nothing at all; two units and below is raised to 1.49, so that it survives one step of open air.
        if luminance <= 1.0 {
            return;
        }
        let luminance = if luminance <= 2.0 { 1.49 } else { luminance };
        // The burning-hospital rule: a direction is skipped when a source already in the buffer, one square that way, is at least as bright.
        let neighbour = |x: i32, y: i32| -> f32 {
            self.index(x, y)
                .and_then(|index| self.buffer.get(index).copied())
                .unwrap_or(0.0)
        };
        let (px, py) = at;
        let on_edge =
            |x: i32, y: i32| x < 0 || y < 0 || (x as usize) >= side || (y as usize) >= side;
        let north = !on_edge(px, py - 1) && neighbour(px, py - 1) < luminance;
        let south = !on_edge(px, py + 1) && neighbour(px, py + 1) < luminance;
        let east = !on_edge(px + 1, py) && neighbour(px + 1, py) < luminance;
        let west = !on_edge(px - 1, py) && neighbour(px - 1, py) < luminance;
        let radius = i32::try_from(side).unwrap_or(1);
        let mut cast = |transform: [i32; 4]| {
            let mut octant = Cast {
                lit: &mut self.light,
                grid: &self.grid,
                transform,
                origin: (px, py),
                quad: quadrant_from_x_y(-transform[0] - transform[1], -transform[2] - transform[3]),
                numerator: luminance,
                radius,
                circular: self.circular,
            };
            octant.run(1, 1.0, 0.0, crate::light::TRANSPARENCY_OPEN_AIR);
        };
        // The pairing below is the reference's, literally, and it is worth reading twice. `apply_light_source`'s "north" test reads the buffer at `y - 1` and then
        // casts the two octants that cover `y + 1`; its "south" test reads `y + 1` and casts towards `y - 1`. The x directions are not inverted. Either the axis is
        // named for where the light comes from rather than where it goes, or the reference has a sign error that has never mattered much because the skip is a
        // performance shortcut and the skipped cast is usually covered by the neighbour that caused it. **The port reproduces it rather than correcting it**, so that a
        // later decision to change it is deliberate, and `docs/parity.md` records it as a suspected upstream inversion.
        if north {
            cast([1, 0, 0, -1]);
            cast([-1, 0, 0, -1]);
        }
        if east {
            cast([0, -1, 1, 0]);
            cast([0, -1, -1, 0]);
        }
        if south {
            cast([1, 0, 0, 1]);
            cast([-1, 0, 0, 1]);
        }
        if west {
            cast([0, 1, 1, 0]);
            cast([0, 1, -1, 0]);
        }
    }

    /// Pours a level's daylight into the map before any source is applied.
    ///
    /// Reference: `build_sunlight_cache` (`lightmap.cpp:311-437`), which *fills* the light map rather than adding to it, so it is the first thing step 2 of
    /// `generate_lightmap` does after zeroing. A source applied afterwards can only brighten a square.
    pub fn pour_sunlight(&mut self, light: &[FourQuadrants]) {
        for (slot, value) in self.light.iter_mut().zip(light.iter()) {
            *slot = *value;
        }
    }

    /// Applies every source recorded in the buffer.
    ///
    /// Reference: the flush at the end of `generate_lightmap` (`lightmap.cpp:650-659`), whose own comment counts the saving for a 12x12 inferno - 92,160 ray casts
    /// unbuffered against 7,680 buffered. The deeper reason is the gating: only after every emitter is recorded can a source see that its neighbour is just as bright, so
    /// **the buffer stays populated while it is being flushed** and is not cleared by this call. Squares are applied in the reference's own x-then-y order.
    pub fn flush(&mut self) {
        let side = self.grid.side;
        let pending: Vec<(i32, i32, f32)> = self
            .buffer
            .iter()
            .enumerate()
            .filter(|(_, luminance)| **luminance > 0.0)
            .filter_map(|(index, luminance)| {
                let x = i32::try_from(index / side).ok()?;
                let y = i32::try_from(index % side).ok()?;
                Some((x, y, *luminance))
            })
            .collect();
        for (x, y, luminance) in pending {
            self.add_source((x, y), luminance);
        }
    }

    /// Replaces a square's four quarters with one value.
    ///
    /// Reference: the `lm_override` pass (`lightmap.cpp:660-662`), which a field with `local_light_override` asks for: the square *is* that light, whatever the sky and
    /// the lamps had put there.
    pub fn override_square(&mut self, at: (i32, i32), value: f32) {
        if let Some(index) = self.index(at.0, at.1)
            && let Some(slot) = self.light.get_mut(index)
        {
            *slot = FourQuadrants::uniform(value);
        }
    }

    /// The dense index of a square, or `None` off the map.
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let side = i32::try_from(self.grid.side).ok()?;
        if x < 0 || y < 0 || x >= side || y >= side {
            return None;
        }
        #[allow(clippy::cast_sign_loss)]
        let index = x as usize * self.grid.side + y as usize;
        Some(index)
    }
}
