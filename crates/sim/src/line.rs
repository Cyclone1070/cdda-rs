//! The squares a line passes through.
//!
//! Reference: `line_to` (`line.cpp:224-239`) over `bresenham` (`line.cpp:26-71`, called with the error term `t = 0`). Two of its properties are the opposite of what a
//! reader expects, and both matter to the rules that use it:
//!
//! * **The origin is not included.** `bresenham` starts *on* the origin and reports each square it moves *to*, so a line from A to B is B's side of A - which is why the
//!   reference's cone can walk a ray and never hit the caster.
//! * **A line to nowhere is the square it is on.** `line_to` short-circuits a zero distance and returns the origin alone, which is why `dash` has to step over the first
//!   square only when it is the caster's own.
//!
//! The error term is integer arithmetic - no floating point anywhere - so two platforms must answer identically.

/// Every square on the straight line from `from` towards `to`, the origin **excluded**, or the origin alone when they are the same square.
///
/// Reference: `line_to( const point &p1, const point &p2, int t = 0 )` and `bresenham( p1, p2, t, interact )`. The walk moves along the longer axis and steps the shorter
/// one whenever the accumulated error `t` is positive, subtracting the longer axis' doubled slope and adding the shorter's - `a` being the slopes doubled to keep the
/// arithmetic whole.
#[must_use]
pub fn between(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    if from == to {
        return vec![from];
    }
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let sx = dx.signum();
    let sy = dy.signum();
    let ax = dx.abs() * 2;
    let ay = dy.abs() * 2;
    let mut t = 0;
    let mut at = from;
    let mut walked = Vec::new();
    if ax == ay {
        while at.0 != to.0 {
            at.1 += sy;
            at.0 += sx;
            walked.push(at);
        }
    } else if ax > ay {
        while at.0 != to.0 {
            if t > 0 {
                at.1 += sy;
                t -= ax;
            }
            at.0 += sx;
            t += ay;
            walked.push(at);
        }
    } else {
        while at.1 != to.1 {
            if t > 0 {
                at.0 += sx;
                t -= ay;
            }
            at.1 += sy;
            t += ax;
            walked.push(at);
        }
    }
    walked
}
