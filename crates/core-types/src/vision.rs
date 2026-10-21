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
