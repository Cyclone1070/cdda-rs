//! Aiming: how fast a shooter can bring the sights down, and how far "point shooting" goes before sights are needed.
//!
//! Reference: `modified_sight_speed` and `point_shooting_limit` (`src/character.cpp:955-987`). Both are arithmetic over numbers the port already has - a shooter's recoil from round 1152,
//! the skill a gun names from round 1144, and the sight dispersion the sights would give - and neither needs the aiming **activity** to be useful: they answer "how fast would this improve"
//! and "when do I stop gaining from grinding the sights off".

use crate::morale::logarithmic_range;

/// The error a shooter's own eyes add to a set of sights: `Character::get_character_parallax` (`character.cpp:946-953`).
///
/// **Perception is the whole of it**, at the coefficient `ranged_per_mod` already uses (`(20 - per) * 1.2`, round 1136), and a **zoomed** sight takes only a quarter of it - which is what
/// "zoom" means here: the same eyes, a narrower view, four times less error. The reference's `character_modifier_ranged_dispersion_vision_mod` is one point zero in the port because no effect
/// it carries states one, and that is said rather than left to look like an omission.
///
/// The result is clamped at zero **after rounding**, so a perception above twenty cannot make sights better than the sights.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn character_parallax(perception: i32, zoom: bool) -> i32 {
    let dispersion = crate::dispersion::from_perception(perception);
    let scaled = if zoom {
        f64::from(dispersion) * 0.25
    } else {
        f64::from(dispersion)
    };
    let rounded = scaled.round() as i32;
    rounded.max(0)
}

/// What a set of sights is worth in a given shooter's hands: `Character::effective_dispersion` (`character.cpp:941-944`).
///
/// It is one addition - the sights' own figure plus the shooter's parallax - and the whole point of keeping it as a function is that **the same sights are worth different numbers to different
/// shooters**, which is the rule a port is most likely to flatten by storing a single "sight dispersion" per gun.
#[must_use]
pub fn effective_dispersion(sight_dispersion: i32, perception: i32, zoom: bool) -> i32 {
    character_parallax(perception, zoom) + sight_dispersion
}

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
