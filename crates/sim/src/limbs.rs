//! What a body part is worth once it is hurt or carrying something.
//!
//! Reference: the three adjustments `bodypart::get_limb_score` applies to a part's own figure before the creature sums them (`src/bodypart.cpp:967-1018`). They are kept as three small
//! functions rather than one because they are asked separately as well as together, and because each has a threshold that is a rule in its own right.

/// What a part's figure is worth once it is hurt.
///
/// Reference: `bodypart::wound_adjusted_limb_value` (`bodypart.cpp:967-976`) - above **three quarters** of its health the part is worth its whole figure; below, the figure is scaled by
/// `health / 0.75`, so a part at a quarter of its health is worth a third of what it was. The reference keeps the arithmetic in `double` and answers a `float`, which is why a part at half
/// health answers two thirds rather than something that rounds to it.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn wound_adjusted_limb_value(value: f32, health: i32, health_max: i32) -> f32 {
    if health_max <= 0 {
        return value;
    }
    let percent = f64::from(health) / f64::from(health_max);
    if percent > 0.75 {
        return value;
    }
    let scaled = f64::from(value) * (percent / 0.75);
    scaled as f32
}

/// What a part's figure is worth once something is in the way of it.
///
/// Reference: `bodypart::encumb_adjusted_limb_value` (`bodypart.cpp:978-990`): **zero** for a limb over its limit, otherwise `value * 19 / ( 19 + load / 3 )` past the part's threshold. The curve
/// starts at one, is a **twentieth** down at three points over, and **never reaches zero** - which is exactly why the reference needs the over-limit flag beside it.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn encumb_adjusted_limb_value(value: f32, load: i32, threshold: i32, over_the_limit: bool) -> f32 {
    if over_the_limit {
        return 0.0;
    }
    let past = (load - threshold).max(0);
    let scaled = f64::from(value) * 19.0 / (19.0 + f64::from(past) / 3.0);
    scaled as f32
}

/// What a part's figure is worth once the creature is skilled with it.
///
/// Reference: `bodypart::skill_adjusted_limb_value` (`bodypart.cpp:992-996`) - `min( value, value * ( 1 + skill / 10 ) )`. **The minimum is inert**: the second figure is larger than the first for
/// every non-negative skill, so the reference answers its own input. That is what this port copies, because the reference's behaviour is the behaviour - a port that made skill help here would
/// answer differently from the game it ported.
#[must_use]
pub fn skill_adjusted_limb_value(value: f32, skill: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let mitigated = value * (1.0 + skill as f32 / 10.0);
    value.min(mitigated)
}
