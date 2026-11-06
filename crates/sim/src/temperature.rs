//! How warm a square is.
//!
//! Reference: `weather_manager::get_temperature` (`src/weather.cpp:956-982`), which is four lines and a cache. A square **below street level** answers the weather
//! generator's `base_temperature` and nothing else - the ground is at the year's average temperature, not the day's. A square above it answers the weather's current
//! temperature plus three terms, and the three are different kinds of data:
//!
//! - [`heat_radiation`]: `get_heat_radiation` (`src/game.cpp:1755-1795`), which looks six squares around for fire and for terrain that radiates, and warms the square by
//!   `6 F * intensity^2 / distance` for each one it can see. This is why a fire warms the room and not just itself, and why lava warms the floor next to it.
//! - [`convection`]: `get_convection_temperature` (`src/game.cpp:1835-1855`), which sums the fields **on** the square. A fire adds 300 F, so standing in one is a
//!   different thing from standing near one.
//! - the submap's own `temperature_mod` (`src/map.cpp:4936-4949`), a number that is almost always zero and that the data can set per submap.
//!
//! **The order of the additions is part of the answer.** The reference builds a `temperature_delta` in single precision, adds the three terms into *it*, and only then
//! adds the total to the weather's temperature (`src/weather.cpp:971-977`); adding them to the temperature one at a time rounds differently, and a gate that compared
//! only "roughly the right warmth" would never see it.
//!
//! **Not ported, declared:** the per-square cache (`temperature_cache`, `:963-980`) - this is the rule, not the store; the trap flag `CONVECTS_TEMPERATURE`
//! (`:1838-1843`), which needs traps on the map rather than in the data; and the creature's own warmth, which needs creatures.

/// How far a heat source reaches, in squares: `points_in_radius( location, 6 )` (`src/game.cpp:1763`).
pub const HEAT_RADIUS: i32 = 6;

/// Degrees Fahrenheit in a kelvin delta: `units::from_fahrenheit_delta` divides by `1.8f` (`src/units.h:466-470`).
pub const FAHRENHEIT_PER_KELVIN: f32 = 1.8;

/// The world's heat sources, as `get_heat_radiation` asks about them.
pub trait Heat {
    /// The intensity of a fire field on the square, zero when there is none (`maptile_field_intensity( mt, fd_fire_int )`, `src/game.cpp:1767`).
    fn fire_intensity(&self, at: (i32, i32)) -> i32;
    /// The square's terrain heat radiation, `mapdata.h:537` (`mt.get_ter()->heat_radiation`, `src/game.cpp:1769`).
    fn terrain_heat(&self, at: (i32, i32)) -> i32;
    /// Whether one square can see another: `map::sees` (`src/map.cpp`), which is a line and a transparency test.
    fn sees(&self, from: (i32, i32), to: (i32, i32)) -> bool;
}

/// What a square's surroundings radiate onto it, as a kelvin delta (`get_heat_radiation`, `src/game.cpp:1755-1795`).
///
/// The box is **square**, not round: `points_in_radius` is a rectangle clipped to the map (`src/map.cpp:10263-10276`), iterated y outer and x inner
/// (`src/map_iterator.h:44-64`), which is the order the float sum is accumulated in.
#[must_use]
pub fn heat_radiation(heat: &impl Heat, at: (i32, i32)) -> f32 {
    let mut total = 0.0f32;
    for dy in -HEAT_RADIUS..=HEAT_RADIUS {
        for dx in -HEAT_RADIUS..=HEAT_RADIUS {
            let source = (at.0 + dx, at.1 + dy);
            let fire = heat.fire_intensity(source);
            let intensity = if fire > 0 {
                fire
            } else {
                heat.terrain_heat(source)
            };
            if intensity == 0 {
                continue;
            }
            if !heat.sees(at, source) {
                continue;
            }
            // `std::max( 1, square_dist( dest, location ) )`: square distance is Chebyshev, and the max guards the division rather than the caller.
            #[allow(clippy::cast_precision_loss)]
            let distance = dx.abs().max(dy.abs()).max(1) as f32;
            #[allow(clippy::cast_precision_loss)]
            let fahrenheit = 6.0f32 * (intensity * intensity) as f32 / distance;
            total += fahrenheit / FAHRENHEIT_PER_KELVIN;
        }
    }
    total
}

/// What the fields **on** a square do to it, as a kelvin delta (`get_convection_temperature`, `src/game.cpp:1835-1855`).
///
/// Every field on the square adds its level's `convection`; a field that is itself fire first **cancels** the lava term, because a fire on the square already explains the
/// heat. The lava term needs traps on the map and is not ported, so what is left is the sum.
#[must_use]
pub fn convection(fields: &[(i32, bool)]) -> f32 {
    let mut total = 0.0f32;
    for (fahrenheit, _is_fire) in fields {
        #[allow(clippy::cast_precision_loss)]
        let term = *fahrenheit as f32;
        total += term / FAHRENHEIT_PER_KELVIN;
    }
    total
}

/// The temperature of a square, in kelvin (`weather_manager::get_temperature`, `src/weather.cpp:956-982`).
///
/// `weather` is the weather's current temperature, `base` the generator's year-round `base_temperature`, and `map` the submap's own modifier. The three terms are summed
/// **into the delta first** and then onto the temperature, which is the reference's order and therefore the reference's rounding.
#[must_use]
pub fn temperature(weather: f32, base: f32, z: i32, heat: f32, convection: f32, map: f32) -> f32 {
    if z < 0 {
        return base;
    }
    let mut delta = heat;
    delta += convection;
    delta += map;
    weather + delta
}

/// Whether a square is warm enough to plant in.
///
/// Reference: `warm_enough_to_plant` (`weather.cpp:875-883`):
///
/// ```text
/// return get_weather().get_temperature( pos ) >= units::from_fahrenheit( 50 );
/// ```
///
/// **The comparison is `>=`**, so a square at exactly fifty is warm enough. The threshold is the reference's own **semi-appropriate temperature for most plants** (its comment), stated in
/// Fahrenheit; the port holds kelvin, so the door is converted once here rather than at every caller: `( 50 - 32 ) / 1.8 + 273.15`.
#[must_use]
pub fn warm_enough_to_plant(temperature_kelvin: f32) -> bool {
    /// Fifty Fahrenheit in kelvin: `units::from_fahrenheit( 50 )` (`weather.cpp:877`).
    const FIFTY_FAHRENHEIT: f32 = (50.0 - 32.0) / 1.8 + 273.15;
    temperature_kelvin >= FIFTY_FAHRENHEIT
}
