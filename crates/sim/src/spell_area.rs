//! Which squares a spell covers.
//!
//! Reference: `spell_effect::spell_effect_area` (`magic_spell_effect.cpp:229-296`) and the two shape machines beside it, `spell_effect_cone_range_override` (`:253-295`)
//! and `spell_effect_line` (`:308-420`). A spell's `shape` is **not** decoration: 221 of the content's 245 spells are blasts, 15 are cones and 9 are lines, and a blast's
//! square covers ground a cone never reaches.
//!
//! The port has the blast and the cone. The **line** shape is named in `docs/parity.md` as unported: its machine wraps an iterable path and offsets it along two legs, which
//! is a round of its own, and until then a line-shaped spell covers its blast - which is what the port did for every shape before this module existed.

/// The squares a shaped spell covers, given where it was cast from and where it landed.
///
/// `passable` answers whether a square lets a ray through, which the reference asks the map (`test_passable`). A cone stops its rays at the first wall; a blast never asks.
#[must_use]
pub fn cover(
    shape: &str,
    source: (i32, i32, i32),
    target: (i32, i32, i32),
    area: i32,
    range: i32,
    passable: &dyn Fn((i32, i32, i32)) -> bool,
    ignore_walls: bool,
) -> Vec<(i32, i32, i32)> {
    match shape {
        // A cone respects walls unless the spell carries `IGNORE_WALLS`; both the flag and the passability test come from the world, and the gate walks a ray into a wall
        // with the flag off and through it with the flag on.
        "cone" => cone(source, target, area, range, ignore_walls, passable),
        // A line runs from the caster towards the aim point, as wide as the spell's area, and stops at anything impassable unless the spell ignores walls.
        "line" => {
            if ignore_walls {
                crate::spell_line::line((source.0, source.1), (target.0, target.1), area, &|_| true, target.2)
            } else {
                crate::spell_line::line((source.0, source.1), (target.0, target.1), area, passable, target.2)
            }
        }
        _ => blast(target, area),
    }
}

/// A square area: every square within `area` on both axes, on the target's level.
///
/// Reference: `spell_effect_blast` (`magic_spell_effect.cpp:200-228`): the radius is the spell's area, and a blast is square because `trigdist` is not consulted for it.
#[must_use]
pub fn blast(target: (i32, i32, i32), area: i32) -> Vec<(i32, i32, i32)> {
    let mut squares = Vec::new();
    for dx in -area..=area {
        for dy in -area..=area {
            squares.push((target.0 + dx, target.1 + dy, target.2));
        }
    }
    squares
}

/// A cone from `source` towards `target`, `area` degrees wide and `range` squares long.
///
/// Reference: `spell_effect_cone_range_override` (`magic_spell_effect.cpp:253-295`):
///
/// * the direction is `coord_to_angle( source, target )`, which is **radians** and normalised into one turn;
/// * the width is half the spell's area in degrees on either side of it, stepped **one degree at a time**;
/// * every square within the range along each of those angles is a candidate, and unless walls are ignored each candidate's line from the source is walked only while
///   the ground is passable, stopping at the first square that is not;
/// * and the source itself is removed, because a blast that hits its own caster is not a blast.
///
/// The port takes the reference's default `CIRCLEDIST` of **true**, so the rays are `cos` and `sin` of the angle rather than the square approximation beside them in
/// `calc_ray_end`; the option is a name the port does not have yet and `docs/parity.md` records that.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn cone(
    source: (i32, i32, i32),
    target: (i32, i32, i32),
    area: i32,
    range: i32,
    ignore_walls: bool,
    passable: &dyn Fn((i32, i32, i32)) -> bool,
) -> Vec<(i32, i32, i32)> {
    let mut targets = std::collections::BTreeSet::new();
    if range <= 0 {
        return Vec::new();
    }
    let initial = angle_of(source, target);
    let half_width = (f64::from(area) / 2.0).to_radians();
    let step = 1.0_f64.to_radians();
    let mut angle = initial - half_width;
    let end_angle = initial + half_width;
    let mut reachable = Vec::new();
    // The reference's loop is `for( angle = start; angle <= end; angle += 1_degrees )`, an accumulation in radians - kept as one so the two land on the same angles.
    while angle <= end_angle {
        for step_out in 1..=range {
            let square = ray_end(angle, step_out, source);
            if ignore_walls {
                targets.insert(square);
            } else {
                reachable.push(square);
            }
        }
        angle += step;
    }
    if !ignore_walls {
        for end in reachable {
            for square in crate::line::between((source.0, source.1), (end.0, end.1)) {
                let along = (square.0, square.1, source.2);
                if !passable(along) {
                    break;
                }
                targets.insert(along);
            }
        }
    }
    targets.remove(&source);
    targets.into_iter().collect()
}

/// The direction from one square to another, in radians, normalised into one turn.
///
/// Reference: `coord_to_angle` (`line.cpp:996-1003`): `atan2` of the difference, plus a full turn when it comes out negative - so east is zero and everything else climbs
/// from there.
#[must_use]
pub fn angle_of(from: (i32, i32, i32), to: (i32, i32, i32)) -> f64 {
    let mut angle = f64::from(to.1 - from.1).atan2(f64::from(to.0 - from.0));
    if angle < 0.0 {
        angle += std::f64::consts::TAU;
    }
    angle
}

/// The square `range` steps from `source` along `angle`.
///
/// Reference: `calc_ray_end` (`line.cpp:966-992`) in its `trigdist` branch: the angle is brought into one turn first, then `p + range * cos( angle )` and
/// `p + range * sin( angle )` are **truncated** to whole squares - `static_cast` toward zero, which the port's `as i32` does too.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn ray_end(angle: f64, range: i32, source: (i32, i32, i32)) -> (i32, i32, i32) {
    let mut angle = angle % std::f64::consts::TAU;
    if angle < 0.0 {
        angle += std::f64::consts::TAU;
    }
    (
        source.0 + (f64::from(range) * angle.cos()) as i32,
        source.1 + (f64::from(range) * angle.sin()) as i32,
        source.2,
    )
}
