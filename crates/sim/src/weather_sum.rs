//! Summing a stretch of weather: how coarsely it is sampled, and what each sample adds up to.
//!
//! Reference: `sum_conditions` and `proc_weather_sum` (`src/weather.cpp:152-215`). The reference walks from a start to an end and picks a **step size** from the length of the stretch -
//! a turn at a time for seconds, a minute at a time for hours, an hour at a time for weeks - then adds each step's contribution. The port keeps the two rules and leaves the walk to a
//! caller that has a calendar.

use cdda_content::weather::{Precipitation, WeatherType};

/// The step a weather sum takes, in the game's own units.
///
/// Reference: `sum_conditions` (`weather.cpp:190-205`) assigns `1_turns`, `1_minutes` or `1_hours`. The port names them by what they are so a reader does not have to remember that a
/// "turn" is a second and a "move" is a hundredth of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovesPerTick {
    /// One turn: one second of game time.
    Turn,
    /// One minute.
    Minute,
    /// One hour.
    Hour,
}

impl MovesPerTick {
    /// How many moves one step is.
    ///
    /// The game's move is a hundredth of a turn (`to_moves<int>( 1_turns )`), which is what the reference's `to_turns` and `to_seconds` conversions count in.
    #[must_use]
    pub const fn moves(self) -> i32 {
        match self {
            Self::Turn => 100,
            Self::Minute => 6_000,
            Self::Hour => 360_000,
        }
    }

    /// How many turns one step is, which is what a per-turn accumulation is scaled by.
    #[must_use]
    pub const fn turns(self) -> i32 {
        match self {
            Self::Turn => 1,
            Self::Minute => 60,
            Self::Hour => 3_600,
        }
    }

    /// How many seconds one step is, which is what a per-second accumulation is scaled by.
    ///
    /// **The same number as [`Self::turns`]**: the reference's `to_seconds( tick )` and `to_turns( tick )` agree because a turn is a second. The port keeps both names because the
    /// reference's two accumulations read differently and a reader should be able to follow either without converting in their head.
    #[must_use]
    pub const fn seconds(self) -> i32 {
        self.turns()
    }
}

/// The step a weather sum of this length takes.
///
/// Reference: `sum_conditions` (`weather.cpp:190-205`):
///
/// ```text
/// if( diff < 10_turns )       tick_size = 1_turns;
/// else if( diff > 7_days )    tick_size = 1_hours;
/// else                        tick_size = 1_minutes;
/// ```
///
/// **The comparisons are asymmetric and that is the trap**: *under* ten turns is the fine step and *over* seven days is the coarse one, so exactly ten turns and exactly seven days both
/// take the **middle** step. A port that wrote `<=` in either place would agree on every value but the two boundaries and never notice. A stretch of nothing is under ten turns and takes
/// the fine step, which is the reference's own answer rather than a special case.
#[must_use]
pub fn tick_size(diff_turns: i32) -> MovesPerTick {
    /// Ten turns, below which the sum is a turn at a time.
    const FINE_BELOW: i32 = 10;
    /// Seven days, above which the sum is an hour at a time.
    const COARSE_ABOVE: i32 = 7 * 24 * 60 * 60;
    if diff_turns < FINE_BELOW {
        MovesPerTick::Turn
    } else if diff_turns > COARSE_ABOVE {
        MovesPerTick::Hour
    } else {
        MovesPerTick::Minute
    }
}

/// What one step of a weather adds to a running sum.
///
/// Reference: `weather_sum` (`weather.h`), whose three numbers are accumulated separately because three different rules read them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Step {
    /// Rain that fell, in the reference's own units: a turn of very light rain is one.
    pub rain: i32,
    /// Light over the step: [`crate::creature::incident_sunlight`] times the step's turns.
    pub sunlight: f32,
    /// Radiant exposure over the step: [`crate::creature::incident_sun_irradiance`] times the step's **seconds**.
    pub radiant_exposure: f32,
}

/// How much rain a precipitation class lays down in one turn.
///
/// Reference: `proc_weather_sum` (`weather.cpp:152-178`): very light is **1**, light **4**, heavy **8**, and nothing else falls at all. **The amount is per turn**, so a minute of
/// light rain is sixty times one turn's four. A weather that does not `rains` adds nothing whatever its precipitation class says - which is how snow is kept out of the rain total.
#[must_use]
pub fn rain_per_turn(precipitation: Precipitation, rains: bool) -> i32 {
    if !rains {
        return 0;
    }
    match precipitation {
        Precipitation::None => 0,
        Precipitation::VeryLight => 1,
        Precipitation::Light => 4,
        Precipitation::Heavy => 8,
    }
}

/// What one step of this weather adds to the sum.
///
/// Reference: `proc_weather_sum` (`weather.cpp:174-180`), which adds `incident_sunlight( w, t ) * to_turns( tick )` to the light and `incident_sun_irradiance( w, t ) * to_seconds( tick )` to
/// the exposure. The clear-sky values are handed in because they come from the calendar, which this rule has no business reading.
#[must_use]
pub fn step_of(
    weather: &WeatherType,
    step: MovesPerTick,
    clear_sky_light: f32,
    clear_sky_irradiance: f32,
) -> Step {
    #[allow(clippy::cast_precision_loss)]
    let turns = step.turns() as f32;
    #[allow(clippy::cast_precision_loss)]
    let seconds = step.seconds() as f32;
    Step {
        rain: rain_per_turn(weather.precip, weather.rains) * step.turns(),
        sunlight: crate::creature::incident_sunlight(
            clear_sky_light,
            weather.light_multiplier,
            weather.light_modifier,
        ) * turns,
        radiant_exposure: crate::creature::incident_sun_irradiance(
            clear_sky_irradiance,
            weather.sun_multiplier,
        ) * seconds,
    }
}
