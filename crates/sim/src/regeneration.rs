//! How fast a monster mends, and when the mending arrives.
//!
//! Reference: `monster::regenerate` (`src/monster.cpp:4094-4103`). A monster's hourly rate is the data's own `regenerates` when a type states one, and otherwise one of two
//! fallbacks: four times the flesh rate for a body that `REVIVES` and is not `DORMANT`, and the flesh rate itself for anything made of flesh or vegetable matter. The
//! reference then heals `roll_remainder( regen * dt )` - a **fractional** hourly rate turned into whole hit points by a draw, so a slow mending lands as a point every so
//! often instead of never.

use bevy_ecs::prelude::*;

/// How many turns an hour is.
///
/// Reference: `to_turns<int>( 1_hours )` - the game's turn is one second, so an hour is three thousand six hundred of them.
pub const TURNS_PER_HOUR: i32 = 3_600;

/// The hit points a monster mends in an hour.
///
/// Reference: `monster::regenerate` (`monster.cpp:4094-4103`), in the reference's own order: a stated rate first, then the reviving rate, then the flesh rate. `materials`
/// are the type's own, and `dormant` is the `DORMANT` flag that keeps a reviving body still.
#[must_use]
pub fn mending_per_hour(
    hit_points: i32,
    revives: bool,
    dormant: bool,
    materials: &[&str],
    stated: f64,
) -> f64 {
    if stated > 0.0 {
        return stated;
    }
    #[allow(clippy::cast_precision_loss)]
    let hp = f64::from(hit_points);
    if revives && !dormant {
        return 0.02 * hp;
    }
    if materials
        .iter()
        .any(|material| matches!(*material, "flesh" | "iflesh" | "veggy"))
    {
        return 0.005 * hp;
    }
    0.0
}

/// What one stretch of time mends, as whole hit points.
///
/// Reference: `roll_remainder( regen * to_turns<int>( dt ) / to_turns<int>( 1_hours ) )` - the rate is per hour, so the fraction of an hour is what the draw is scaled by,
/// and `roll_remainder` turns that fraction into a whole point with the fraction as its chance. A world with no dice mends **nothing**, which is the port's own convention:
/// a rule that needs a draw does not guess one.
pub fn mend_over(world: &mut World, per_hour: f64, turns: i32) -> i32 {
    if per_hour <= 0.0 || turns <= 0 {
        return 0;
    }
    let expected = per_hour * f64::from(turns) / f64::from(TURNS_PER_HOUR);
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return 0;
    };
    let whole = rolls.roll_remainder(expected);
    crate::singletons::put_rolls(world, rolls);
    whole
}
