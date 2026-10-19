//! Coordinates: a point, an origin, and a scale, kept distinct in the type
//! system.
//!
//! Reference: `src/coordinates.h`. The scale factors match
//! `map_squares_per()` (`coordinates.h:32`) and the constants in
//! `src/map_scale_constants.h`. Projection matches `project_to_impl`
//! (`coordinates.h:601-644`): multiplying when the result is finer, and
//! **flooring toward negative infinity** when it is coarser, via
//! `divide_xy_round_to_minus_infinity` (`src/point.h:146`). The vertical
//! coordinate is never scaled.

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
/// Only the relative origin exists so far; absolute and bubble origins arrive
/// with the world phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relative;

/// A point at a given origin and scale.
#[derive(Debug, PartialEq, Eq)]
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
    /// that cell's corner, both keeping this point's origin.
    ///
    /// Reference: `project_remain` (`coordinates.h:796-810`). The offset is
    /// therefore never negative, and `quotient * cell + remainder` restores the
    /// source exactly.
    ///
    /// The remainder's origin is the coarse cell, not the world; a distinct
    /// origin type arrives with the absolute coordinates in the world phase.
    ///
    /// # Panics
    ///
    /// Fails to compile when the target scale is not coarser than this one.
    #[must_use]
    pub fn project_remain<R: Scale>(self) -> QuotientRemainder<O, R, S> {
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
}

/// A coarse cell and the offset inside it, as produced by
/// [`Point::project_remain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotientRemainder<O, C: Scale, F: Scale> {
    /// The cell the source point falls in, at the coarse scale.
    pub quotient: Point<O, C>,
    /// The offset from that cell's corner, at the fine scale.
    pub remainder: Point<O, F>,
}
