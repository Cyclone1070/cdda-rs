//! The weather generator: the numbers behind a weather type.
//!
//! Reference: `weather_gen.cpp`. Everything the weather system needs to decide *what* the sky is doing comes from two noise fields and a cosine: a
//! temperature that swings with the hour, the season and a four-dimensional noise field, a humidity and a pressure that follow the same field at
//! different seeds, and a wind that follows the pressure.
//!
//! What is ported here is the deterministic half - the temperature - because it is the half that can be checked exactly. The wind needs dice
//! (`rng(1, 2)`, `rng(9, base_wind_distrib_peaks)`, `one_in(W * 2160)`), and this port's rule is that dice belong to the caller, so the wind arrives
//! with the rest of the turn machinery rather than being faked here.

/// The hour of the day, out of twenty-four, at which the day is coldest (`weather_gen.cpp:30`).
pub const COLDEST_HOUR: f64 = 5.0;

/// How far the daily swing reaches from the day's average, in kelvins (`weather_gen.cpp:32`).
pub const DAILY_MAGNITUDE_K: f64 = 5.0;

/// How far the seasonal swing reaches from the year's average, in kelvins (`weather_gen.cpp:34`).
pub const SEASONALITY_MAGNITUDE_K: f64 = 15.0;

/// How far the day-to-day noise reaches, in kelvins (`weather_gen.cpp:36`).
pub const NOISE_MAGNITUDE_K: f64 = 8.0;

/// The constants the day's and year's swings are measured against.
const TAU: f64 = std::f64::consts::TAU;

/// The inputs a generator shares across every derived number (`weather_gen.cpp`, `weather_gen_common`).
///
/// The reference builds these from the player's absolute position, the turn and the world seed. They are passed in already built because that
/// conversion is calendar work - a separate piece with its own rules and its own gate - and the formula underneath should be checkable without it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommonData {
    /// The absolute x position, divided by the noise's widening factor of 2000.
    pub x: f64,
    /// The absolute y position, divided by the same factor.
    pub y: f64,
    /// The turn, in days since the calendar's zero.
    pub z: f64,
    /// The year's cosine: `+1` at midwinter, `-1` at midsummer.
    pub cyf: f64,
    /// The seed the noise fields are shifted by, already reduced to the noise limit.
    pub mod_seed: u32,
    /// The season, as the reference's enumerator: spring 0, summer 1, autumn 2, winter 3.
    pub season: usize,
}

/// The year's cosine, `+1` at midwinter and `-1` at midsummer (`weather_gen.cpp:75-79`).
///
/// The eighth is what lines the swing up with a Cataclysm year, which begins in spring rather than at midwinter.
#[must_use]
pub fn year_cosine(year_fraction: f64) -> f64 {
    (TAU * (year_fraction + 0.125)).cos()
}

/// The day's cosine, `-1` at the coldest hour and `+1` twelve hours later (`weather_gen.cpp:93-95`).
#[must_use]
pub fn day_cosine(day_fraction: f64) -> f64 {
    (TAU * (day_fraction + 0.5 - COLDEST_HOUR / 24.0)).cos()
}

/// What a region sets: its average temperature and its seasonal adjustments (`weather_gen.h`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Generator {
    /// The year's average temperature, in Celsius.
    pub base_temperature: f64,
    /// The region's manual adjustment per season, in kelvins, in the reference's season order.
    pub seasonal_temp_mod: [f64; 4],
}

/// The temperature at a moment, in Celsius (`weather_gen.cpp:99-108`).
#[must_use]
pub fn temperature(generator: &Generator, common: &CommonData, day_fraction: f64) -> f64 {
    let seasonality = -common.cyf;
    let dayv = day_cosine(day_fraction);
    let seasonal = generator
        .seasonal_temp_mod
        .get(common.season)
        .copied()
        .unwrap_or(0.0);
    let baseline = generator.base_temperature
        + seasonal
        + dayv * DAILY_MAGNITUDE_K
        + seasonality * SEASONALITY_MAGNITUDE_K;
    let noise = crate::noise::raw_noise_4d(
        common.x as f32,
        common.y as f32,
        common.z as f32,
        common.mod_seed as f32,
    );
    baseline + f64::from(noise) * NOISE_MAGNITUDE_K
}

/// The temperature at a moment, in kelvins - which is what the oracle records.
///
/// **The reference's temperature is a `float`**, not a double: `units::temperature` is `quantity<float, ...>` (`units_fwd.h:39`), so the double the
/// formula produces is rounded to single precision on the way in and widened again on the way out. Doing the same here is not pedantry - at 275 K the
/// spacing between `float`s is 0.00003 K, which is larger than every arithmetic difference the port could otherwise have, and skipping the round trip
/// leaves a residual that looks like a formula error when it is only a storage width.
#[must_use]
pub fn temperature_k(generator: &Generator, common: &CommonData, day_fraction: f64) -> f64 {
    // `from_celsius`: `v + 273.150f` where `v` is a double and the constant is a float, so the addition happens in double and only the sum is
    // narrowed. Narrowing first and adding in single precision is off by one ulp on some hours, which is a good reminder that "then convert to float"
    // is a recipe with an order in it.
    let celsius = temperature(generator, common, day_fraction);
    f64::from((celsius + f64::from(273.150f32)) as f32)
}
