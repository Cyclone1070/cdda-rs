//! Coordinates: a point, an origin, and a scale, kept distinct in the type
//! system.
//!
//! Reference: `src/coordinates.h` and `src/coords_fwd.h`. The scale factors match
//! `map_squares_per()` (`coordinates.h:32`) and the constants in
//! `src/map_scale_constants.h`. Projection matches `project_to_impl`
//! (`coordinates.h:601-644`): multiplying when the result is finer, and
//! **flooring toward negative infinity** when it is coarser, via
//! `divide_xy_round_to_minus_infinity` (`src/point.h:146`). The vertical
//! coordinate is never scaled.
//!
//! # The origin and scale legality matrix
//!
//! A point is `(origin, scale)`. The reference lets some pairs talk to each other
//! and refuses the rest at compile time; the port does the same, so an illegal
//! combination is a build error rather than a wrong number at runtime.
//!
//! | Operation | Allowed when | Reference |
//! |---|---|---|
//! | `project_to::<Scale>` | the two scales are multiples of one another; the origin is kept | `coordinates.h:601-644` |
//! | `project_remain::<Scale>` | the result scale is **coarser** and has an origin of its own: submap, overmap terrain or overmap | `coordinates.h:796-810` |
//! | `combine(coarse, fine)` | the fine point's origin **is** the origin implied by the coarse scale | `coordinates.h:826-845` |
//! | `project_bounds::<Scale>` | as `project_to`; the cell's far corner comes from the coarse cell plus one | `coordinates.h:847-865` |
//! | difference, midpoint, distances | identical origin **and** scale | `coordinates.h:1029-1038`, `:921-932` |
//!
//! Two things are refused rather than guessed:
//!
//! * `project_remain` onto a scale with no origin of its own - map square or
//!   segment. The reference kills the build (`constexpr_fatal`, `:53-65`); the
//!   port has no `OriginOf` implementation, so it does not compile.
//! * `combine` where the fine origin does not match the coarse scale, and
//!   combining with a coarse scale that has no origin.
//!
//! The reference also refuses to combine two tripoints because it cannot say
//! which `z` wins. The port keeps a single three-dimensional point, so `combine`
//! takes the `z` of the coarse cell - the only reading that cannot disagree with
//! the reference, since `z` is never scaled.

use core::marker::PhantomData;

/// A coordinate scale: how many map squares one unit of this scale spans.
///
/// Implemented by zero-sized marker types so that mixing scales is a type error
/// rather than a runtime surprise.
pub trait Scale {
    /// Number of map squares covered by one unit at this scale.
    const MAP_SQUARES_PER: i32;
}

/// One map square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSquare;
/// One submap, `12` map squares across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Submap;
/// One overmap terrain tile, `24` map squares across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvermapTerrain;
/// One map segment, `32` overmap terrain tiles across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment;
/// One overmap, `180` overmap terrain tiles across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overmap;

impl Scale for MapSquare {
    const MAP_SQUARES_PER: i32 = 1;
}
impl Scale for Submap {
    const MAP_SQUARES_PER: i32 = 12;
}
impl Scale for OvermapTerrain {
    const MAP_SQUARES_PER: i32 = 24;
}
impl Scale for Segment {
    const MAP_SQUARES_PER: i32 = 768;
}
impl Scale for Overmap {
    const MAP_SQUARES_PER: i32 = 4320;
}

/// An origin: what the coordinate is measured from.
///
/// Reference: `origin` (`coords_fwd.h:13-20`). Only the ones that name a scale
/// can be the origin of a remainder or the fine half of a combination; the rest
/// are carried along and keep the type system honest.
pub mod origin {
    /// Measured from wherever the caller says - the only origin that may be
    /// added to another.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Relative;
    /// Measured from the corner of the world.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Absolute;
    /// Measured from the corner of a submap.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Submap;
    /// Measured from the corner of an overmap terrain tile.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OvermapTerrain;
    /// Measured from the corner of an overmap.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Overmap;
    /// Measured from the corner of a reality bubble.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Bubble;
}

pub use origin::Relative;

/// The origin a scale is measured from, when it has one.
///
/// Reference: `origin_from_scale()` (`coordinates.h:53-65`), which is fatal for
/// map squares and segments. Leaving the trait unimplemented for those two is how
/// the port refuses the same combinations at build time.
pub trait OriginOf: Scale {
    /// The origin of this scale.
    type Origin;
}

impl OriginOf for Submap {
    type Origin = origin::Submap;
}
impl OriginOf for OvermapTerrain {
    type Origin = origin::OvermapTerrain;
}
impl OriginOf for Overmap {
    type Origin = origin::Overmap;
}

/// Ties a fine origin to the coarse scale it may be combined with.
///
/// Implemented only for the pairs the reference accepts
/// (`origin_from_scale( CoarseScale ) == FineOrigin`, `coordinates.h:834`).
pub trait FineFor<CoarseScale: Scale> {}

impl FineFor<Submap> for origin::Submap {}
impl FineFor<OvermapTerrain> for origin::OvermapTerrain {}
impl FineFor<Overmap> for origin::Overmap {}

/// A point at a given origin and scale.
#[derive(Debug)]
pub struct Point<O, S: Scale> {
    x: i32,
    y: i32,
    z: i32,
    marker: PhantomData<(O, S)>,
}

// The marker types are phantom data, so a point is always copyable no matter
// what they are; deriving would wrongly require them to be `Copy` themselves.
impl<O, S: Scale> Clone for Point<O, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<O, S: Scale> Copy for Point<O, S> {}

// Equality, ordering and hashing see only the components, as the reference
// compares and hashes the raw point; the marker types take no part.
impl<O, S: Scale> PartialEq for Point<O, S> {
    fn eq(&self, other: &Self) -> bool {
        (self.x, self.y, self.z) == (other.x, other.y, other.z)
    }
}

impl<O, S: Scale> Eq for Point<O, S> {}

impl<O, S: Scale> PartialOrd for Point<O, S> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<O, S: Scale> Ord for Point<O, S> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.x, self.y, self.z).cmp(&(other.x, other.y, other.z))
    }
}

impl<O, S: Scale> core::hash::Hash for Point<O, S> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
    }
}

impl<O, S: Scale> Point<O, S> {
    /// Builds a point from raw components.
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self {
            x,
            y,
            z,
            marker: PhantomData,
        }
    }

    /// The horizontal component along x.
    #[must_use]
    pub const fn x(&self) -> i32 {
        self.x
    }

    /// The horizontal component along y.
    #[must_use]
    pub const fn y(&self) -> i32 {
        self.y
    }

    /// The horizontal components.
    #[must_use]
    pub const fn xy(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// The vertical component.
    #[must_use]
    pub const fn z(&self) -> i32 {
        self.z
    }

    /// Projects this point onto another scale, keeping the origin and the
    /// vertical coordinate.
    ///
    /// # Panics
    ///
    /// Fails to compile when the two scales are not multiples of one another.
    #[must_use]
    pub fn project_to<R: Scale>(self) -> Point<O, R> {
        const {
            assert!(
                S::MAP_SQUARES_PER % R::MAP_SQUARES_PER == 0
                    || R::MAP_SQUARES_PER % S::MAP_SQUARES_PER == 0,
                "projecting between scales that are not multiples of each other"
            );
        }
        if R::MAP_SQUARES_PER >= S::MAP_SQUARES_PER {
            // The result is coarser: one result unit spans several source units.
            let factor = R::MAP_SQUARES_PER / S::MAP_SQUARES_PER;
            Point::new(self.x.div_euclid(factor), self.y.div_euclid(factor), self.z)
        } else {
            let factor = S::MAP_SQUARES_PER / R::MAP_SQUARES_PER;
            Point::new(self.x * factor, self.y * factor, self.z)
        }
    }

    /// Splits this point into the coarse cell it falls in and the offset from
    /// that cell's corner.
    ///
    /// Reference: `project_remain` (`coordinates.h:795-810`). The quotient keeps
    /// this point's origin; the remainder is measured from the coarse cell, so
    /// its origin is the one the coarse scale names - splitting absolute submap
    /// coordinates by overmap terrain gives an absolute quotient and a remainder
    /// relative to that terrain tile. The offset is never negative, and
    /// `quotient * cell + remainder` restores the source exactly.
    ///
    /// # Panics
    ///
    /// Fails to compile when the target scale is not coarser, or is coarser but
    /// has no origin of its own (map squares, segments).
    #[must_use]
    pub fn project_remain<R: OriginOf>(self) -> QuotientRemainder<O, R, R::Origin, S> {
        const {
            assert!(
                R::MAP_SQUARES_PER % S::MAP_SQUARES_PER == 0
                    && R::MAP_SQUARES_PER > S::MAP_SQUARES_PER,
                "project_remain only projects to coarser scales"
            );
        }
        let factor = R::MAP_SQUARES_PER / S::MAP_SQUARES_PER;
        let quotient = self.project_to::<R>();
        QuotientRemainder {
            quotient,
            remainder: Point::new(
                self.x - quotient.x * factor,
                self.y - quotient.y * factor,
                self.z,
            ),
        }
    }

    /// The circle with a box, truncating toward zero.
    ///
    /// Reference: `midpoint` (`coordinates.h:1029-1038`), which divides the sum
    /// of the raw components by two. That division truncates toward zero, so the
    /// midpoint of `-1` and `0` is `0`, not `-1` - which is what
    /// [`i32::midpoint`] does, without the overflow the plain sum would have.
    #[must_use]
    pub const fn midpoint(self, other: Self) -> Self {
        Point::new(
            self.x.midpoint(other.x),
            self.y.midpoint(other.y),
            self.z.midpoint(other.z),
        )
    }

    /// Chebyshev distance: the longest single axis.
    ///
    /// Reference: `square_dist` (`src/line.h:182-192`).
    #[must_use]
    pub const fn square_dist(self, other: Self) -> i32 {
        let dx = (self.x - other.x).abs();
        let dy = (self.y - other.y).abs();
        let dz = (self.z - other.z).abs();
        let mut out = dx;
        if dy > out {
            out = dy;
        }
        if dz > out {
            out = dz;
        }
        out
    }

    /// Manhattan distance: the sum of the axes.
    #[must_use]
    pub const fn manhattan_dist(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs() + (self.z - other.z).abs()
    }

    /// Straight-line distance, truncated to a whole number.
    ///
    /// Reference: `trig_dist` (`src/line.h:168-171`) computes in doubles and the
    /// coordinate overload (`coordinates.h:921-925`) returns an `int`, so the
    /// fraction is dropped: ten apart on two axes is `14`, not `15`.
    #[must_use]
    pub fn trig_dist(self, other: Self) -> i32 {
        let dx = f64::from(self.x - other.x);
        let dy = f64::from(self.y - other.y);
        let dz = f64::from(self.z - other.z);
        // The reference returns the `int` the caller asked for, dropping the
        // fraction rather than rounding.
        #[allow(clippy::cast_possible_truncation)]
        let out = (dx * dx + dy * dy + dz * dz).sqrt() as i32;
        out
    }
}

impl<O, S: Scale> core::fmt::Display for Point<O, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "({},{},{})", self.x, self.y, self.z)
    }
}

/// A coarse cell and the offset inside it, as produced by
/// [`Point::project_remain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotientRemainder<O, C: Scale, R, F: Scale> {
    /// The cell the source point falls in, at the coarse scale, keeping the
    /// source's origin.
    pub quotient: Point<O, C>,
    /// The offset from that cell's corner, at the fine scale, measured from the
    /// cell.
    pub remainder: Point<R, F>,
}

impl<O, C: Scale, R, F: Scale> QuotientRemainder<O, C, R, F> {
    /// Puts a split point back together.
    ///
    /// This is the inverse of [`Point::project_remain`] and is the only way to
    /// add the two halves, because their origins differ.
    #[must_use]
    pub fn combine(self) -> Point<O, F>
    where
        R: FineFor<C>,
    {
        combine(self.quotient, self.remainder)
    }
}

/// Puts a coarse cell and an offset inside it back into one point.
///
/// Reference: `project_combine` (`coordinates.h:826-845`). The fine point's
/// origin must be the one the coarse scale names, which is why the bound is on
/// the fine origin rather than on the scales.
///
/// The vertical coordinate comes from the coarse cell: the reference refuses to
/// combine two tripoints because it cannot choose, and since `z` is never scaled
/// the coarse one is the same number the fine one would carry.
#[must_use]
pub fn combine<CO, CS: Scale, FO, FS: Scale>(
    coarse: Point<CO, CS>,
    fine: Point<FO, FS>,
) -> Point<CO, FS>
where
    FO: FineFor<CS>,
{
    let factor = CS::MAP_SQUARES_PER / FS::MAP_SQUARES_PER;
    Point::new(
        coarse.x * factor + fine.x,
        coarse.y * factor + fine.y,
        coarse.z,
    )
}

/// The half-open box a coarse cell covers on a finer scale.
///
/// Reference: `project_bounds` (`coordinates.h:847-865`): the near corner is the
/// cell itself and the far corner is the cell plus one, both projected.
#[must_use]
pub fn project_bounds<F: Scale, O, C: Scale>(coarse: Point<O, C>) -> (Point<O, F>, Point<O, F>) {
    (
        coarse.project_to::<F>(),
        Point::<O, C>::new(coarse.x + 1, coarse.y + 1, coarse.z).project_to::<F>(),
    )
}
