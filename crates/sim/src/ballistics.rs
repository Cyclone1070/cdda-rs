//! Where a shot lands: the error angle it drew, and the square a wild miss walks to.
//!
//! Reference: `projectile_attack_roll` and the miss branch of `projectile_attack` (`src/ballistics.cpp:204-352`), with the triangle in `iso_tangent` (`src/line.cpp:21-24`).
//! The port keeps the **arithmetic** and not the reference's `dealt_projectile_attack` record: what a caller needs is where the shot stopped and how badly it missed, and
//! those are a square and a number.

/// How far a shot missed, in tiles, and as a fraction of the target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aim {
    /// The error angle the shot drew, in arcminutes - the reference's dispersion unit.
    pub dispersion: f64,
    /// How far off the aim that angle puts the shot at this range, in tiles.
    pub missed_by_tiles: f64,
    /// The same error as a fraction of the target's size, capped at a whole miss.
    pub missed_by: f64,
}

/// An arcminute count as radians: sixty to a degree, and a degree is pi over a hundred and eighty.
fn arcmin_to_radians(arcmin: f64) -> f64 {
    arcmin / 60.0 * std::f64::consts::PI / 180.0
}

/// The width of an isosceles triangle at `distance`, given half-angle `radians`.
///
/// Reference: `iso_tangent` (`src/line.cpp:21-24`): `tan( vertex / 2 ) * distance * 2`. The vertex is the whole error angle, so the tangent takes half of it.
#[must_use]
pub fn iso_tangent(distance: f64, radians: f64) -> f64 {
    (radians / 2.0).tan() * distance * 2.0
}

/// What a shot's drawn error comes to at this range, against a target this wide.
///
/// Reference: `projectile_attack_roll` (`src/ballistics.cpp:204-231`): the rolled dispersion is an angle, the angle is an isosceles triangle over the range, and the miss is
/// that width over the target's size - capped at one whole miss. A target with no size is a full miss, which is the reference's guard against dividing by zero.
#[must_use]
pub fn aim_of(rolled_dispersion: f64, range: f64, target_size: f64) -> Aim {
    let missed_by_tiles = iso_tangent(range, arcmin_to_radians(rolled_dispersion));
    let missed_by = if target_size > 0.0 {
        // `std::min( 1.0, aim.missed_by_tiles / target_size )`
        (missed_by_tiles / target_size).min(1.0)
    } else {
        1.0
    };
    Aim {
        dispersion: rolled_dispersion,
        missed_by_tiles,
        missed_by,
    }
}

/// A shot on its way: where it came from, where it was aimed, and how badly it missed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    /// Where it was thrown from.
    pub source: (i32, i32, i32),
    /// The square it was aimed at.
    pub target: (i32, i32, i32),
    /// How far that is, in squares.
    pub range: i32,
    /// How badly it missed.
    pub aim: Aim,
    /// Whether the shot stops at the square it was aimed at rather than flying on - true for a throw at open ground.
    pub no_overshoot: bool,
}

/// The square a shot stops on.
///
/// Reference: the miss branch of `projectile_attack` (`src/ballistics.cpp:314-352`). A miss under a tile leaves the aim alone and spends no dice. A wider miss turns the
/// aim by the error angle - **capped at thirty degrees** - and walks the range out by a random amount bounded by the square root of the miss, so a shot that misses by four
/// tiles can land two squares short or two squares long. A shot that would land on its own square is nudged one square along the aim instead, because a shot at your own
/// square is refused outright by the reference.
pub fn landing(shot: &Shot, dice: &mut cdda_core_types::rng::Rolls) -> (i32, i32, i32) {
    let (sx, sy, sz) = shot.source;
    let (tx, ty, _) = shot.target;
    if shot.target == shot.source || shot.aim.missed_by_tiles < 1.0 {
        return shot.target;
    }
    let dx = f64::from(tx - sx);
    let dy = f64::from(ty - sy);
    // The reference caps a wild miss at thirty degrees either way, and turns by the shot's **rolled** angle rather than by the triangle's width.
    let capped = arcmin_to_radians(shot.aim.dispersion).min(std::f64::consts::PI / 6.0);
    let turn = if dice.one_in(2) { capped } else { -capped };
    let angle = dy.atan2(dx) + turn;
    // `std::min<int>( range, std::sqrt( aim.missed_by_tiles ) )` - the reach of the walk, in squares.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let offset = shot.range.min(shot.aim.missed_by_tiles.sqrt() as i32);
    let new_range = if shot.no_overshoot {
        shot.range + dice.rng(-offset, offset)
    } else {
        dice.rng(shot.range - offset, shot.range)
    }
    .max(1);
    #[allow(clippy::cast_possible_truncation)]
    let walked = (
        sx + dice.roll_remainder(f64::from(new_range) * angle.cos()),
        sy + dice.roll_remainder(f64::from(new_range) * angle.sin()),
        sz,
    );
    if walked == shot.source {
        // Reference: a shot that lands back on the thrower is pushed one square along the aim - the sign of the offset, not the turned angle.
        return (sx + sgn(tx - sx), sy + sgn(ty - sy), sz);
    }
    walked
}

/// The sign of a number, as the reference's `sgn` writes it: zero stays zero.
fn sgn(value: i32) -> i32 {
    value.signum()
}

/// What share of its tile a creature of this size fills: `occupied_tile_fraction` (`src/ranged.cpp:603-622`).
///
/// These five numbers are the target size the aim roll divides by, so they are the difference between a shot that is counted as a graze and one that is counted as a clean miss against
/// the same error. The reference falls back to the medium fraction after a `debugmsg`, which is what a size the port cannot name gets here.
#[must_use]
pub fn occupied_tile_fraction(size: crate::CreatureSize) -> f64 {
    match size {
        crate::CreatureSize::Tiny => 0.1,
        crate::CreatureSize::Small => 0.25,
        crate::CreatureSize::Medium => 0.5,
        crate::CreatureSize::Large => 0.75,
        crate::CreatureSize::Huge => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cap is thirty degrees, and the reference writes it as `30_degrees` rather than as arcminutes.
    #[test]
    fn a_wild_miss_turns_by_no_more_than_thirty_degrees() {
        let cap = std::f64::consts::PI / 6.0;
        assert!((arcmin_to_radians(1_800.0) - cap).abs() < 1e-12, "thirty degrees is 1800 arcminutes");
        assert!(arcmin_to_radians(90_000.0).min(cap) < cap + 1e-12);
    }
}
