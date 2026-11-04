//! Aiming: how fast a shooter can bring the sights down, and how far "point shooting" goes before sights are needed.
//!
//! Reference: `modified_sight_speed` and `point_shooting_limit` (`src/character.cpp:955-987`). Both are arithmetic over numbers the port already has - a shooter's recoil from round 1152,
//! the skill a gun names from round 1144, and the sight dispersion the sights would give - and neither needs the aiming **activity** to be useful: they answer "how fast would this improve"
//! and "when do I stop gaining from grinding the sights off".

use crate::morale::logarithmic_range;

/// How fast aiming improves while the shooter is still shaking, attenuated by how far past the sights' own precision the recoil is.
///
/// Reference: `modified_sight_speed` (`character.cpp:955-975`):
///
/// * **nothing improves while the recoil is already inside the sights' precision** - `recoil <= effective_sight_dispersion` returns zero, which is what "the sights cannot improve on this"
///   means;
/// * a negative sight dispersion returns zero as well, which is the reference's guard rather than a rule;
/// * otherwise the speed is `(10 + aim_speed_modifier)` scaled by `1 - logarithmic_range( eff, 3 * eff + 1, recoil )` - so the bonus **tends to zero** as the recoil approaches the sights'
///   precision, and reaches full strength once the recoil is past three times it plus one. The `+ 1` is the reference's own, and its comment says why: without it the range would be
///   `min == max` at zero sight dispersion.
#[must_use]
pub fn sight_speed_with_attenuation(
    aim_speed_modifier: f64,
    effective_sight_dispersion: f64,
    recoil: f64,
) -> f64 {
    if recoil <= effective_sight_dispersion || effective_sight_dispersion < 0.0 {
        return 0.0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let low = effective_sight_dispersion as i64;
    #[allow(clippy::cast_possible_truncation)]
    let high = (3.0 * effective_sight_dispersion + 1.0) as i64;
    #[allow(clippy::cast_possible_truncation)]
    let at = recoil as i64;
    let attenuation = 1.0 - logarithmic_range(low, high, at);
    (10.0 + aim_speed_modifier) * attenuation
}

/// How much recoil a shooter can shrug off by point shooting, before the sights matter at all.
///
/// Reference: `point_shooting_limit` (`character.cpp:977-987`). The two branches are **the skill's, not the weapon's**: archery divides, so a bow's limit falls from 250 at nothing to 50 at
/// ten (it approaches 30 as skill grows without bound), and every other gun skill subtracts ten a level from 200. The reference's comment says why perception is not in it - "the accuracy of
/// aim shooting depends more on muscle memory and skill".
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn point_shooting_limit(skill_level: i32, archery: bool) -> i32 {
    let skill = f64::from(skill_level.min(10));
    if archery {
        (30.0 + 220.0 / (1.0 + skill)) as i32
    } else {
        (200.0 - 10.0 * skill) as i32
    }
}
