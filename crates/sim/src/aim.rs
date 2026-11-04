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

/// What a gun's sights are worth in a shooter's hands, and how fast those sights would bring the aim down.
///
/// Reference: `item::sight_dispersion` (`item.cpp:11134-11160`) hands the gun's own figure to `Character::effective_dispersion`, and `modified_sight_speed` takes the result as its
/// `effective_sight_dispersion`. The port joins the two here so that the chain - **gun data, shooter's eyes, recoil, speed** - is one call with the reference's own four numbers in it.
///
/// `DISABLE_SIGHTS`, which the reference treats as a flat 300, is not read: the flag is in the item data and its handling is named in `docs/parity.md` rather than guessed.
#[must_use]
pub fn sight_report(
    gun: &cdda_content::guns::GunType,
    perception: i32,
    recoil: i32,
    zoom: bool,
) -> (i32, f64) {
    let dispersion = effective_dispersion(gun.sight_dispersion, perception, zoom);
    let speed = sight_speed_with_attenuation(0.0, f64::from(dispersion), f64::from(recoil));
    (dispersion, speed)
}

/// The smallest improvement the reference admits: `MIN_RECOIL_IMPROVEMENT` = **0.01** (`ranged.h:24`), there "just to prevent data anomalies".
pub const MIN_RECOIL_IMPROVEMENT: f64 = 0.01;

/// How much aim one move buys, which is the rule that **spends** recoil.
///
/// Reference: `Character::aim_per_move` (`character.cpp:1146-1210`), transcribed in its own order:
///
/// 1. the base is **ten** plus the sights' speed from [`sight_speed_with_attenuation`] - the reference keeps the ten "to prevent extreme rate changes as aim speed approaches 0";
/// 2. **skill divides**, by `max(1, 2.5 - 0.2 * skill)`, so an unskilled shooter aims at two fifths speed and a five-skill one at full;
/// 3. the recoil **fades the speed in**: `max( recoil / MAX_RECOIL, 1 - logarithmic_range( 0, MAX_RECOIL, recoil ) )`, which is the reference's replacement for an older logarithmic
///    attenuation - it is near zero at rest, near one at the cap, and takes whichever of its two terms is larger;
/// 4. a **cap** from skill, `5 + skill + max(10, 3 * skill)`, which usually only binds in the first half of the aiming;
/// 5. the whole thing is scaled by **2.4**, floored at [`MIN_RECOIL_IMPROVEMENT`], and finally **limited by the sights**: `min( aim_speed, recoil - limit )`, so a shooter whose recoil is
///    already inside what the sights permit gains nothing.
///
/// The reference's three modifier hooks (`aim_speed_skill_mod`, `aim_speed_dex_mod`, `aim_speed_mod`) and its volume and length factors are **one, one, one and one in the port**: no effect the
/// port carries states them, and no gun barrel is modelled yet. They are named in `docs/parity.md` rather than guessed at.
#[must_use]
pub fn aim_per_move(recoil: f64, limit: f64, skill: i32, sight_speed: f64) -> f64 {
    let max_recoil = f64::from(crate::shot::MAX_RECOIL);
    let mut aim_speed = 10.0 + sight_speed;

    // Skill divides, and never multiplies back: `max( 1.0, ... )` is what makes five skill the break-even point.
    aim_speed /= (2.5 - 0.2 * f64::from(skill)).max(1.0);

    // The recoil fades the speed in, and the `max` of the two terms is the reference's, not a lerp.
    #[allow(clippy::cast_possible_truncation)]
    let at = recoil as i64;
    #[allow(clippy::cast_possible_truncation)]
    let cap = max_recoil as i64;
    let fade = (recoil / max_recoil).max(1.0 - logarithmic_range(0, cap, at));
    aim_speed *= fade;

    // The skill cap, then the reference's flat scale.
    let skill_cap = 5.0 + f64::from(skill) + (3.0 * f64::from(skill)).max(10.0);
    aim_speed = aim_speed.min(skill_cap);
    aim_speed *= 2.4;

    aim_speed.max(MIN_RECOIL_IMPROVEMENT).min(recoil - limit)
}
