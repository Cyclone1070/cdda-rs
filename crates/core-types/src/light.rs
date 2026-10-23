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
/// A transparency of zero: nothing gets through. Not a small number, but the value that means "a wall".
pub const TRANSPARENCY_SOLID: f32 = 0.0;

/// What a tile's light means to the rest of the game.
///
/// Reference: `enum class lit_level` (`src/lightmap.h:55-63`). All seven values are here, including the two that are not brightnesses: `MEMORIZED` behaves like a light
/// level, and `BLANK` is what `map::apparent_light_at` answers for a square whose detail cannot be made out. An earlier version of this enum had four, on the stated
/// grounds that the other two were overlay state; `apparent_light_at` returns `BLANK` and `BRIGHT_ONLY`, so they are values this code produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitLevel {
    /// Nothing to see by.
    Dark,
    /// Hard to see.
    Low,
    /// Bright, but without enough detail to make anything out.
    BrightOnly,
    /// Full daylight or better.
    Lit,
    /// A light source is right here.
    Bright,
    /// Remembered rather than seen. Not a light level, but it behaves like one.
    Memorized,
    /// Nothing drawn at all.
    Blank,
}

/// What the player can make of a square once the light is known.
///
/// Reference: `enum class visibility_type` (`src/enums.h:128-135`). A boomer's bile is the only thing that changes the answer, and it changes it for every value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityType {
    /// Nothing is drawn: the square is not there to the player.
    Hidden,
    /// Seen plainly.
    Clear,
    /// Lit, but not seen - the player can tell it is bright and nothing more.
    Lit,
    /// Seen through bile.
    Boomer,
    /// Dark, and the player knows it.
    Dark,
    /// Dark through bile.
    BoomerDark,
}

/// What a square at a known light level looks like to the player.
///
/// Reference: `map::get_visibility` (`src/map.cpp:6808-6849`). The switch has five arms for seven values, because `LOW`, `LIT` and `BRIGHT` all answer `CLEAR` - the
/// light level itself is not what the drawing code needs, only whether the square can be seen. `boomered` stands in for the reference's whole `visibility_variables`
/// cache, which holds exactly one fact that changes this answer.
#[must_use]
pub fn visibility(level: LitLevel, boomered: bool) -> VisibilityType {
    match level {
        LitLevel::Dark => {
            if boomered {
                VisibilityType::BoomerDark
            } else {
                VisibilityType::Dark
            }
        }
        LitLevel::BrightOnly => {
            if boomered {
                VisibilityType::Boomer
            } else {
                VisibilityType::Lit
            }
        }
        LitLevel::Low | LitLevel::Lit | LitLevel::Bright => VisibilityType::Clear,
        LitLevel::Blank | LitLevel::Memorized => {
            if boomered {
                VisibilityType::BoomerDark
            } else {
                VisibilityType::Hidden
            }
        }
    }
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

/// What the player can see with: their distance to a square, and the three things that change what that distance means.
///
/// Reference: the player half of `map::apparent_light_at` (`lightmap.cpp:801-857`) - `rl_dist`, `u_clairvoyance`, `unimpaired_range()` and
/// `get_vision_threshold()`. Grouping them means the decision tree below takes one argument instead of four, and it is the half a test can state without a character.
#[derive(Debug, Clone, Copy)]
pub struct Eyes {
    /// How far away the square is, in the reference's own distance metric (`rl_dist`).
    pub dist: i32,
    /// How far clairvoyance reaches, or zero for none.
    pub clairvoyance: i32,
    /// How far the player can see without help.
    pub unimpaired_range: i32,
    /// The dimmest light this player can still see by.
    pub vision_threshold: f32,
}

/// What the light map says about one square.
///
/// Reference: `map::apparent_light_info` (`lightmap.h`) together with the cache values the decision tree reads - `sm` (the light source sitting in the square),
/// `camera_cache` and `g_light_level` (the ambient light outside).
#[derive(Debug, Clone, Copy)]
pub struct Apparent {
    /// Whether the line of sight to this square is blocked.
    pub obstructed: bool,
    /// Whether it is blocked for every eye, cameras included.
    pub absolute_obstructed: bool,
    /// How much light reaches the square: `apparent_light_helper`'s answer, the brightest quadrant the player can see it from.
    pub light: f32,
    /// The light source sitting in the square itself, the reference's `sm` cache.
    pub light_source: f32,
    /// Light from a camera at the square, which the reference keeps apart from the player's own sight.
    pub camera: f32,
    /// The ambient light **outside**, `g_light_level`.
    pub ambient: f32,
}

/// What the player can make of one square.
///
/// Reference: `map::apparent_light_at` (`lightmap.cpp:801-857`), in its order. The order is the behaviour:
///
/// 1. clairvoyance reaching the square sees it, whatever is in the way;
/// 2. a square past the player's unimpaired range and without a camera is `BRIGHT_ONLY` if it is itself a light source and nothing blocks it absolutely, and blank
///    otherwise;
/// 3. an obstructed square is `BRIGHT_ONLY` only when enough light gets through **and** that light beats the ambient light outside - otherwise it is blank;
/// 4. an unobstructed square is `BRIGHT` above `LIGHT_SOURCE_BRIGHT` or with a light source in it, `LIT` above `LIGHT_AMBIENT_LIT`, `LOW` at or above the player's
///    vision threshold, and blank below it.
#[must_use]
pub fn apparent_light(eyes: &Eyes, square: &Apparent) -> LitLevel {
    if eyes.clairvoyance > 0 && eyes.dist <= eyes.clairvoyance {
        return LitLevel::Bright;
    }
    if eyes.dist > eyes.unimpaired_range && square.camera == 0.0 {
        if !square.absolute_obstructed && square.light_source > 0.0 {
            return LitLevel::BrightOnly;
        }
        return LitLevel::Blank;
    }
    if square.obstructed {
        if square.light > AMBIENT_LIT && square.light > square.ambient {
            return LitLevel::BrightOnly;
        }
        return LitLevel::Blank;
    }
    if square.light > SOURCE_BRIGHT || square.light_source > 0.0 {
        return LitLevel::Bright;
    }
    if square.light > AMBIENT_LIT {
        return LitLevel::Lit;
    }
    if square.light >= eyes.vision_threshold {
        return LitLevel::Low;
    }
    LitLevel::Blank
}

/// Which side of a square a light comes from or is looked at from.
///
/// Reference: `enum class quadrant` (`src/shadowcasting.h:24-30`). The order is **not** clockwise from north: it is NE, SE, SW, NW, which is north-east then
/// east's two then west's two. A port that ordered these clockwise would be a rotation of the truth, and every quadrant product would pair the wrong sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quadrant {
    /// Up and to the right.
    NorthEast = 0,
    /// Down and to the right.
    SouthEast = 1,
    /// Down and to the left.
    SouthWest = 2,
    /// Up and to the left.
    NorthWest = 3,
}

/// Four values, one per quadrant.
///
/// Reference: `struct four_quadrants` (`src/shadowcasting.h:38-70`), which is the light map's per-square value: light does not arrive at a wall uniformly, it arrives
/// from wherever the lamps are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quadrants([f32; 4]);

/// Builds a set of four quadrants, as the reference's `four_quadrants( float )` and `four_quadrants{...}` constructors do.
#[must_use]
pub fn four_quadrants(values: [f32; 4]) -> Quadrants {
    Quadrants(values)
}

// Every index below comes from `quadrant as usize`, and the enum has exactly four variants numbered 0 to 3, so the arithmetic cannot leave the array. The allow is
// here rather than a `match` because a `match` would let a future fifth variant compile and silently answer with a stale quadrant.
#[allow(clippy::indexing_slicing)]
impl Quadrants {
    /// Builds a set of four quadrants.
    #[must_use]
    pub fn new(values: [f32; 4]) -> Self {
        Self(values)
    }

    /// One quadrant's value.
    #[must_use]
    pub fn get(self, quadrant: Quadrant) -> f32 {
        self.0[quadrant as usize]
    }

    /// Sets one quadrant's value.
    pub fn set(&mut self, quadrant: Quadrant, value: f32) {
        self.0[quadrant as usize] = value;
    }

    /// The brightest quadrant, which is what a square light passes through answers with.
    #[must_use]
    pub fn max(self) -> f32 {
        self.0.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    }

    /// The quadrant values.
    #[must_use]
    pub fn values(self) -> [f32; 4] {
        self.0
    }
}

/// What one of the eight neighbours says about being seen.
///
/// Reference: the three tests the helper applies to a neighbour (`lightmap.cpp:779-786`): is it inside the lightmap, is it opaque, and can anybody see it - the last
/// being its distance against the player's unimpaired range and its seen and camera values.
#[derive(Debug, Clone, Copy)]
pub struct SeenFrom {
    /// How visible the neighbour is, the reference's `seen_cache`.
    pub seen: f32,
    /// Light from a camera at the neighbour, which the reference keeps apart from the player's sight.
    pub camera: f32,
    /// How far the neighbour is from the player, the reference's `rl_dist`.
    pub dist: i32,
}

/// The eight neighbours of a square, in the order the reference walks them.
///
/// Reference: `adjacent_offsets` (`lightmap.cpp:754-771`): south, north, east, south-east, north-east, west, south-west, north-west. The order does not change the
/// answer here - each neighbour only ever raises quadrant values - but keeping it means a divergence shows up as the same neighbour in both.
#[derive(Debug, Clone, Copy)]
pub struct Neighbourhood {
    /// Whether each neighbour is inside the lightmap at all.
    pub in_bounds: [bool; 8],
    /// Whether each neighbour is opaque, in which case it cannot be seen past.
    pub opaque: [bool; 8],
    /// How far the player can see without help, in squares.
    pub unimpaired_range: i32,
    /// What each neighbour says about being seen.
    pub neighbours: [SeenFrom; 8],
}

/// The quadrants each neighbour lights, in the reference's own order.
///
/// Reference: `adjacent_offsets` (`lightmap.cpp:754-771`). A **cardinal** neighbour lights two quadrants - the south neighbour lights `SE` and `SW` - and a **diagonal**
/// one lights a single quadrant twice, because it is the corner shared by two quadrants. That asymmetry is the whole mechanism: it is how a wall with a lamp on one side
/// and darkness on the other shows only the lit side.
const LIT_QUADRANTS: [[Quadrant; 2]; 8] = [
    // south
    [Quadrant::SouthEast, Quadrant::SouthWest],
    // north
    [Quadrant::NorthEast, Quadrant::NorthWest],
    // east
    [Quadrant::SouthEast, Quadrant::NorthEast],
    // south-east
    [Quadrant::SouthEast, Quadrant::SouthEast],
    // north-east
    [Quadrant::NorthEast, Quadrant::NorthEast],
    // west
    [Quadrant::SouthWest, Quadrant::NorthWest],
    // south-west
    [Quadrant::SouthWest, Quadrant::SouthWest],
    // north-west
    [Quadrant::NorthWest, Quadrant::NorthWest],
];

/// How much light reaches a square, given what its neighbours are like.
///
/// Reference: `map::apparent_light_helper` (`lightmap.cpp:725-799`). Two cases, and only one of them is interesting:
///
/// * a square **light passes through** takes light from every direction equally, so it is `vis` times the brightest quadrant;
/// * a square **light cannot pass through** - a wall - is only lit on the sides the player can see it from, so each neighbour the player can see raises the quadrants it
///   sits in to `vis`, and the answer is the brightest product of those with the square's own light map.
///
/// `vis` is the player's own visibility of the square, and `opaque` is whether the square itself blocks light.
// The three arrays are each exactly eight entries and the loop walks exactly eight quadrant pairs, so the indices cannot leave them; writing this with `iter().zip`
// would obscure the neighbour-by-neighbour reading the reference has.
#[allow(clippy::indexing_slicing)]
#[must_use]
pub fn visible_light(
    opaque: bool,
    vis: f32,
    neighbourhood: &Neighbourhood,
    light: &Quadrants,
) -> f32 {
    if !opaque {
        return vis * light.max();
    }
    let mut seen_from = Quadrants::new([0.0; 4]);
    for (index, quadrants) in LIT_QUADRANTS.iter().enumerate() {
        if !neighbourhood.in_bounds[index] || neighbourhood.opaque[index] {
            continue;
        }
        let neighbour = neighbourhood.neighbours[index];
        let out_of_range =
            neighbour.dist > neighbourhood.unimpaired_range && neighbour.camera == 0.0;
        let unseen = neighbour.seen == 0.0 && neighbour.camera == 0.0;
        if out_of_range || unseen {
            continue;
        }
        for quadrant in quadrants {
            seen_from.set(*quadrant, vis);
        }
    }
    let values = seen_from.values();
    let lit = light.values();
    values
        .iter()
        .zip(lit.iter())
        .map(|(seen, light)| seen * light)
        .fold(f32::NEG_INFINITY, f32::max)
}

/// How much of a light survives a distance through a given transparency.
///
/// Reference: `sight_calc` (`src/shadowcasting.h:98-101`). The Beer-Lambert law, `1 / e^( a*l )`, with the length factored out and `a` the mean transparency along the
/// way: bright light falling on open air at `MAX_VIEW_DISTANCE` is exactly at the lit threshold, which is not an accident - it is how `TRANSPARENCY_OPEN_AIR` is chosen.
///
/// Note what a transparency of **zero** does here: nothing. Blocking is `sight_check`'s job, because a ray never enters an opaque square in the first place.
#[must_use]
pub fn sight_calc(numerator: f32, transparency: f32, distance: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let distance = distance as f32;
    numerator / (transparency * distance).exp()
}

/// Whether a ray may pass into a square of this transparency.
///
/// Reference: `sight_check` (`src/shadowcasting.h:102-105`). Strictly above zero, so a square with no transparency at all stops the ray and a square with any at all
/// does not - the reference's transparencies are all either zero or `2.3 / 72` upwards.
#[must_use]
pub fn sight_check(transparency: f32) -> bool {
    transparency > TRANSPARENCY_SOLID
}

/// Folds one more square's transparency into a ray's running average.
///
/// Reference: `accumulate_transparency` (`src/shadowcasting.h:112-116`): `(( distance - 1 ) * cumulative + current ) / distance`. At each distance this is the exact
/// arithmetic mean of everything the ray has passed through so far, which makes the result **independent of the order** the squares are met in - a property the light
/// cast relies on and a test pins.
#[must_use]
pub fn accumulate_transparency(cumulative: f32, current: f32, distance: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let distance = distance as f32;
    ((distance - 1.0) * cumulative + current) / distance
}

/// Keeps the brighter of two light values.
///
/// Reference: `update_light` (`src/shadowcasting.h:105-108`), which is a maximum written as an `if` for a compiler reason the reference explains; a port can call the
/// maximum directly.
#[must_use]
pub fn update_light(existing: f32, candidate: f32) -> f32 {
    existing.max(candidate)
}
