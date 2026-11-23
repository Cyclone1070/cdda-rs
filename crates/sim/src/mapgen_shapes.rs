//! The shapes a `set` entry draws: a point, a Bresenham line, a filled square.
//!
//! Reference: `jmapgen_setmap::apply` (`mapgen.cpp:5622-5800`) draws its entry one of three ways, and the differences are the sort that only a transcription catches:
//!
//! - a **point** is one square, drawn from the entry's ranges;
//! - a **line** is `line_to( p1, p2 )` (`line.cpp:224-238`), which starts the walk at `p1` and returns the cells **after** it - so a line never writes its own first square;
//! - a **square** is `draw_square` (`drawing_primitives.cpp:22-36`), which swaps the corners into order and **fills** the rectangle, both corners included.
//!
//! The port keeps the three rules here rather than inside the mapgen writer, because a shape is geometry and has nothing to say about terrain.

/// One square on a chunk, as the pieces of the reference count them.
pub type Square = (i32, i32);

/// The squares a point entry draws: one, straight from its ranges.
#[must_use]
pub fn point(at: Square) -> Vec<Square> {
    vec![at]
}

/// The squares a line entry draws: the Bresenham walk **after** `p1`, which is what `line_to` returns.
///
/// Reference: `line.cpp:224-238`: `line_to` calls `bresenham( p1, p2, 0 )` and collects every cell the walk **arrives at**, so `p1` is only in the line when `p1 == p2`.
#[must_use]
pub fn line(from: Square, to: Square) -> Vec<Square> {
    let mut drawn = Vec::new();
    if from == to {
        drawn.push(from);
        return drawn;
    }
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let sx = dx.signum();
    let sy = dy.signum();
    let ax = dx.abs() * 2;
    let ay = dy.abs() * 2;
    let mut cursor = from;
    let mut t = 0;
    // The three branches are the reference's three, and the comparison is written as a `match` on the ordering because that is what the reference's three-way branch is:
    // `a.x == a.y` walks the diagonal one step at a time, and the other two carry the `t` accumulator.
    match ax.cmp(&ay) {
        std::cmp::Ordering::Equal => {
            while cursor.0 != to.0 {
                cursor = (cursor.0 + sx, cursor.1 + sy);
                drawn.push(cursor);
            }
        }
        std::cmp::Ordering::Greater => {
            while cursor.0 != to.0 {
                if t > 0 {
                    cursor.1 += sy;
                    t -= ax;
                }
                cursor.0 += sx;
                t += ay;
                drawn.push(cursor);
            }
        }
        std::cmp::Ordering::Less => {
            while cursor.1 != to.1 {
                if t > 0 {
                    cursor.0 += sx;
                    t -= ay;
                }
                cursor.1 += sy;
                t += ax;
                drawn.push(cursor);
            }
        }
    }
    drawn
}

/// The squares a square entry draws: the filled rectangle, both corners included.
///
/// Reference: `drawing_primitives.cpp:22-36`: the corners are swapped into order and every square between them is drawn - a **fill**, not an outline.
#[must_use]
pub fn square(corner: Square, opposite: Square) -> Vec<Square> {
    let (x1, x2) = if corner.0 <= opposite.0 {
        (corner.0, opposite.0)
    } else {
        (opposite.0, corner.0)
    };
    let (y1, y2) = if corner.1 <= opposite.1 {
        (corner.1, opposite.1)
    } else {
        (opposite.1, corner.1)
    };
    let mut drawn = Vec::new();
    for x in x1..=x2 {
        for y in y1..=y2 {
            drawn.push((x, y));
        }
    }
    drawn
}
