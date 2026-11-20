//! **Where cities stand.**
//!
//! Reference: `overmap::place_cities` (`overmap.cpp:6037-6162`), the root of the overmap's own generation. Two facts decide the overmap's character before anything else is
//! drawn: how many cities it holds, and how big each one is. Both are dice, and both are rules - which is why this is a module of its own rather than a loop inside a
//! generator that has not been written yet.
//!
//! **What is ported** is the count, the size and the centre: `roll_remainder` over the coverage formula, the size ladder (tiny, small, large, huge), the clip to
//! `[2, 55]`, the centre drawn inside the map, the refusal to build on a tile that is already something, and the intersection the centre becomes.
//!
//! **What is not** is `build_city_street` (`:6148-6154`) - the street grid and the buildings it hangs off it - and `city::merge`, the rule that two cities too close become
//! one. Both are named here rather than half-done: a city without streets is a place, not a town, and the streets are the next layer down.

use bevy_ecs::prelude::*;

/// The options a city placement reads, from the world's own settings (`CITY_SIZE`, `CITY_SPACING`, `OVERMAP_MAXIMUM_URBANITY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CitySettings {
    /// How big a city is before the urban/rural gradient and the dice adjust it.
    pub city_size: i32,
    /// How much of the overmap is covered by cities, as a power of two.
    pub city_spacing: i32,
    /// How far the urban/rural gradient may grow a city, as a multiple of `city_size`.
    pub maximum_urbanity: i32,
}

/// A city the overmap placed: where its centre stands and how far it reaches, in overmap tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedCity {
    /// The centre tile, the overmap's own coordinates.
    pub centre: (i32, i32),
    /// The city's radius in overmap tiles, as the reference's `city::size`.
    pub size: i32,
}

/// How many times the reference retries a placement before giving up (`overmap.cpp:6126`).
const MAX_PLACEMENT_ATTEMPTS: i32 = 50;

/// The tile a city centre becomes, which is what "a city starts with an intersection" means (`overmap.cpp:6137`).
const CITY_CENTRE_TERRAIN: &str = "road_nesw";

/// The side of an overmap, in overmap tiles (`map_scale_constants.h:37`).
pub const OVERMAP_SIDE: i32 = 180;

/// Places this overmap's cities, answering what was placed.
///
/// Reference: `overmap::place_cities` (`overmap.cpp:6037-6162`). `urbanity` and `forestosity` are the overmap's place on the world's urban/rural gradient, computed by
/// [`urbanity`](crate::city_placement::urbanity_of_position) and the forest rule respectively; a caller that has neither stands at the origin, where both are zero.
///
/// The dice are the store's own, so a seed places the same cities: the count is one draw, and every attempt after it draws a size and a centre.
pub fn place_cities(
    world: &mut World,
    _at: (i32, i32),
    settings: CitySettings,
    urbanity: i32,
    forestosity: f64,
) -> Vec<PlacedCity> {
    let mut placed = Vec::new();
    if settings.city_size <= 0 {
        return placed;
    }
    // Reference: `overmap.cpp:6046-6060`. The gradient moves the size ceiling and the spacing, and the ceiling never falls below the base size - "funny things happen if
    // max_city_size is less than op_city_size", says the reference, and it clamps rather than finding out.
    #[allow(clippy::cast_possible_truncation)]
    let forest_half = (forestosity / 2.0) as i32;
    let city_size_adjust = std::cmp::min(urbanity - forest_half, -settings.city_size + 2);
    let mut city_space_adjust = urbanity / 2;
    let mut max_city_size = std::cmp::min(
        settings.city_size + city_size_adjust,
        settings.city_size * settings.maximum_urbanity,
    );
    if max_city_size < settings.city_size {
        max_city_size = settings.city_size;
    }
    let mut spacing = settings.city_spacing;
    if spacing > 0 {
        city_space_adjust = std::cmp::min(city_space_adjust, spacing - 2);
        #[allow(clippy::cast_possible_truncation)]
        let forest = forestosity as i32;
        spacing = spacing - city_space_adjust + forest;
    }
    spacing = std::cmp::min(spacing, 10);
    // **How many cities**: the reference's coverage table in one line (`overmap.cpp:6094-6099`). The `3 / 4` is the reference's own estimate of how much of a city's bounding
    // square is city.
    #[allow(clippy::cast_precision_loss)]
    let omts_per_overmap = f64::from(OVERMAP_SIDE * OVERMAP_SIDE);
    #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
    let city_map_coverage_ratio = 1.0 / 2.0f64.powi(spacing);
    #[allow(clippy::cast_precision_loss)]
    let omts_per_city =
        f64::from((settings.city_size * 2 + 1) * (max_city_size * 2 + 1)) * 3.0 / 4.0;
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return placed;
    };
    let wanted = rolls.roll_remainder(omts_per_overmap * city_map_coverage_ratio / omts_per_city);
    let mut attempts = 0;
    while i32::try_from(placed.len()).unwrap_or(0) < wanted && attempts < MAX_PLACEMENT_ATTEMPTS {
        attempts += 1;
        // The size ladder (`overmap.cpp:6104-6119`): a size drawn between the base and the ceiling, then adjusted by one of four fates - a third tiny, a third small, and the
        // rest split between large and huge, each by its own coin.
        let mut size = rolls.rng(settings.city_size - 1, max_city_size);
        // **Four fates, in the reference's order** (`overmap.cpp:6106-6115`): a third tiny, a third small, then the rest split between large and huge by one more coin. The
        // two `one_in( 2 )` calls are the same call twice on purpose - the second is a fresh draw, not the first answered again - so the lint that sees them alike is answered
        // by the comment and by the order of the draws, which a replay would notice if it changed.
        // The lint that sees two `one_in( 2 )` branches as alike is answered by the draws: the second is a **fresh** coin, which a replay would notice.
        let tiny = rolls.one_in(3);
        let small = if tiny { false } else { rolls.one_in(2) };
        let large = if tiny || small {
            false
        } else {
            rolls.one_in(2)
        };
        if tiny {
            size /= 3;
        } else if small {
            size = size * 2 / 3;
        } else if large {
            size = size * 3 / 2;
        } else {
            size *= 2;
        }
        size = size.clamp(2, 55);
        let centre = (
            rolls.rng(size - 1, OVERMAP_SIDE - size),
            rolls.rng(size - 1, OVERMAP_SIDE - size),
        );
        // **Only a tile that is still default may host a centre** (`overmap.cpp:6133-6134`), and an accepted one becomes an intersection (`:6137`).
        if crate::overmap::terrain_at(world, centre).is_some() {
            continue;
        }
        crate::overmap::set_terrain(world, centre, CITY_CENTRE_TERRAIN);
        placed.push(PlacedCity { centre, size });
    }
    crate::singletons::put_rolls(world, rolls);
    placed
}

/// The urbanity of an overmap from its position on the world, by the reference's gradient (`overmap.cpp:5968-6024`).
///
/// The four `OVERMAP_URBAN_INCREASE_*` options say how fast the world becomes a city in each direction; the default data sets only the southern one, to five
/// (`data/core/external_options.json`), so a world is urban to the south and empty at the origin - which is where a new game starts.
#[must_use]
pub fn urbanity_of_position(
    at: (i32, i32),
    north: i32,
    east: i32,
    west: i32,
    south: i32,
    settings: CitySettings,
) -> i32 {
    if settings.city_size <= 0 {
        return 0;
    }
    if north == 0 && east == 0 && west == 0 && south == 0 {
        return 0;
    }
    #[allow(clippy::cast_precision_loss)]
    let mut adjust = 0.0_f64;
    let (x, y) = at;
    #[allow(clippy::cast_precision_loss)]
    let falloff = |along: i32, across: i32| -> f64 {
        let divisor = if along < 0 { across / -2 } else { across / 2 };
        f64::from(divisor.max(1))
    };
    if north != 0 && y < 0 {
        adjust -= f64::from(y) * f64::from(north) / 10.0;
        if x < 0 && west == 0 {
            adjust /= falloff(y, x);
        }
        if x > 0 && east == 0 {
            adjust /= falloff(y, x);
        }
    }
    if east != 0 && x > 0 {
        adjust += f64::from(x) * f64::from(east) / 10.0;
        if y < 0 && north == 0 {
            adjust /= falloff(x, y);
        }
        if y > 0 && south == 0 {
            adjust /= falloff(x, y);
        }
    }
    if west != 0 && x < 0 {
        adjust -= f64::from(x) * f64::from(west) / 10.0;
        if y < 0 && north == 0 {
            adjust /= falloff(x, y);
        }
        if y > 0 && south == 0 {
            adjust /= falloff(x, y);
        }
    }
    if south != 0 && y > 0 {
        adjust += f64::from(y) * f64::from(south) / 10.0;
        if x < 0 && west == 0 {
            adjust /= falloff(y, x);
        }
        if x > 0 && east == 0 {
            adjust /= falloff(y, x);
        }
    }
    #[allow(clippy::cast_possible_truncation)]
    {
        adjust as i32
    }
}
