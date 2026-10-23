//! What the character can actually see: the cast, limited by light.
//!
//! Reference: the draw path reads `map::seen` and `lm.apparent_light_at` together
//! (`src/game.cpp`, the map drawing loop), so a square in sight but in pitch darkness is not shown
//! at all. The two halves live in different places in the reference and are joined when the frame is
//! drawn; this module joins them the same way, as a query rather than a cache.
//!
//! The port keeps the visible-*and*-lit rule only. The reference's finer behaviour - remembering a
//! square's terrain after it goes dark, drawing it dimmed below `LIGHT_AMBIENT_LOW` - needs map
//! memory, which is not ported.

use crate::level::Level;
use crate::shadowcasting::FourQuadrants;
use std::collections::BTreeMap;

/// How much light reaches a square: the brightest of the four quadrants that cover it.
///
/// Reference: `level_cache::apparent_light_at` (`src/lightmap.cpp`), which adds the ambient light to
/// the brightest quadrant and clamps at the low and high ambient values.
#[must_use]
pub fn apparent_light_at(light: &BTreeMap<(i32, i32), FourQuadrants>, square: (i32, i32)) -> f32 {
    light.get(&square).map_or(0.0, FourQuadrants::max)
}

/// Whether a square is shown to the character: in the cast, and lit by something.
///
/// # Panics
///
/// Panics if the radius is negative, as [`Level::seen_at`] does.
pub fn shown_at(
    level: &mut Level,
    origin: (i32, i32),
    radius: i32,
    blocks_light: impl Fn(u32) -> bool,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    square: (i32, i32),
) -> bool {
    if apparent_light_at(light, square) <= 0.0 {
        return false;
    }
    level.seen_at(origin, radius, blocks_light, square)
}

/// What a square looks like to the character: the reference's `lit_level`.
///
/// Reference: `map::apparent_light_at` (`src/lightmap.cpp:801-851`), with the parts that need an avatar - clairvoyance, the camera cache, the recoil check, the impaired
/// range - left to the caller, which supplies the three facts they decide: whether the light got through only as haze (`obstructed`), whether anything is on the square
/// (`sees_items`), and the character's vision threshold.
///
/// `obstructed` is the reference's `a.obstructed`, which is true when the square's own `seen_cache` value is at or below solid: the light arrives as a glow rather than as
/// detail. `apparent_light` is the brightest quadrant of the light map.
#[must_use]
pub fn lit_level_at(
    apparent_light: f32,
    obstructed: bool,
    ground_light_level: f32,
    vision_threshold: f32,
    sees_items: bool,
) -> crate::light::LitLevel {
    use crate::light::{AMBIENT_LIT, LitLevel, SOURCE_BRIGHT};
    if obstructed {
        // Hazy: enough light to glow, not enough to make anything out.
        if apparent_light > AMBIENT_LIT && apparent_light > ground_light_level {
            return LitLevel::BrightOnly;
        }
        return LitLevel::Blank;
    }
    if apparent_light > SOURCE_BRIGHT || sees_items {
        return LitLevel::Bright;
    }
    if apparent_light > AMBIENT_LIT {
        return LitLevel::Lit;
    }
    if apparent_light >= vision_threshold {
        return LitLevel::Low;
    }
    LitLevel::Blank
}

/// Whether a square can be seen from another on the same level.
///
/// Reference: `map::sees`'s same-level branch (`src/map.cpp:7329-7362`). Sight is a Bresenham walk
/// from the viewer to the target that stops **before** testing the square it is looking at, so a
/// wall is seen as a wall and only what is behind it stays hidden.
///
/// The three gates come in the reference's order: the range, the bounds of the level, and then the
/// walk. `range` is negative for "no limit", which is the reference's own convention. A line that
/// leaves the level is blocked from the square where it leaves.
#[must_use]
pub fn sees_on_one_level(
    from: (i32, i32),
    to: (i32, i32),
    range: i32,
    transparent: impl Fn(i32, i32) -> bool,
    in_bounds: impl Fn(i32, i32) -> bool,
) -> bool {
    if range >= 0 && range < rl_dist(from, to) {
        return false;
    }
    if !in_bounds(to.0, to.1) {
        return false;
    }
    for step in crate::line::line_to(
        crate::line::Point::new(from.0, from.1),
        crate::line::Point::new(to.0, to.1),
        0,
    ) {
        // The square being looked at is seen even when it is what stops the line: a wall is visible as a wall.
        if step.x == to.0 && step.y == to.1 {
            return true;
        }
        if !in_bounds(step.x, step.y) || !transparent(step.x, step.y) {
            return false;
        }
    }
    // An empty walk means the viewer is looking at their own square, which they can see.
    true
}

/// The distance the reference's range check uses.
///
/// Reference: `rl_dist` (`src/line.cpp`), which is Chebyshev distance unless the "circular distances"
/// option is on. The option is off by default and the port has no options yet, so this is the
/// square-ring distance and the option is a declared gap.
fn rl_dist(from: (i32, i32), to: (i32, i32)) -> i32 {
    (from.0 - to.0).abs().max((from.1 - to.1).abs())
}
