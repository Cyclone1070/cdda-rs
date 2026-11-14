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
    /// The year's average relative humidity, as a percentage (`weather_gen.h`).
    pub base_humidity: f64,
    /// The year's average pressure, in millibars.
    pub base_pressure: f64,
    /// The region's manual adjustment to humidity per season, in percentage points, in the same order.
    pub humidity_manual_mod: [f64; 4],
    /// The year's average wind, which the pressure is divided into (`weather_gen.h`).
    pub base_wind: f64,
    /// The pressure exponent's upper bound when the wind is being rolled (`weather_gen.h`).
    pub base_wind_distrib_peaks: i32,
    /// How much the season moves the wind, as a divisor (`weather_gen.h`).
    pub base_wind_season_variation: f64,
}

impl Generator {
    /// The default region's settings, from `data/json/regional_map_settings.json`: 6.5 C, 70 per cent humidity, 1015 millibars, 3.4 wind, exponent
    /// peaks at 80, and a seasonal wind variation of 50. No season carries a manual adjustment.
    #[must_use]
    pub fn default_region() -> Self {
        Generator {
            base_temperature: 6.5,
            seasonal_temp_mod: [0.0; 4],
            base_humidity: 70.0,
            base_pressure: 1015.0,
            humidity_manual_mod: [0.0; 4],
            base_wind: 3.4,
            base_wind_distrib_peaks: 80,
            base_wind_season_variation: 50.0,
        }
    }
}

/// The terms the temperature is built from, so a port can be held to the arithmetic and not only to the rounding.
///
/// Reference: `weather_gen.cpp:90-108`, and the oracle's own `harness_weather_temperature_terms`, which exists for exactly this reason - one hour in a fortnight lands
/// within an ulp of a `float` boundary, and from the narrowed value alone there is no way to tell a formula error from a rounding tie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemperatureTerms {
    /// The total before the conversion to kelvin, in Celsius.
    pub celsius: f64,
    /// The day's cosine: `-1` at the coldest hour, `+1` twelve hours later.
    pub day: f64,
    /// `-cyf`: `+1` in midwinter, `-1` in midsummer.
    pub seasonality: f64,
    /// The noise field's value at this place and moment.
    pub noise: f64,
    /// The region's average temperature, in Celsius.
    pub base: f64,
    /// How far through the day the moment is.
    pub day_fraction: f64,
}

/// The temperature at a moment, in Celsius (`weather_gen.cpp:99-108`), with every term that produced it.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
#[must_use]
pub fn temperature_terms(
    generator: &Generator,
    common: &CommonData,
    day_fraction: f64,
) -> TemperatureTerms {
    let seasonality = -common.cyf;
    let day = day_cosine(day_fraction);
    let seasonal = generator
        .seasonal_temp_mod
        .get(common.season)
        .copied()
        .unwrap_or(0.0);
    // The association is the reference's, term by term: `( ( base + seasonal ) + day * daily ) + seasonality * seasonal_amplitude`, then the noise.
    let baseline = generator.base_temperature
        + seasonal
        + day * DAILY_MAGNITUDE_K
        + seasonality * SEASONALITY_MAGNITUDE_K;
    let noise = f64::from(crate::noise::raw_noise_4d(
        common.x as f32,
        common.y as f32,
        common.z as f32,
        common.mod_seed as f32,
    ));
    TemperatureTerms {
        celsius: baseline + noise * NOISE_MAGNITUDE_K,
        day,
        seasonality,
        noise,
        base: generator.base_temperature,
        day_fraction,
    }
}

/// The temperature at a moment, in Celsius (`weather_gen.cpp:99-108`).
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
#[must_use]
pub fn temperature(generator: &Generator, common: &CommonData, day_fraction: f64) -> f64 {
    temperature_terms(generator, common, day_fraction).celsius
}

/// The temperature at a moment, in kelvins - which is what the oracle records.
///
/// **The reference's temperature is a `float`**, not a double: `units::temperature` is `quantity<float, ...>` (`units_fwd.h:39`), so the double the
/// formula produces is rounded to single precision on the way in and widened again on the way out. Doing the same here is not pedantry - at 275 K the
/// spacing between `float`s is 0.00003 K, which is larger than every arithmetic difference the port could otherwise have, and skipping the round trip
/// leaves a residual that looks like a formula error when it is only a storage width.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn temperature_k(generator: &Generator, common: &CommonData, day_fraction: f64) -> f64 {
    // `from_celsius`: `v + 273.150f` where `v` is a double and the constant is a float, so the addition happens in double and only the sum is
    // narrowed. Narrowing first and adding in single precision is off by one ulp on some hours, which is a good reminder that "then convert to float"
    // is a recipe with an order in it.
    let celsius = temperature(generator, common, day_fraction);
    f64::from((celsius + f64::from(273.150f32)) as f32)
}

/// The relative humidity at a moment, as a percentage (`weather_gen.cpp:141-152`).
///
/// Reference: `std::min( 100., std::max( 0., base + mod + 100 * ( .15 * seasonality + noise * .2 * ( -seasonality + 2 ) ) ) )`. The noise is the same
/// field the temperature uses, two hundred and one seeds away, so the two move together - which is what makes a cold snap also a wet one.
#[must_use]
pub fn humidity(generator: &Generator, common: &CommonData) -> f64 {
    let seasonality = -common.cyf;
    let manual = generator
        .humidity_manual_mod
        .get(common.season)
        .copied()
        .unwrap_or(0.0);
    let noise = f64::from(noise_at(common, 101));
    let unclamped = generator.base_humidity
        + manual
        + 100.0 * (0.15 * seasonality + noise * 0.2 * (-seasonality + 2.0));
    unclamped.clamp(0.0, 100.0)
}

/// The atmospheric pressure at a moment, in millibars (`weather_gen.cpp:154-158`).
///
/// **`noise * 15` is a single-precision product in the reference.** `raw_noise_4d` returns `float` and the literal is an `int`, so C++ promotes the literal to `float` and
/// multiplies in `float` before widening to `double` for the rest of the expression. Widening first and multiplying in double leaves a residual of up to a millionth of a
/// millibar - which is small, and is exactly the size a parity gate is meant to see.
#[must_use]
pub fn pressure(generator: &Generator, common: &CommonData) -> f64 {
    let seasonality = -common.cyf;
    let scaled = f64::from(noise_at(common, 211) * 15.0);
    generator.base_pressure + scaled * (-seasonality + 2.0)
}

/// The noise field at a given seed offset from the world's.
///
/// The offset is added to an **unsigned** seed in the reference (`modSEED + 101`), so it wraps rather than saturating: a world seed one below the noise
/// limit would otherwise leave the field entirely.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn noise_at(common: &CommonData, offset: u32) -> f32 {
    crate::noise::raw_noise_4d(
        common.x as f32,
        common.y as f32,
        common.z as f32,
        common.mod_seed.wrapping_add(offset) as f32,
    )
}

/// The noise field at one of the reference's seed offsets, widened for comparison.
///
/// The humidity and the pressure use the same field at two other seeds; exposing it lets a gate separate "the noise is right" from "the arithmetic around it is right".
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn noise_for(common: &CommonData, offset: u32) -> f64 {
    f64::from(noise_at(common, offset))
}

/// The noise the wind starts from, before the pressure divides it (`weather_gen.cpp:134`).
///
/// Reference: `raw_noise_4d( x / 2.5, y / 2.5, z / 200, modSEED ) * 10.0` - the same field again, widened on x and y and narrowed on z, so the wind
/// varies over tens of map squares and over years rather than hours.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
#[must_use]
pub fn wind_noise(common: &CommonData) -> f64 {
    f64::from(crate::noise::raw_noise_4d(
        (common.x / 2.5) as f32,
        (common.y / 2.5) as f32,
        (common.z / 200.0) as f32,
        common.mod_seed as f32,
    )) * 10.0
}

/// The wind's strength at a moment (`weather_gen.cpp:160-164`).
///
/// Reference: `max( 0, int( base_wind * rng(1,2) / pow( (P + W) / 1014.78, rng(9, peaks) ) + -cyf / variation * rng(1,2) ) )`.
///
/// The dice come from the caller, in the reference's order - multiplier, exponent, seasonal multiplier - because that is the order the trace shows and a
/// port that draws its own would be comparing luck. The three are asked for through `roll`, which returns what it was given.
///
/// # Panics
///
/// Panics only if `roll` returns a value outside the range it was asked for; the reference's dice cannot, and a caller that supplies its own must not
/// either.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn wind_power(
    generator: &Generator,
    common: &CommonData,
    pressure: f64,
    roll: &mut dyn FnMut(i32, i32) -> i32,
) -> i32 {
    let multiplier = roll(1, 2);
    let exponent = roll(9, generator.base_wind_distrib_peaks);
    let seasonal = roll(1, 2);
    let noise = wind_noise(common);
    let numerator = generator.base_wind * f64::from(multiplier);
    let ratio = ((pressure + noise) / 1014.78).powf(f64::from(exponent));
    let seasonal_term = -common.cyf / generator.base_wind_season_variation * f64::from(seasonal);
    let wind = numerator / ratio + seasonal_term;
    // `static_cast<int>` truncates toward zero, and the maximum with zero is applied after the cast, not before.
    (wind as i32).max(0)
}

/// The sixteen wind sectors' weights, one table per season (`weather_gen.cpp:226-246`).
///
/// The reference builds a `std::discrete_distribution` from one of these per season, so the weights are a probability table and their sum is whatever it
/// is - the distribution normalises. They are exposed rather than hidden inside a picker because the pick needs a uniform draw, and draws belong to the
/// caller.
pub const WIND_DIRECTION_WEIGHTS: [[i32; 16]; 4] = [
    // Spring.
    [3, 3, 5, 8, 11, 10, 5, 2, 5, 6, 6, 5, 8, 10, 8, 6],
    // Summer.
    [3, 4, 4, 8, 8, 9, 8, 3, 7, 8, 10, 7, 7, 7, 5, 3],
    // Autumn.
    [4, 6, 6, 7, 6, 5, 4, 3, 5, 6, 8, 8, 10, 10, 8, 5],
    // Winter.
    [5, 3, 2, 3, 2, 2, 2, 2, 4, 6, 10, 8, 12, 19, 13, 9],
];

/// One of the sixteen sectors as an angle in degrees (`weather_gen.cpp:250-255`).
///
/// Reference: `static_cast<int>( inputdir * 22.5f )` - the multiply is single precision and the cast truncates.
#[must_use]
pub fn wind_direction_angle(index: usize) -> i32 {
    // `22.5f` is exact in binary, so `index as f32 * 22.5f` is exact for every index below sixteen and the truncation is the only rounding.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let angle = (index as f32 * 22.5f32) as i32;
    angle
}

/// The wind's direction, as the reference keeps it: a value that starts unset and is redrawn on a schedule.
///
/// The reference holds this in a `static int current_winddir = 1000`, which is process state that survives every call and every world (`weather_gen.cpp:47`). Making it a value the
/// caller owns is the same rule this port applies to dice: a system that remembers something between turns should say so in its type, not in a file-scope variable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindDirection {
    /// The current direction in degrees, or `None` while the reference still holds its `1000` sentinel.
    pub degrees: Option<i32>,
}

impl WindDirection {
    /// Whether this update draws a new direction.
    ///
    /// Two reasons: nothing has been drawn yet, or the wind has changed enough to draw again. The first is the sentinel, and it happens exactly once per world.
    #[must_use]
    pub fn needs_redraw(&self, wind_power: i32, check: Option<i32>) -> bool {
        self.degrees.is_none() || needs_new_direction(wind_power, check)
    }

    /// Takes a new direction from a sector of the seasonal distribution.
    pub fn set_from_sector(&mut self, sector: usize) {
        self.degrees = Some(wind_direction_angle(sector));
    }
}

/// Whether the wind's direction is redrawn this update (`weather_gen.cpp:165-178`).
///
/// The reference keeps the direction in a `static` that starts at a sentinel, draws it the first time, and afterwards redraws only when
/// `one_in( W * 2160 )` fires.
///
/// `one_in` returns true **without rolling** whenever its chance is one or less (`rng.cpp`), and since the wind is never negative, `W * 2160 <= 1` means
/// the air is dead calm. That is not a detail: across a recorded day the only hour whose direction changed is the hour whose wind was zero, and the
/// trace shows that hour rolling no check at all.
#[must_use]
pub fn needs_new_direction(wind_power: i32, check: Option<i32>) -> bool {
    let chance = wind_power.saturating_mul(2160);
    if chance <= 1 {
        // `one_in( <= 1 )` is true by definition, with no draw to consult.
        return true;
    }
    // A calm hour rolls nothing, so a missing check where the chance is above one would be a recording that does not belong to this rule.
    check.is_some_and(|drawn| drawn == 0)
}
