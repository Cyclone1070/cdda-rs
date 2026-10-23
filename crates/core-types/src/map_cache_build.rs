//! The four caches a level needs, built in the order they depend on each other.
//!
//! Reference: `map::build_map_cache( zlev, skip_lightmap )` (`src/map.cpp:9549-9569`). The loop is three lines and the order in it is load-bearing:
//!
//! 1. `build_outside_cache` - is there sky above this square;
//! 2. `build_transparency_cache` - **reads the outside cache** to charge the weather for the squares that are in it;
//! 3. `build_floor_cache` - underfoot, and its dirty flag feeds the seen cache;
//! 4. `do_vehicle_caching`, **in a separate pass over all levels**, because a vehicle part writes the floor cache of the level *above* itself;
//! 5. `build_vision_transparency_cache` - **copies** the transparency cache, so it has to be last among the three.
//!
//! The reference spells out the reason for the separate vehicle pass in a comment: otherwise "such changes might be overwritten by main cache-building logic". The reason for
//! vision being last is in its own body: it starts as a copy.

use crate::floor_cache::{FloorCache, FloorFacts, floor_of};
use crate::outside_cache::{OutsideFacts, outside_of};
use crate::transparency::{
    SquareFacts, Transparency, VisionFacts, transparency_of, vision_transparency_of,
};

/// One vehicle part's effect on the caches at its square.
///
/// Reference: `vehicle_caching_internal` and `vehicle_caching_internal_above` (`map.cpp:9483-9527`). Grouped because a part writes up to four caches, and splitting them
/// would let a caller apply three of the four.
// Five independent flags: opacity, an open door, a deck, an interior, and a roof overhead. Collapsing them into an enum would lose the combinations that matter - an opaque
// part that is also boardable is a car door with a step, and an openable part that is closed is opaque.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct VehiclePartCaching {
    /// Which square, as `(x, y)`.
    pub at: (usize, usize),
    /// Whether the part is `OPAQUE` and not broken.
    pub opaque: bool,
    /// Whether the part has an openable door that is currently **open**, which cancels the opacity.
    pub openable_open: bool,
    /// Whether the part is `BOARDABLE`, which gives the square a floor.
    pub boardable: bool,
    /// Whether the square is inside the vehicle, which takes it out of the weather.
    pub inside: bool,
    /// Whether the part is `ROOF` or `OPAQUE`, which gives the square **above** a floor.
    pub roof_or_opaque_above: bool,
}

/// Everything a level's caches are built from.
///
/// Reference: the inputs of `build_map_cache`'s five steps for one level, gathered so that the order can be enforced in one place rather than by five callers.
#[derive(Debug, Clone)]
pub struct LevelFacts {
    /// The side of the level, in squares.
    pub side: usize,
    /// The weather's cost to light, applied to every square under open sky.
    pub sight_penalty: f32,
    /// Which squares are marked indoors.
    pub outside: OutsideFacts,
    /// Per-square transparency facts. `outside` on each of these is the caller's copy; the built cache is the authority.
    pub squares: Vec<SquareFacts>,
    /// Which squares' terrain opens the floor.
    pub no_floor: Vec<bool>,
    /// Whether the level **below** has a sun roof under each square.
    pub sun_roof_below: Vec<bool>,
    /// Which squares' terrain is `TRANSLUCENT`, and so blocks sight but not light.
    pub translucent: Vec<bool>,
    /// Each square's coverage, for the crouch rule.
    pub coverage: Vec<i32>,
    /// Where the player is, when they are on this level.
    pub player: Option<(i32, i32)>,
    /// Whether the player is keeping low.
    pub hiding: bool,
    /// The vehicle parts on this level.
    pub vehicles: Vec<VehiclePartCaching>,
}

/// A level's four caches, in the order they were built.
///
/// Reference: the `level_cache` members `outside_cache`, `transparency_cache`, `floor_cache` and `vision_transparency_cache` (`map.cpp:9556-9568`).
#[derive(Debug, Clone)]
pub struct LevelCaches {
    /// Which squares are under open sky.
    pub outside: Vec<bool>,
    /// Per-square transparency, with and without fields.
    pub transparency: Vec<Transparency>,
    /// Which squares have a floor, and whether the level has gaps.
    pub floor: FloorCache,
    /// What blocks sight, which is the transparency cache plus three exceptions.
    pub vision: Vec<f32>,
}

/// Builds a level's four caches in the reference's order.
///
/// `above_floor` is the floor cache of the level **above**, which the vehicle pass writes into - it is passed in rather than returned because that level built its own caches
/// in an earlier or later iteration of the caller's loop, exactly as the reference's two passes are ordered.
#[must_use]
pub fn build_level(zlev: i32, facts: &LevelFacts, above_floor: &mut [bool]) -> LevelCaches {
    let side = facts.side;
    let cells = side * side;

    // 1. The sky, first, because the weather is charged against it.
    let mut outside = outside_of(zlev, &facts.outside);

    // 2. Transparency reads the sky it just computed, not the caller's copy of it.
    let mut transparency = Vec::with_capacity(cells);
    for x in 0..side {
        for y in 0..side {
            let index = x * side + y;
            let mut square = facts.squares.get(index).cloned().unwrap_or(SquareFacts {
                terrain_transparent: true,
                furniture_transparent: true,
                outside: false,
                fields: Vec::new(),
            });
            square.outside = outside.get(index).copied().unwrap_or(false);
            transparency.push(transparency_of(&square, facts.sight_penalty));
        }
    }

    // 3. The floor.
    let floor = floor_of(&FloorFacts {
        side,
        no_floor: facts.no_floor.clone(),
        sun_roof_below: facts.sun_roof_below.clone(),
    });
    let mut floor_cells = floor.cells.clone();

    // 4. Vehicles, which write this level's caches and the floor of the one above.
    for part in &facts.vehicles {
        let (x, y) = part.at;
        let index = x * side + y;
        // An open door is not an obstacle, and a broken part was already reported as not opaque by the caller.
        let opaque = part.opaque && !part.openable_open;
        if opaque && let Some(slot) = transparency.get_mut(index) {
            *slot = Transparency {
                value: 0.0,
                without_fields: 0.0,
            };
        }
        if (opaque || part.inside)
            && let Some(cell) = outside.get_mut(index)
        {
            // The sky map is written by the vehicle pass too, which is one more reason it cannot be part of the per-level loop.
            *cell = false;
        }
        if part.boardable
            && let Some(slot) = floor_cells.get_mut(index)
        {
            *slot = true;
        }
        if part.roof_or_opaque_above
            && let Some(slot) = above_floor.get_mut(index)
        {
            *slot = true;
        }
    }
    // The summary bit is recomputed rather than copied, because a vehicle deck can put a floor back.
    let no_floor_gaps = floor_cells.iter().all(|floor| *floor);

    // 5. Vision last: it starts as a copy of the transparency cache that the vehicle pass has finished writing.
    let vision = vision_transparency_of(&VisionFacts {
        side,
        transparency: transparency.iter().map(|cell| cell.value).collect(),
        translucent: facts.translucent.clone(),
        coverage: facts.coverage.clone(),
        player: facts.player,
        hiding: facts.hiding,
    });

    LevelCaches {
        outside,
        transparency,
        floor: FloorCache {
            cells: floor_cells,
            no_floor_gaps,
        },
        vision,
    }
}
