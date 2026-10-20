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

/// The sun's altitude at which it stops being fully night: -18 degrees.
pub const ASTRONOMICAL_DAWN: f32 = -18.0;
/// -12 degrees: the horizon is just distinguishable.
pub const NAUTICAL_DAWN: f32 = -12.0;
/// -6 degrees: colours in the sky.
pub const CIVIL_DAWN: f32 = -6.0;
/// -1 degree: the sun's disc is up.
pub const SUNRISE: f32 = -1.0;
/// How bright full daylight is.
pub const FULL_DAYLIGHT: f32 = 125.0;

/// How bright the sky is for a given solar altitude, in degrees.
///
/// Reference: `sun_light_at` (`src/calendar.cpp:381-402`), which is a piecewise curve
/// rather than a cosine: exponential from nothing to the low ambient over the three dawns,
/// exponential again up to the horizon, then linear to full daylight and flat after. The
/// split matters at the edges of the day, which is when players are looking.
#[must_use]
pub fn sun_light_from_altitude(altitude: f32) -> f32 {
    if altitude < ASTRONOMICAL_DAWN {
        0.0
    } else if altitude <= NAUTICAL_DAWN {
        // 0 to 3.7 as the sun climbs from -18 to -12.
        3.7 * ((altitude - ASTRONOMICAL_DAWN) / 6.0)
            .exp2()
            .mul_add(1.0, -1.0)
    } else if altitude <= CIVIL_DAWN {
        // 3.7 to 5.0 from -12 to -6.
        (5.0 - 3.7) * ((altitude - NAUTICAL_DAWN) / 6.0).exp2().mul_add(1.0, -1.0) + 3.7
    } else if altitude <= SUNRISE {
        // 5.0 to 60 from -6 to -1, and this is the steep one: sunrise happens fast.
        (60.0 - 5.0) * ((altitude - CIVIL_DAWN) / 5.0).exp2().mul_add(1.0, -1.0) + 5.0
    } else if altitude <= 60.0 {
        // Linear from 60 to 125 as the sun climbs to 60 degrees.
        (65.0 / 61.0) * altitude + 65.0 / 61.0 + 60.0
    } else {
        FULL_DAYLIGHT
    }
}

/// What the sky alone makes a tile, given the sun's altitude.
///
/// This joins the two halves of the light model that are ported: the sun's curve
/// (`src/calendar.cpp:381-402`) and the thresholds that turn a number into a level
/// (`src/lightmap.cpp:674-696`). It answers the outdoors; a tile under a roof gets its
/// quarters from the outside-light pass instead, because the sun does not reach it.
#[must_use]
pub fn open_sky_lit_level(altitude: f32) -> LitLevel {
    // No source in the tile: the sky is ambient light, not something burning underfoot.
    lit_level(true, 0.0, sun_light_from_altitude(altitude))
}
