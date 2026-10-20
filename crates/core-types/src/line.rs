//! Walking a line from one cell to another.
//!
//! Reference: `src/line.cpp` (`line_to`, `bresenham`) and the canonical copy the
//! reference keeps to test against, `tests/line_test.cpp:17-88`. The two differ
//! only in how they are called; the walk below is that walk, step for step,
//! including its tie-breaking, because a line that is one cell different quietly
//! changes what a bullet hits and what a player sees.

/// A cell on a two-dimensional grid.
///
/// The reference calls this `point` and keeps `tripoint` separate, so the walk is
/// written over two numbers. The bubble points in [`crate::coords`] are a
/// different animal - three numbers, an origin and a scale - and project onto
/// this one at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    /// The x component.
    pub x: i32,
    /// The y component.
    pub y: i32,
}

impl Point {
    /// Builds a point from its components.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// The sign of a difference, as the reference's `SGN` macro: zero counts as
/// positive, which is what its `(a < 0) ? -1 : 1` says.
const fn sign(value: i32) -> i32 {
    if value < 0 { -1 } else { 1 }
}

/// The cells a line passes through, from `from` towards `to`, excluding `from`.
///
/// `t` is the reference's starting error term: it decides which neighbour is
/// taken first when a line is exactly on a tie, and `0` is the ordinary choice.
/// It is passed straight into the walk rather than normalised, because the
/// reference passes it straight in too (`tests/line_test.cpp:430`).
///
/// The three walks below are the reference's three branches: an exact diagonal,
/// a line wider than it is tall, and one taller than it is wide. They are kept
/// written out rather than folded into one, because the reference's own copies
/// (`tests/line_test.cpp:47-71`) are written out too, and reading them side by
/// side is how a divergence gets spotted.
///
/// A line whose target is its origin is not empty - the walk always enters its
/// body once - so it yields that one cell.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn line_to(from: Point, to: Point, starting_error: i32) -> Vec<Point> {
    let mut line = Vec::new();
    let difference = Point::new(to.x - from.x, to.y - from.y);
    let doubled = Point::new(difference.x.abs() << 1, difference.y.abs() << 1);
    let mut step = Point::new(sign(difference.x), sign(difference.y));
    if difference.y == 0 {
        step.y = 0;
    }
    if difference.x == 0 {
        step.x = 0;
    }

    let mut current = from;
    let mut error = starting_error;

    // The reference widens the target box by the size of the step so the walk
    // cannot run past it, and uses that as its loop guard.
    let mut low = Point::new(from.x.min(to.x), from.y.min(to.y));
    low.x -= difference.x.abs();
    low.y -= difference.y.abs();
    let high = Point::new(
        from.x.max(to.x) + difference.x.abs(),
        from.y.max(to.y) + difference.y.abs(),
    );

    let in_bounds = |point: Point| {
        point.x >= low.x && point.x <= high.x && point.y >= low.y && point.y <= high.y
    };
    let arrived = |point: Point| point.x == to.x && point.y == to.y;

    // The lint would fold these three into a `match` on the comparison; the
    // reference writes three branches and this reads line for line against it.
    #[allow(clippy::comparison_chain)]
    if doubled.x == doubled.y {
        loop {
            current.y += step.y;
            current.x += step.x;
            line.push(current);
            if arrived(current) || !in_bounds(current) {
                break;
            }
        }
    } else if doubled.x > doubled.y {
        loop {
            if error > 0 {
                current.y += step.y;
                error -= doubled.x;
            }
            current.x += step.x;
            error += doubled.y;
            line.push(current);
            if arrived(current) || !in_bounds(current) {
                break;
            }
        }
    } else {
        loop {
            if error > 0 {
                current.x += step.x;
                error -= doubled.y;
            }
            current.y += step.y;
            error += doubled.x;
            line.push(current);
            if arrived(current) || !in_bounds(current) {
                break;
            }
        }
    }
    line
}
