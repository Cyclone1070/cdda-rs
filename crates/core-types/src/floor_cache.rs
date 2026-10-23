//! Where a level's floor is missing.
//!
//! Reference: `map::build_floor_cache( zlev )` (`src/map.cpp:9417-9471`). The cache is one bit per square, and the sunlight pass reads the bit one level up: a square receives
//! daylight only when the level above has no floor there. That is what makes a stairwell a column of light and a solid roof a lid.
//!
//! Four terrain flags open the floor - open air, water deep enough to fall into, a stair going down, and a see-through floor - and one happens to close it again: a
//! greenhouse's `SUN_ROOF_ABOVE` furniture on the level **below**, which catches the sun on its own level instead of letting the square above pass it down.

/// The terrain flags that leave a square without a floor.
///
/// Reference: `TFLAG_NO_FLOOR`, `TFLAG_NO_FLOOR_WATER`, `TFLAG_GOES_DOWN` and `TFLAG_TRANSPARENT_FLOOR` (`map.cpp:9453-9456`). Listed by name because that is how the
/// reference names them, and because each one is a different reason to fall: nothing there, water there, a stair there, or glass there.
pub const NO_FLOOR_FLAGS: [&str; 4] = [
    "NO_FLOOR",
    "NO_FLOOR_WATER",
    "GOES_DOWN",
    "TRANSPARENT_FLOOR",
];

/// The furniture flag that catches the sun where a floor is missing.
///
/// Reference: `TFLAG_SUN_ROOF_ABOVE` (`map.cpp:9458`), read from the level **below** the one being built.
pub const SUN_ROOF_ABOVE: &str = "SUN_ROOF_ABOVE";

/// What the floor cache needs to know about a level.
///
/// Reference: the inputs of `build_floor_cache` (`map.cpp:9431-9461`) - whether each square's terrain carries any of [`NO_FLOOR_FLAGS`], and whether the square one level
/// **down** has furniture carrying [`SUN_ROOF_ABOVE`]. The disjunction over the four flags is the caller's, because they live on the terrain registry.
#[derive(Debug, Clone)]
pub struct FloorFacts {
    /// The side of the level, in squares.
    pub side: usize,
    /// Whether each square's terrain carries any of the four no-floor flags.
    pub no_floor: Vec<bool>,
    /// Whether each square of the level **below** has a sun roof above it.
    pub sun_roof_below: Vec<bool>,
}

/// A level's answer: which squares have a floor, and whether the level has any gaps at all.
///
/// Reference: `floor_cache` together with its summary bit `no_floor_gaps` (`map.cpp:9425-9429`). The summary exists so that a caller can skip the sunlight pass in one test.
#[derive(Debug, Clone)]
pub struct FloorCache {
    /// Whether each square has something underfoot.
    pub cells: Vec<bool>,
    /// Whether every square has a floor, so that no light can pass through this level.
    pub no_floor_gaps: bool,
}

/// Builds a level's floor cache.
///
/// Reference: `map::build_floor_cache` (`map.cpp:9417-9471`): start with a floor everywhere, then clear the squares the terrain opens - unless a sun roof below catches the
/// light first, in which case the square keeps its floor as far as the sun is concerned.
#[must_use]
pub fn floor_of(facts: &FloorFacts) -> FloorCache {
    let side = facts.side;
    let cells = side * side;
    let mut floor = vec![true; cells];
    let mut no_floor_gaps = true;

    for x in 0..side {
        for y in 0..side {
            let index = x * side + y;
            if !facts.no_floor.get(index).copied().unwrap_or(false) {
                continue;
            }
            if facts.sun_roof_below.get(index).copied().unwrap_or(false) {
                // The greenhouse below catches the sun itself, so this square is not a hole.
                continue;
            }
            if let Some(slot) = floor.get_mut(index) {
                *slot = false;
            }
            no_floor_gaps = false;
        }
    }

    FloorCache {
        cells: floor,
        no_floor_gaps,
    }
}
