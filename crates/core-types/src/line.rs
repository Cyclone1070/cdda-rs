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


/// The cells a line passes through in **three** dimensions, from `from` towards `to`, excluding `from`.
///
/// Reference: `line_to( const tripoint &loc1, const tripoint &loc2, int t, int t2 )` and the three-dimensional `bresenham` it calls (`src/line.cpp:73-215, 241-257`).
/// The reference's own walk is written out as **ten** branches - a flat line where one axis is unchanged splits three ways by which of the other two dominates, and a line
/// that climbs splits seven ways by which axis dominates and which two are equal - and so is this one, for the same reason the reference keeps them apart: the branches
/// step differently, and a reader has to be able to hold one against the other.
///
/// `t` and `t2` are the starting error terms. They are passed in rather than normalised because the reference passes them in, and because they decide which neighbour
/// is taken first when a step sits exactly on a tie - `line_to( …, 3 )` and `line_to( …, -3 )` part company on their very first cell.
///
/// A line whose target is its origin is not empty: the walk yields that one cell, which is the reference's `numCells == 0` branch.
#[must_use]
pub fn line_to_3d(from: [i32; 3], to: [i32; 3], t: i32, t2: i32) -> Vec<[i32; 3]> {
    // `line_to`'s own `numCells == 0` branch: a line that goes nowhere yields the origin rather than an empty walk.
    if from == to {
        return vec![from];
    }
    let mut walk = Walk::new(from, to, t, t2);
    if walk.twice_z == 0 {
        // A flat line: three branches, chosen by which of the two level axes is longer. The equal case walks the diagonal.
        match walk.twice_x.cmp(&walk.twice_y) {
            core::cmp::Ordering::Equal => walk.flat_diagonal(),
            core::cmp::Ordering::Greater => walk.flat_along_x(),
            core::cmp::Ordering::Less => walk.flat_along_y(),
        }
    } else if walk.twice_x == walk.twice_y && walk.twice_y == walk.twice_z {
        walk.diagonal_3d();
    } else if walk.twice_z > walk.twice_x && walk.twice_z > walk.twice_y {
        walk.climbing_z_dominant();
    } else if walk.twice_x == walk.twice_y {
        walk.climbing_x_and_y_equal();
    } else if walk.twice_x == walk.twice_z {
        walk.climbing_x_and_z_equal();
    } else if walk.twice_y == walk.twice_z {
        walk.climbing_y_and_z_equal();
    } else if walk.twice_x > walk.twice_y {
        walk.climbing_x_dominant();
    } else {
        // The reference's last branch: `dy > dx >= dz`.
        walk.climbing_y_dominant();
    }
    walk.cells
}

/// One three-dimensional walk in progress: where it is, where it is going, and the two error terms.
///
/// The reference keeps this state in locals inside a 140-line function. Splitting it out is what lets each branch be its own method, which is close to how
/// `src/line.cpp:73-215` reads - one block per case, each stepping the same three coordinates.
struct Walk {
    cx: i32,
    cy: i32,
    cz: i32,
    tx: i32,
    ty: i32,
    tz: i32,
    sx: i32,
    sy: i32,
    sz: i32,
    /// `|d| * 2` per axis, which keeps the walk in integers.
    twice_x: i32,
    twice_y: i32,
    twice_z: i32,
    t: i32,
    t2: i32,
    cells: Vec<[i32; 3]>,
}

impl Walk {
    fn new(from: [i32; 3], to: [i32; 3], t: i32, t2: i32) -> Self {
        let [cx, cy, cz] = from;
        let [tx, ty, tz] = to;
        let (dx, dy, dz) = (tx - cx, ty - cy, tz - cz);
        Self {
            cx,
            cy,
            cz,
            tx,
            ty,
            tz,
            sx: sign_of(dx),
            sy: sign_of(dy),
            sz: sign_of(dz),
            twice_x: dx.abs() * 2,
            twice_y: dy.abs() * 2,
            twice_z: dz.abs() * 2,
            t,
            t2,
            cells: Vec::new(),
        }
    }

    fn step(&mut self) {
        self.cells.push([self.cx, self.cy, self.cz]);
    }

    fn flat_diagonal(&mut self) {
        while self.cx != self.tx {
            self.cy += self.sy;
            self.cx += self.sx;
            self.step();
        }
    }

    fn flat_along_x(&mut self) {
        while self.cx != self.tx {
            if self.t > 0 {
                self.cy += self.sy;
                self.t -= self.twice_x;
            }
            self.cx += self.sx;
            self.t += self.twice_y;
            self.step();
        }
    }

    fn flat_along_y(&mut self) {
        while self.cy != self.ty {
            if self.t > 0 {
                self.cx += self.sx;
                self.t -= self.twice_y;
            }
            self.cy += self.sy;
            self.t += self.twice_x;
            self.step();
        }
    }

    fn diagonal_3d(&mut self) {
        while self.cx != self.tx {
            self.cx += self.sx;
            self.cy += self.sy;
            self.cz += self.sz;
            self.step();
        }
    }

    fn climbing_z_dominant(&mut self) {
        while self.cz != self.tz {
            if self.t > 0 {
                self.cx += self.sx;
                self.t -= self.twice_z;
            }
            if self.t2 > 0 {
                self.cy += self.sy;
                self.t2 -= self.twice_z;
            }
            self.cz += self.sz;
            self.t += self.twice_x;
            self.t2 += self.twice_y;
            self.step();
        }
    }

    fn climbing_x_and_y_equal(&mut self) {
        while self.cx != self.tx {
            if self.t > 0 {
                self.cz += self.sz;
                self.t -= self.twice_x;
            }
            self.cy += self.sy;
            self.cx += self.sx;
            self.t += self.twice_z;
            self.step();
        }
    }

    fn climbing_x_and_z_equal(&mut self) {
        while self.cx != self.tx {
            if self.t > 0 {
                self.cy += self.sy;
                self.t -= self.twice_x;
            }
            self.cz += self.sz;
            self.cx += self.sx;
            self.t += self.twice_y;
            self.step();
        }
    }

    fn climbing_y_and_z_equal(&mut self) {
        while self.cy != self.ty {
            if self.t > 0 {
                self.cx += self.sx;
                self.t -= self.twice_z;
            }
            self.cy += self.sy;
            self.cz += self.sz;
            self.t += self.twice_x;
            self.step();
        }
    }

    fn climbing_x_dominant(&mut self) {
        while self.cx != self.tx {
            if self.t > 0 {
                self.cy += self.sy;
                self.t -= self.twice_x;
            }
            if self.t2 > 0 {
                self.cz += self.sz;
                self.t2 -= self.twice_x;
            }
            self.cx += self.sx;
            self.t += self.twice_y;
            self.t2 += self.twice_z;
            self.step();
        }
    }

    fn climbing_y_dominant(&mut self) {
        while self.cy != self.ty {
            if self.t > 0 {
                self.cx += self.sx;
                self.t -= self.twice_y;
            }
            if self.t2 > 0 {
                self.cz += self.sz;
                self.t2 -= self.twice_y;
            }
            self.cy += self.sy;
            self.t += self.twice_x;
            self.t2 += self.twice_z;
            self.step();
        }
    }
}

/// The sign of a difference for the three-dimensional walk, where a **zero** difference has no sign at all.
///
/// The two-dimensional walk's [`sign`] counts zero as positive because the reference's `SGN` macro does; the three-dimensional walk writes its own
/// `d.x() == 0 ? 0 : sgn( d.x() )`, so zero means "this axis does not step".
const fn sign_of(value: i32) -> i32 {
    if value < 0 {
        -1
    } else if value > 0 {
        1
    } else {
        0
    }
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
