//! How much light there is, and what the game calls it.
//!
//! Reference: `src/lightmap.h:9-19` for the thresholds, `LIGHT_RANGE` at
//! `src/lightmap.h:42-49`, and `map::light_at` at `src/lightmap.cpp:674-696`, which
//! turns a light value into the level everything else reads.

/// A dim light source - a match, a candle stub.
pub const SOURCE_LOCAL: f32 = 0.1;
/// A bright light source - a floodlight, the sun in your face.
pub const SOURCE_BRIGHT: f32 = 10.0;

/// Below this the outdoors is dark enough to matter.
pub const AMBIENT_MINIMAL: f32 = 3.7;
/// Below this there is not enough light to see by.
pub const AMBIENT_LOW: f32 = 3.5;
/// Overcast daylight.
pub const AMBIENT_DIM: f32 = 5.0;
/// Full daylight.
pub const AMBIENT_LIT: f32 = 10.0;

/// How many map squares air stays legible for: `SEEX * HALF_MAPSIZE`, 72.
pub const MAX_VIEW_DISTANCE: i32 = 72;
/// Cumulative light lost per map square of open air, `2.3 / MAX_VIEW_DISTANCE`.
pub const TRANSPARENCY_OPEN_AIR: f32 = 2.3 / 72.0;

/// What a tile's light means to the rest of the game.
///
/// The reference's `lit_level` (`src/lightmap.h:53-61`) has two more entries -
/// `MEMORIZED` and `BLANK` - which are not light levels but states of the map
/// overlay, so they are not part of what this function can answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitLevel {
    /// Nothing to see by.
    Dark,
    /// Hard to see.
    Low,
    /// Full daylight or better.
    Lit,
    /// A light source is right here.
    Bright,
}

/// How far a light of `brightness` reaches, in map squares.
///
/// Zero when there is nothing worth calling light: the reference returns 0 at or
/// below `AMBIENT_LOW` rather than a negative number.
#[must_use]
pub fn light_range(brightness: f32) -> i32 {
    if brightness <= AMBIENT_LOW {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let range = (-(AMBIENT_LOW / brightness).ln() * (1.0 / TRANSPARENCY_OPEN_AIR)) as i32;
    range
}

/// What the game calls the light at a tile.
///
/// * `light_source` is the brightest light source sitting *in* the tile, which is the
///   reference's `sm` cache.
/// * `ambient` is the brightest quadrant of the tile's light map, its `lm.max()`.
///
/// # Panics
///
/// Never; every value has a level, and a tile outside the map is dark.
#[must_use]
pub fn lit_level(in_bounds: bool, light_source: f32, ambient: f32) -> LitLevel {
    // Out of bounds is dark before anything else is considered, which is the first
    // line of the reference's `map::light_at`.
    if !in_bounds {
        return LitLevel::Dark;
    }
    if light_source >= SOURCE_BRIGHT {
        return LitLevel::Bright;
    }
    if ambient >= AMBIENT_LIT {
        return LitLevel::Lit;
    }
    if ambient >= AMBIENT_LOW {
        return LitLevel::Low;
    }
    LitLevel::Dark
}
