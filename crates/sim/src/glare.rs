//! When the sun is too bright to look at, and what it does to the eyes that are looking.
//!
//! Reference: `glare` (`src/weather.cpp:100-138`). The reference runs it from the weather's own turn pass, over the player only - its comment says "todo npcs, also" - and the port
//! keeps the rule rather than the sweep: a caller asks what glare a creature would take, and the creature's own state is handed in rather than read from a global character.

use bevy_ecs::prelude::Component;

/// The sun's brightness at which a **winter** sun glares: `irradiance::moderate` (`weather.h:64-66`), the sun at 35 degrees on a clear day.
const MODERATE: f32 = 573.0;

/// The sun's brightness at which any other sun glares: `irradiance::high` (`weather.h:69-71`), the sun at 45 degrees.
const HIGH: f32 = 707.0;

/// What stops glare before it starts, and what makes it worse.
///
/// Reference: the five early returns and the one trait multiplier in `glare` (`weather.cpp:104-135`). **A struct rather than five bools**: a function taking six true-or-false arguments is
/// a function nobody can read at the call site, and a misspelt one is a bug that compiles.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Conditions {
    /// The creature is under a roof (`g->is_sheltered`).
    pub sheltered: bool,
    /// The creature is asleep (`in_sleep_state`).
    pub asleep: bool,
    /// Something worn carries `SUN_GLASSES` (`worn_with_flag`).
    pub sun_glasses: bool,
    /// The creature carries `GLARE_RESIST`.
    pub glare_resist: bool,
    /// The creature is blind.
    pub blind: bool,
    /// The creature already has the effect this sun would give it, which shortens the next one.
    pub already_glared: bool,
    /// The creature has `CEPH_VISION`, which doubles how long glare lasts.
    pub ceph_vision: bool,
}

/// Which effect the sun gives, and for how many turns.
///
/// Reference: `glare` (`weather.cpp:110-137`), whose shape the port keeps exactly:
///
/// ```text
/// if( season == WINTER && incident_sun_irradiance > irradiance::moderate )  effect = snow_glare;   // 573
/// else if( incident_sun_irradiance > irradiance::high )                     effect = glare;        // 707
/// dur = has_effect( effect ) ? 10 : 20;
/// if( CEPH_VISION ) dur *= 2;
/// ```
///
/// **The winter branch is an `else if`**, so a winter sun strong enough for either takes the **snow** effect and never both - and the winter threshold is **573, not 707**, which is what
/// makes the same sun glare in winter and not in summer. The thresholds are **exclusive**: a square at exactly 707 is not glared.
#[must_use]
pub fn glare_of(
    winter: bool,
    irradiance: f32,
    conditions: Conditions,
) -> Option<(&'static str, i32)> {
    if conditions.sheltered
        || conditions.asleep
        || conditions.sun_glasses
        || conditions.glare_resist
        || conditions.blind
    {
        return None;
    }
    let effect = if winter && irradiance > MODERATE {
        "snow_glare"
    } else if irradiance > HIGH {
        "glare"
    } else {
        return None;
    };
    let mut duration = if conditions.already_glared { 10 } else { 20 };
    if conditions.ceph_vision {
        duration *= 2;
    }
    Some((effect, duration))
}

/// Marks a creature that glare is currently burning.
///
/// Reference: `effect_glare` and `effect_snow_glare` are effects like any other, so the port keeps them in the effect store. This component exists only so a reader can see the two
/// effect names the rule produces in one place; it carries no state of its own.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SunGlared;
