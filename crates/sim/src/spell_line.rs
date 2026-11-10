//! The **line** shape: a spell that runs a band from the caster to where it was aimed.
//!
//! Reference: `spell_effect::spell_effect_line` (`magic_spell_effect.cpp:308-444`) and the three helpers it is built from, `spell_detail::line_iterable`,
//! `spell_detail::side_of` and `spell_detail::build_line` (`:101-157`).
//!
//! The idea the code is built around is a **wrapped iteration**: the canonical Bresenham path from the source to the target is walked forward, and when it runs off the
//! end it starts again from the next parallel path - which is what makes a widened line a band rather than a set of separate segments. `line_iterable` is exactly that:
//! an index into the path plus an origin that steps by the delta whenever the index wraps.
//!
//! Three rules of the reference's geometry that a transcription has to keep, and that a tidier implementation would lose:
//!
//! 1. **The band is bounded by two perpendiculars**, one through the source and one through the target (`between_or_on`), not by a length along the path.
//! 2. **The width is split unevenly**: `ccw_len = aoe / 2` and `cw_len = aoe - ccw_len`, so an odd width leans clockwise rather than centring.
//! 3. **The source square is erased at the end** (`result.erase( source )`), so a line never hits the caster.
//!
//! **Named gaps**: the reference scales the width by `( |dx| + |dy| ) / dist` when the `trigdist` option is off, and the port fixes `CIRCLEDIST` on, so that branch is not
//! transcribed. `ignore_walls` is set by an `IGNORE_WALLS` spell flag the port does not load, so a line always stops at an impassable square.

use std::collections::BTreeSet;

/// A canonical path walked with wrapping, which is how the reference builds a widened line.
///
/// Reference: `spell_detail::line_iterable` (`magic_spell_effect.cpp:103-130`). `next` and `prev` wrap the index and step the origin whenever they cross the end, so the
/// iteration continues onto the next parallel path instead of stopping.
#[derive(Debug, Clone)]
struct LineIterable {
    /// The path from the origin, as offsets.
    delta_line: Vec<(i32, i32)>,
    /// Where this pass of the path starts.
    cur_origin: (i32, i32),
    /// How far one pass is from the next.
    delta: (i32, i32),
    /// Where along the path the iteration is.
    index: usize,
}

impl LineIterable {
    /// A walk of a path, starting at its beginning from `origin`.
    fn new(origin: (i32, i32), delta: (i32, i32), delta_line: Vec<(i32, i32)>) -> Self {
        Self {
            delta_line,
            cur_origin: origin,
            delta,
            index: 0,
        }
    }

    /// The square this iteration is on.
    fn get(&self) -> (i32, i32) {
        let step = self.delta_line.get(self.index).copied().unwrap_or((0, 0));
        (self.cur_origin.0 + step.0, self.cur_origin.1 + step.1)
    }

    /// Moves one step along the path, wrapping to the next pass when it runs out.
    fn next(&mut self) {
        let length = self.delta_line.len().max(1);
        self.index = (self.index + 1) % length;
        if self.index == 0 {
            self.cur_origin = (self.cur_origin.0 + self.delta.0, self.cur_origin.1 + self.delta.1);
        }
    }

    /// Moves one step back, wrapping to the previous pass when it runs out at the front.
    fn prev(&mut self) {
        let length = self.delta_line.len().max(1);
        if self.index == 0 {
            self.cur_origin = (self.cur_origin.0 - self.delta.0, self.cur_origin.1 - self.delta.1);
        }
        self.index = (self.index + length - 1) % length;
    }

    /// Puts the iteration on a new pass.
    fn reset(&mut self, origin: (i32, i32)) {
        self.cur_origin = origin;
        self.index = 0;
    }
}

/// Which side of the line from `a` to `b` the point `c` is on: `1` clockwise, `-1` the other way, `0` on the line.
///
/// Reference: `spell_detail::side_of` (`magic_spell_effect.cpp:132-136`), the sign of a cross product.
fn side_of(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i32 {
    let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    i32::from(cross > 0) - i32::from(cross < 0)
}

/// Whether a point is between the two perpendiculars, or on either of them.
///
/// Reference: `spell_detail::between_or_on` (`magic_spell_effect.cpp:138-142`): the point is inside when it is not clockwise of the near line and not counter-clockwise of
/// the far one.
fn between_or_on(a0: (i32, i32), a1: (i32, i32), d: (i32, i32), c: (i32, i32)) -> bool {
    side_of(a0, (a0.0 + d.0, a0.1 + d.1), c) != 1
        && side_of(a1, (a1.0 + d.0, a1.1 + d.1), c) != -1
}

/// Walks one pass of the path, recording squares until the band's edge or an obstruction stops it.
///
/// Reference: `spell_detail::build_line` (`magic_spell_effect.cpp:144-156`): the walk continues **while** the square is inside the band, and a square that fails the
/// caller's test ends it - so a wall truncates the line rather than being skipped.
fn build_line(
    mut line: LineIterable,
    source: (i32, i32, i32),
    delta: (i32, i32),
    delta_perp: (i32, i32),
    passable: &dyn Fn((i32, i32, i32)) -> bool,
    result: &mut BTreeSet<(i32, i32)>,
) {
    while between_or_on((0, 0), delta, delta_perp, line.get()) {
        let at = line.get();
        if !passable((source.0 + at.0, source.1 + at.1, source.2)) {
            break;
        }
        result.insert(at);
        line.next();
    }
}

/// The squares a line spell covers, as offsets from the source.
///
/// Reference: `spell_effect::spell_effect_line` (`magic_spell_effect.cpp:308-444`). `source` and `target` are on the same level; `width` is the spell's `aoe`.
#[must_use]
pub fn line(
    source: (i32, i32),
    target: (i32, i32),
    width: i32,
    passable: &dyn Fn((i32, i32, i32)) -> bool,
    at_level: i32,
) -> Vec<(i32, i32, i32)> {
    let delta = (target.0 - source.0, target.1 - source.1);
    let dist = delta.0.abs().max(delta.1.abs());
    if dist == 0 {
        return Vec::new();
    }
    let delta_perp = (-delta.1, delta.0);
    let axis_delta = if delta.0.abs() > delta.1.abs() {
        (delta.0, 0)
    } else {
        (0, delta.1)
    };
    let cw_perp_axis = (-axis_delta.1, axis_delta.0);
    let unit_cw_perp_axis = (cw_perp_axis.0.signum(), cw_perp_axis.1.signum());
    // Reference: `int ccw_len = params.aoe_radius / 2; int cw_len = params.aoe_radius - ccw_len;` - an uneven split towards the clockwise side.
    let ccw_len = width / 2;
    let cw_len = width - ccw_len;
    let delta_side = side_of((0, 0), axis_delta, delta);
    let origin = (0, 0);
    // Reference: `line_to( zero, delta )`, with the far end removed and the near end put back in front - which leaves the origin **twice** at the head of the path. That
    // duplication is load-bearing: the wrap-forward in `next` depends on it.
    let mut path = crate::line::between(origin, delta);
    path.pop();
    let mut path_with_origin: Vec<(i32, i32)> = Vec::with_capacity(path.len() + 2);
    path_with_origin.push(origin);
    path_with_origin.push(origin);
    path_with_origin.extend(path);
    let base_line = LineIterable::new(origin, delta, path_with_origin);

    let mut result: BTreeSet<(i32, i32)> = BTreeSet::new();
    build_line(base_line.clone(), (source.0, source.1, at_level), delta, delta_perp, passable, &mut result);

    if delta_side == 0 {
        // Axis aligned: straight legs each way.
        for p in crate::line::between(origin, (unit_cw_perp_axis.0 * cw_len, unit_cw_perp_axis.1 * cw_len)) {
            let mut line = base_line.clone();
            line.reset(p);
            if !passable((source.0 + p.0, source.1 + p.1, at_level)) {
                break;
            }
            build_line(line, (source.0, source.1, at_level), delta, delta_perp, passable, &mut result);
        }
        for p in crate::line::between(origin, (unit_cw_perp_axis.0 * -ccw_len, unit_cw_perp_axis.1 * -ccw_len)) {
            let mut line = base_line.clone();
            line.reset(p);
            if !passable((source.0 + p.0, source.1 + p.1, at_level)) {
                break;
            }
            build_line(line, (source.0, source.1, at_level), delta, delta_perp, passable, &mut result);
        }
    } else if delta_side == 1 {
        // The delta leans clockwise of the primary axis: the counter-clockwise leg starts behind the perpendicular and is walked forward.
        for p in crate::line::between(origin, (unit_cw_perp_axis.0 * -ccw_len, unit_cw_perp_axis.1 * -ccw_len)) {
            let mut line = base_line.clone();
            line.reset(p);
            while side_of(origin, delta_perp, line.get()) == 1 {
                line.next();
            }
            if !passable((source.0 + p.0, source.1 + p.1, at_level)) {
                break;
            }
            build_line(line, (source.0, source.1, at_level), delta, delta_perp, passable, &mut result);
        }
        for p in crate::line::between(origin, (unit_cw_perp_axis.0 * cw_len, unit_cw_perp_axis.1 * cw_len)) {
            let mut line = base_line.clone();
            line.reset(p);
            while side_of(origin, delta_perp, line.get()) != 1 {
                line.prev();
            }
            line.next();
            if !passable((source.0 + p.0, source.1 + p.1, at_level)) {
                break;
            }
            build_line(line, (source.0, source.1, at_level), delta, delta_perp, passable, &mut result);
        }
    } else {
        // The delta leans counter-clockwise: the two legs swap which way they are walked.
        for p in crate::line::between(origin, (unit_cw_perp_axis.0 * -ccw_len, unit_cw_perp_axis.1 * -ccw_len)) {
            let mut line = base_line.clone();
            line.reset(p);
            while side_of(origin, delta_perp, line.get()) != 1 {
                line.prev();
            }
            line.next();
            if !passable((source.0 + p.0, source.1 + p.1, at_level)) {
                break;
            }
            build_line(line, (source.0, source.1, at_level), delta, delta_perp, passable, &mut result);
        }
        for p in crate::line::between(origin, (unit_cw_perp_axis.0 * cw_len, unit_cw_perp_axis.1 * cw_len)) {
            let mut line = base_line.clone();
            line.reset(p);
            while side_of(origin, delta_perp, line.get()) == 1 {
                line.next();
            }
            if !passable((source.0 + p.0, source.1 + p.1, at_level)) {
                break;
            }
            build_line(line, (source.0, source.1, at_level), delta, delta_perp, passable, &mut result);
        }
    }

    // Reference: `result.erase( source )` - a line never hits the caster.
    result.remove(&origin);
    result
        .into_iter()
        .map(|(x, y)| (source.0 + x, source.1 + y, at_level))
        .collect()
}
