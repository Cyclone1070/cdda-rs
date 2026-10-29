//! Game time and calendar.
//!
//! Reference: `src/calendar.h`, `src/calendar.cpp` and `tests/calendar_test.cpp`
//! in Cataclysm-DDA at tag `0.I-1` (commit `7b2efa5`).
//!
//! A `TimeDuration` is an integer number of turns, and every conversion to a
//! larger unit truncates toward zero, exactly as the reference's integer
//! division does. Nothing here reads a global option: season length, latitude,
//! longitude and the eternal-season/time-of-day flags are passed in a
//! [`CalendarConfig`].
#![allow(clippy::trivially_copy_pass_by_ref)]
// Config is passed by reference uniformly as it grows.
// The reference stores vector components as float and truncates angles to whole
// turns; these casts are the port, not accidents.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

/// A span of game time, stored as a whole number of turns.
///
/// Reference: `calendar.h:187-348`. One turn is one second of game time.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeDuration {
    turns: i32,
}

impl TimeDuration {
    /// A duration of zero turns.
    pub const ZERO: Self = Self { turns: 0 };

    /// Builds a duration from an exact number of turns.
    ///
    /// Reference: `calendar.h:211-214`.
    #[must_use]
    pub const fn from_turns(turns: i32) -> Self {
        Self { turns }
    }

    /// Builds a duration from a number of seconds (one second is one turn).
    ///
    /// Reference: `calendar.h:220-222`.
    #[must_use]
    pub const fn from_seconds(seconds: i32) -> Self {
        Self { turns: seconds }
    }

    /// Builds a duration from a number of minutes.
    ///
    /// Reference: `calendar.h:223-226` (multiplies by 60 through `from_turns`).
    #[must_use]
    pub const fn from_minutes(minutes: i32) -> Self {
        Self::from_turns(minutes * 60)
    }

    /// Builds a duration from a number of hours.
    ///
    /// Reference: `calendar.h:227-230`.
    #[must_use]
    pub const fn from_hours(hours: i32) -> Self {
        Self::from_minutes(hours * 60)
    }

    /// Builds a duration from a number of days.
    ///
    /// Reference: `calendar.h:231-234`.
    #[must_use]
    pub const fn from_days(days: i32) -> Self {
        Self::from_hours(days * 24)
    }

    /// Builds a duration from a number of weeks.
    ///
    /// Reference: `calendar.h:235-238`.
    #[must_use]
    pub const fn from_weeks(weeks: i32) -> Self {
        Self::from_days(weeks * 7)
    }

    /// Builds a duration from a number of moves (one turn is 100 moves).
    ///
    /// Reference: `calendar.h:215-218`, integer division truncating toward zero.
    #[must_use]
    pub const fn from_moves(moves: i32) -> Self {
        Self { turns: moves / 100 }
    }

    /// The number of whole turns in this duration.
    ///
    /// Reference: `calendar.h:252-255`.
    #[must_use]
    pub const fn to_turns(self) -> i32 {
        self.turns
    }

    /// The number of whole seconds in this duration.
    ///
    /// Reference: `calendar.h:256-259`. Seconds and turns have the same size.
    #[must_use]
    pub const fn to_seconds(self) -> i32 {
        self.turns
    }

    /// The number of whole minutes in this duration, truncating toward zero.
    ///
    /// Reference: `calendar.h:260-263`, where the reference divides in the
    /// requested integer type.
    #[must_use]
    pub const fn to_minutes(self) -> i32 {
        self.turns / 60
    }

    /// The number of whole hours in this duration, truncating toward zero.
    ///
    /// Reference: `calendar.h:264-267`.
    #[must_use]
    pub const fn to_hours(self) -> i32 {
        self.turns / (60 * 60)
    }

    /// The number of whole days in this duration, truncating toward zero.
    ///
    /// Reference: `calendar.h:268-271`.
    #[must_use]
    pub const fn to_days(self) -> i32 {
        self.turns / (60 * 60 * 24)
    }

    /// The number of whole weeks in this duration, truncating toward zero.
    ///
    /// Reference: `calendar.h:272-275`.
    #[must_use]
    pub const fn to_weeks(self) -> i32 {
        self.turns / (60 * 60 * 24 * 7)
    }

    /// The number of moves in this duration (turns times 100).
    ///
    /// Reference: `calendar.h:276-279`.
    #[must_use]
    pub const fn to_moves(self) -> i32 {
        self.turns * 100
    }

    /// The duration in minutes as a floating-point number.
    ///
    /// Reference: `calendar.h:260-263` with `T = double`.
    #[must_use]
    pub fn to_minutes_f64(self) -> f64 {
        f64::from(self.turns) / 60.0
    }

    /// The duration in hours as a floating-point number.
    ///
    /// Reference: `calendar.h:264-267` with `T = double`.
    #[must_use]
    pub fn to_hours_f64(self) -> f64 {
        f64::from(self.turns) / (60.0 * 60.0)
    }

    /// The duration in days as a floating-point number.
    ///
    /// Reference: `calendar.h:268-271` with `T = double`.
    #[must_use]
    pub fn to_days_f64(self) -> f64 {
        f64::from(self.turns) / (60.0 * 60.0 * 24.0)
    }

    /// Scales the duration by a floating-point factor, truncating toward zero.
    ///
    /// Reference: `calendar.h:328-335`: the product is computed as a double and
    /// then stored in the integer turn count, which truncates toward zero.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // The reference stores the product in an int.
    pub fn mul_f64(self, factor: f64) -> Self {
        Self {
            turns: (f64::from(self.turns) * factor) as i32,
        }
    }

    /// Scales the duration by a single-precision factor, truncating toward zero.
    ///
    /// Reference: `calendar.h:328-335` with `T = float`. The reference's
    /// template computes `float * int` in single precision, so the port must
    /// not widen the product to a double.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn mul_f32(self, factor: f32) -> Self {
        Self {
            turns: (factor * self.turns as f32) as i32,
        }
    }
}

impl std::ops::Neg for TimeDuration {
    type Output = Self;

    /// Reference: `calendar.h:301-303`.
    fn neg(self) -> Self {
        Self { turns: -self.turns }
    }
}

impl std::ops::Add for TimeDuration {
    type Output = Self;

    /// Reference: `calendar.h:304-306`.
    fn add(self, rhs: Self) -> Self {
        Self {
            turns: self.turns + rhs.turns,
        }
    }
}

impl std::ops::Sub for TimeDuration {
    type Output = Self;

    /// Reference: `calendar.h:310-312`.
    fn sub(self, rhs: Self) -> Self {
        Self {
            turns: self.turns - rhs.turns,
        }
    }
}

impl std::ops::Mul<f64> for TimeDuration {
    type Output = Self;

    /// Reference: `calendar.h:328-331`, truncating as [`Self::mul_f64`] does.
    fn mul(self, rhs: f64) -> Self {
        self.mul_f64(rhs)
    }
}

impl std::ops::Mul<i32> for TimeDuration {
    type Output = Self;

    /// Reference: `calendar.h:328-331`.
    fn mul(self, rhs: i32) -> Self {
        Self {
            turns: self.turns * rhs,
        }
    }
}

impl std::ops::Div<i32> for TimeDuration {
    type Output = Self;

    /// Reference: `calendar.h:320-323`, integer division truncating toward zero.
    fn div(self, rhs: i32) -> Self {
        Self {
            turns: self.turns / rhs,
        }
    }
}

impl std::ops::Div for TimeDuration {
    type Output = f64;

    /// Reference: `calendar.h:316-319`: the ratio of the two turn counts.
    fn div(self, rhs: Self) -> f64 {
        f64::from(self.turns) / f64::from(rhs.turns)
    }
}

impl std::ops::Rem for TimeDuration {
    type Output = Self;

    /// Reference: `calendar.h:340-342`, C++ `%`, whose result has the sign of
    /// the dividend.
    fn rem(self, rhs: Self) -> Self {
        Self {
            turns: self.turns % rhs.turns,
        }
    }
}

/// A point in game time, stored as an integer turn.
///
/// Reference: `calendar.h:450-529`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimePoint {
    turn: i32,
}

impl TimePoint {
    /// The turn representing the start of the game clock.
    ///
    /// Reference: `calendar.h:557` (`calendar::turn_zero`).
    pub const TURN_ZERO: Self = Self { turn: 0 };

    /// The latest representable turn.
    ///
    /// Reference: `calendar.h:562` (`calendar::turn_max`).
    pub const TURN_MAX: Self = Self { turn: i32::MAX };

    /// Builds a time point from an exact turn number.
    ///
    /// Reference: `calendar.h:463-465`.
    #[must_use]
    pub const fn from_turn(turn: i32) -> Self {
        Self { turn }
    }

    /// The turn number of this point.
    ///
    /// Reference: `calendar.h:471-474`.
    #[must_use]
    pub const fn to_turn(self) -> i32 {
        self.turn
    }
}

impl std::ops::Sub for TimePoint {
    type Output = TimeDuration;

    /// Reference: `calendar.h:495-498`: the difference is a duration.
    fn sub(self, rhs: Self) -> TimeDuration {
        TimeDuration::from_turns(self.turn - rhs.turn)
    }
}

impl std::ops::Add<TimeDuration> for TimePoint {
    type Output = Self;

    /// Reference: `calendar.h:499-502`.
    fn add(self, rhs: TimeDuration) -> Self {
        Self {
            turn: self.turn + rhs.to_turns(),
        }
    }
}

impl std::ops::Sub<TimeDuration> for TimePoint {
    type Output = Self;

    /// Reference: `calendar.h:506-509`.
    fn sub(self, rhs: TimeDuration) -> Self {
        Self {
            turn: self.turn - rhs.to_turns(),
        }
    }
}

impl std::ops::Div<TimeDuration> for TimePoint {
    type Output = Self;

    /// Reference: `calendar.h:514-517`.
    fn div(self, rhs: TimeDuration) -> Self {
        Self {
            turn: self.turn / rhs.to_turns(),
        }
    }
}

impl std::ops::Mul<TimeDuration> for TimePoint {
    type Output = Self;

    /// Reference: `calendar.h:518-520`.
    fn mul(self, rhs: TimeDuration) -> Self {
        Self {
            turn: self.turn * rhs.to_turns(),
        }
    }
}

/// A season of the game year.
///
/// Reference: `calendar.h:22-28`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    /// Spring, season index 0.
    Spring,
    /// Summer, season index 1.
    Summer,
    /// Autumn, season index 2.
    Autumn,
    /// Winter, season index 3.
    Winter,
}

impl Season {
    /// The number of seasons in a year.
    ///
    /// Reference: `calendar.h:27` (`NUM_SEASONS`).
    pub const COUNT: i32 = 4;
}

/// The configuration the reference reads from globals and game options.
///
/// Every field corresponds to one reference global or option; no function in
/// this module reads process-wide state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalendarConfig {
    /// Length of one season in days.
    ///
    /// Reference: the `SEASON_LENGTH` option (`options.cpp:2794-2795`, range
    /// 14 to 127, default 91), stored by `calendar::set_season_length`
    /// (`calendar.cpp:733-736`).
    pub season_length_days: i32,
    /// Whether seasons never advance.
    ///
    /// Reference: the `ETERNAL_SEASON` option (`options.cpp:2800`), stored by
    /// `calendar::set_eternal_season` (`calendar.cpp:721-724`).
    pub eternal_season: bool,
    /// The season reported when `eternal_season` is enabled.
    ///
    /// Reference: `calendar::initial_season` (`calendar.cpp:36`), initialized
    /// to spring.
    pub initial_season: Season,
    /// The time point at which the current game started.
    ///
    /// Reference: `calendar::start_of_game` (`calendar.cpp:34`).
    pub start_of_game: TimePoint,
    /// Latitude used for the sun's position, in degrees.
    ///
    /// Reference: the `LATITUDE` external option
    /// (`data/core/external_options.json:550-555`, default 42.36 for Boston),
    /// read at `game.cpp:818`.
    pub latitude: f32,
    /// Longitude used for the sun's position, in degrees.
    ///
    /// Reference: the `LONGITUDE` external option
    /// (`data/core/external_options.json:557-562`, default -71.06), read at
    /// `game.cpp:818`.
    pub longitude: f32,
    /// Whether the sun is pinned at 90 degrees of altitude.
    ///
    /// Reference: the `ETERNAL_TIME_OF_DAY` option value `"day"`
    /// (`options.cpp:2802`, `game.cpp:816`).
    pub eternal_day: bool,
    /// Whether the sun is pinned ten degrees below astronomical dawn.
    ///
    /// Reference: the `ETERNAL_TIME_OF_DAY` option value `"night"`
    /// (`options.cpp:2802`, `game.cpp:815`).
    pub eternal_night: bool,
}

impl Default for CalendarConfig {
    /// The reference defaults: 91-day seasons (`options.cpp:2794`) and eternal
    /// season off (`options.cpp:2800`).
    fn default() -> Self {
        Self {
            season_length_days: 91,
            eternal_season: false,
            initial_season: Season::Spring,
            start_of_game: TimePoint::TURN_ZERO,
            latitude: 42.36,
            longitude: -71.06,
            eternal_day: false,
            eternal_night: false,
        }
    }
}

impl CalendarConfig {
    /// The configured season length.
    ///
    /// Reference: `calendar.cpp:717-720`: the option is clamped to at least one
    /// day, so a zero or negative setting still yields a one-day season.
    #[must_use]
    pub fn season_length(self) -> TimeDuration {
        TimeDuration::from_days(self.season_length_days.max(1))
    }

    /// The configured year length, four seasons.
    ///
    /// Reference: `calendar.cpp:712-715`.
    #[must_use]
    pub fn year_length(self) -> TimeDuration {
        self.season_length() * 4
    }

    /// The configured season length divided by the default season length of 91
    /// days.
    ///
    /// Reference: `calendar.cpp:750-753` (`season_from_default_ratio`) and
    /// `calendar.cpp:742-743` (`default_season_length`).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // The reference converts the turns to float first.
    pub fn season_from_default_ratio(self) -> f32 {
        let days = self.season_length().to_turns() as f32 / (60.0 * 60.0 * 24.0);
        days / 91.0
    }
}

/// One minute.
pub const MINUTE: TimeDuration = TimeDuration::from_seconds(60);
/// One hour.
pub const HOUR: TimeDuration = TimeDuration::from_minutes(60);
/// One day.
pub const DAY: TimeDuration = TimeDuration::from_hours(24);
/// One week.
pub const WEEK: TimeDuration = TimeDuration::from_days(7);

/// The reference's "indefinitely long" duration, `INT_MAX` turns.
///
/// Reference: `calendar.h:542-543`. Note `calendar::INDEFINITELY_LONG`
/// (`calendar.cpp:26`) is a different, smaller value used for moves.
pub const INDEFINITELY_LONG_DURATION: TimeDuration = TimeDuration::from_turns(i32::MAX);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClippedUnit {
    Forever,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Season,
    Year,
}

/// The duration rounded down to the largest unit that is no larger than it.
///
/// Reference: `calendar.cpp:501-535`. The order of the comparisons matters:
/// the season check treats eternal season as "no seasons to count", and the
/// year check is skipped in eternal season.
fn clipped_time(duration: TimeDuration, config: &CalendarConfig) -> (i32, ClippedUnit) {
    if duration >= INDEFINITELY_LONG_DURATION {
        return (0, ClippedUnit::Forever);
    }
    if duration < MINUTE {
        return (duration.to_seconds(), ClippedUnit::Second);
    }
    if duration < HOUR {
        return (duration.to_minutes(), ClippedUnit::Minute);
    }
    if duration < DAY {
        return (duration.to_hours(), ClippedUnit::Hour);
    }
    if duration < WEEK {
        return (duration.to_days(), ClippedUnit::Day);
    }
    if duration < config.season_length() || config.eternal_season {
        return (duration.to_weeks(), ClippedUnit::Week);
    }
    if duration < config.year_length() && !config.eternal_season {
        return (
            duration.to_turns() / config.season_length().to_turns(),
            ClippedUnit::Season,
        );
    }
    (
        duration.to_turns() / config.year_length().to_turns(),
        ClippedUnit::Year,
    )
}

/// Renders a clipped amount with its unit, singular only for exactly one.
///
/// Reference: `calendar.cpp:425-499`, the `clipped_align::none` branch, using
/// `n_gettext`, which selects the singular form only when the count is 1.
fn clipped_text(amount: i32, unit: ClippedUnit) -> String {
    let (one, many) = match unit {
        ClippedUnit::Forever => return "forever".to_string(),
        ClippedUnit::Second => ("second", "seconds"),
        ClippedUnit::Minute => ("minute", "minutes"),
        ClippedUnit::Hour => ("hour", "hours"),
        ClippedUnit::Day => ("day", "days"),
        ClippedUnit::Week => ("week", "weeks"),
        ClippedUnit::Season => ("season", "seasons"),
        ClippedUnit::Year => ("year", "years"),
    };
    let name = if amount == 1 { one } else { many };
    format!("{amount} {name}")
}

fn clipped_string(duration: TimeDuration, config: &CalendarConfig) -> String {
    let (amount, unit) = clipped_time(duration, config);
    clipped_text(amount, unit)
}

/// Formats a duration the way the reference shows it to the player.
///
/// Reference: `calendar.cpp:544-583`. At most two units appear; the second unit
/// is the remainder after the dividing unit, itself clipped to its largest
/// whole unit.
#[must_use]
pub fn to_string(duration: TimeDuration, config: &CalendarConfig) -> String {
    if duration >= INDEFINITELY_LONG_DURATION {
        return "forever".to_string();
    }
    if duration <= MINUTE {
        return clipped_string(duration, config);
    }

    let divider = if duration < HOUR {
        MINUTE
    } else if duration < DAY {
        HOUR
    } else if duration < WEEK {
        DAY
    } else if duration < config.season_length() || config.eternal_season {
        WEEK
    } else if duration < config.year_length() {
        config.season_length()
    } else {
        config.year_length()
    };

    let remainder = duration % divider;
    if remainder != TimeDuration::ZERO {
        return format!(
            "{} and {}",
            clipped_string(duration, config),
            clipped_string(remainder, config)
        );
    }
    clipped_string(duration, config)
}

/// The season of the year at a time point.
///
/// Reference: `calendar.cpp:812-830`. In eternal season every time point maps
/// to the configured initial season.
///
/// The reference casts the integer index to `season_type`, so a time point far
/// enough before turn zero to produce a negative index yields an out-of-range
/// enum value rather than a season. Game time never goes there; this port
/// reports the initial season for such an index.
#[must_use]
pub fn season_of_year(point: TimePoint, config: &CalendarConfig) -> Season {
    if config.eternal_season {
        return config.initial_season;
    }
    let index = point.to_turn() / config.season_length().to_turns() % Season::COUNT;
    match index {
        0 => Season::Spring,
        1 => Season::Summer,
        2 => Season::Autumn,
        3 => Season::Winter,
        _ => config.initial_season,
    }
}

/// The year number shown to the player, starting at one.
///
/// Reference: `calendar.cpp:834-835`.
#[must_use]
pub fn year(point: TimePoint, config: &CalendarConfig) -> i32 {
    point.to_turn() / config.year_length().to_turns() + 1
}

/// The time elapsed since the previous midnight.
///
/// Reference: `calendar.h:566-569`. The remainder keeps the sign of the input,
/// because the reference uses `%` without normalizing.
#[must_use]
pub fn time_past_midnight(point: TimePoint) -> TimeDuration {
    (point - TimePoint::TURN_ZERO) % DAY
}

/// The time elapsed since the start of the configured year.
///
/// Reference: `calendar.h:571-574`, using the configured year length.
#[must_use]
pub fn time_past_new_year(point: TimePoint, config: &CalendarConfig) -> TimeDuration {
    (point - TimePoint::TURN_ZERO) % config.year_length()
}

/// The whole minute within the current hour.
///
/// Reference: `calendar.h:576-580` with `T = int`.
#[must_use]
pub fn minute_of_hour(point: TimePoint) -> i32 {
    ((point - TimePoint::TURN_ZERO) % HOUR).to_minutes()
}

/// The whole hour within the current day.
///
/// Reference: `calendar.h:582-586` with `T = int`.
#[must_use]
pub fn hour_of_day(point: TimePoint) -> i32 {
    time_past_midnight(point).to_hours()
}

/// The whole day within the current season.
///
/// Reference: `calendar.h:588-593` with `T = int`.
#[must_use]
pub fn day_of_season(point: TimePoint, config: &CalendarConfig) -> i32 {
    ((point - TimePoint::TURN_ZERO) % config.season_length()).to_days()
}

/// One phase of the moon.
///
/// Reference: `calendar.h:36-55`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoonPhase {
    /// New moon, completely dark.
    New,
    /// One quarter lit, waxing.
    WaxingCrescent,
    /// One half lit, waxing.
    HalfMoonWaxing,
    /// Three quarters lit, waxing.
    WaxingGibbous,
    /// Full moon.
    Full,
    /// Three quarters lit, waning.
    WaningGibbous,
    /// One half lit, waning.
    HalfMoonWaning,
    /// One quarter lit, waning.
    WaningCrescent,
}

impl MoonPhase {
    /// The number of named phases, the reference's `MOON_PHASE_MAX`.
    ///
    /// Reference: `calendar.h:54`.
    pub const MAX: i32 = 8;
}

/// The default lunar month, 29.530588853 days truncated to whole turns.
///
/// Reference: `calendar.cpp:54-57`.
#[must_use]
pub fn lunar_month() -> TimeDuration {
    DAY.mul_f64(29.530_588_853)
}

/// The phase of the moon at a time point.
///
/// Reference: `calendar.cpp:95-106`. The moon phase duration scales with the
/// configured season length, and the phase changes at noon.
///
/// As with [`season_of_year`], a time point far enough before turn zero makes
/// the reference cast a negative index to `moon_phase`; this port reports a new
/// moon there.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // The reference casts the rounded index to int.
pub fn get_moon_phase(point: TimePoint, config: &CalendarConfig) -> MoonPhase {
    let phase_duration = lunar_month().mul_f32(config.season_from_default_ratio());
    // The +12h means "days since the last noon", which is when the phase flips.
    let num_middays = (point - TimePoint::TURN_ZERO + DAY / 2).to_days();
    let nearest_midnight = DAY * num_middays;
    let phase_change = nearest_midnight / phase_duration;
    let index = (phase_change * f64::from(MoonPhase::MAX)).round() as i32 % MoonPhase::MAX;
    match index {
        0 => MoonPhase::New,
        1 => MoonPhase::WaxingCrescent,
        2 => MoonPhase::HalfMoonWaxing,
        3 => MoonPhase::WaxingGibbous,
        4 => MoonPhase::Full,
        5 => MoonPhase::WaningGibbous,
        6 => MoonPhase::HalfMoonWaning,
        _ => MoonPhase::WaningCrescent,
    }
}

/// A day of the week.
///
/// Reference: `calendar.h:656-664`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    /// Sunday.
    Sunday,
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
}

impl Weekday {
    /// Day zero of the game calendar, a Thursday.
    ///
    /// Reference: `calendar.cpp:692` (`start_day`), chosen in the comment block
    /// at `calendar.cpp:666-686`.
    const GAME_START: i32 = Self::Thursday as i32;

    const fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Sunday,
            1 => Self::Monday,
            2 => Self::Tuesday,
            3 => Self::Wednesday,
            4 => Self::Thursday,
            5 => Self::Friday,
            _ => Self::Saturday,
        }
    }
}

/// The day of the week at a time point.
///
/// Reference: `calendar.cpp:664-695`: whole days since `start_of_game`, with a
/// single `+ 7` when the count is negative, then counted from Thursday.
#[must_use]
pub fn day_of_week(point: TimePoint, config: &CalendarConfig) -> Weekday {
    let mut days_since_start = (point - config.start_of_game).to_days();
    if days_since_start < 0 {
        days_since_start = days_since_start % 7 + 7;
    }
    Weekday::from_index((days_since_start + Weekday::GAME_START) % 7)
}

/// The point at which the game clock displays 12:00.
///
/// Reference: `calendar.cpp:336-341`. The expression is reproduced exactly,
/// including its surprising pre-midday result: for a time before 12:00 the
/// reference adds the negative offset instead of subtracting it, so only the
/// afternoon branch lands on 12:00 of the current day.
#[must_use]
pub fn noon(point: TimePoint) -> TimePoint {
    let time_of_day = (point - TimePoint::TURN_ZERO) % DAY;
    let till_noon = time_of_day - HOUR * 12;
    if till_noon > TimeDuration::ZERO {
        point - till_noon
    } else {
        point + till_noon
    }
}

/// An angle in radians, mirroring `units::angle`.
///
/// Reference: `units.h:685-714` stores angles in radians and
/// `units.h:1083-1115` converts to radians before calling the standard math
/// functions.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Angle {
    radians: f64,
}

impl Angle {
    /// Reference: `units.h:696-699` (`from_degrees`).
    fn from_degrees(degrees: f64) -> Self {
        Self {
            radians: degrees * std::f64::consts::PI / 180.0,
        }
    }

    /// Reference: `units.h:701-704` (`to_degrees`).
    fn to_degrees(self) -> f64 {
        self.radians * 180.0 / std::f64::consts::PI
    }

    /// Reference: `units.h:1083-1086`.
    fn sin(self) -> f64 {
        self.radians.sin()
    }

    /// Reference: `units.h:1088-1091`.
    fn cos(self) -> f64 {
        self.radians.cos()
    }
}

impl std::ops::Add for Angle {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            radians: self.radians + rhs.radians,
        }
    }
}

impl std::ops::Sub for Angle {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            radians: self.radians - rhs.radians,
        }
    }
}

impl std::ops::Neg for Angle {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            radians: -self.radians,
        }
    }
}

impl std::ops::Mul<f64> for Angle {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self {
        Self {
            radians: self.radians * rhs,
        }
    }
}

impl std::ops::Div<f64> for Angle {
    type Output = Self;

    fn div(self, rhs: f64) -> Self {
        Self {
            radians: self.radians / rhs,
        }
    }
}

impl std::ops::Div for Angle {
    type Output = f64;

    fn div(self, rhs: Self) -> f64 {
        self.radians / rhs.radians
    }
}

/// Reference: `units.h:1103-1106` (`atan2`).
fn atan2_angle(y: f64, x: f64) -> Angle {
    Angle {
        radians: y.atan2(x),
    }
}

/// Reference: `units.h:1108-1111` (`asin`).
fn asin_angle(x: f64) -> Angle {
    Angle { radians: x.asin() }
}

/// Reference: `units.h:1113-1116` (`acos`).
fn acos_angle(x: f64) -> Angle {
    Angle { radians: x.acos() }
}

/// A two-component vector with single-precision components.
///
/// Reference: `line.h:320-347` (`rl_vec2d`), whose fields are `float`.
#[derive(Debug, Clone, Copy)]
struct Vec2f {
    x: f32,
    y: f32,
}

/// A three-component vector with single-precision components.
///
/// Reference: `line.h:349-380` (`rl_vec3d`), whose fields are `float`.
#[derive(Debug, Clone, Copy)]
struct Vec3f {
    x: f32,
    y: f32,
    z: f32,
}

/// Converts an angle to a duration at fifteen degrees per hour.
///
/// Reference: `calendar.cpp:108-111` (`angle_to_time`). Dividing the angle by
/// 15 degrees yields a double, and multiplying by one hour stores the product
/// in the integer turn count, truncating toward zero.
fn angle_to_time(angle: Angle) -> TimeDuration {
    let hours = angle / Angle::from_degrees(15.0);
    TimeDuration::from_seconds((hours * 3_600.0) as i32)
}

/// Normalizes an angle into `[0, mod)`.
///
/// Reference: `units_utility.cpp:10-17`.
fn normalize(angle: Angle, modulus: Angle) -> Angle {
    let mut radians = angle.radians % modulus.radians;
    if radians < 0.0 {
        radians += modulus.radians;
    }
    Angle { radians }
}

/// The time used for season-dependent solar calculations.
///
/// Reference: `calendar.cpp:113-121` (`season_effective_time`). In eternal
/// season the day of the year is pinned to the day the game started, so the sun
/// does not move with the date.
fn season_effective_time(point: TimePoint, config: &CalendarConfig) -> TimePoint {
    if config.eternal_season {
        let start_midnight = config.start_of_game - time_past_midnight(config.start_of_game);
        start_midnight + time_past_midnight(point)
    } else {
        point
    }
}

/// The solar noon of the calendar day containing `point`.
///
/// Reference: `calendar.cpp:257-265` (`solar_noon_near`): the previous midnight
/// plus twelve hours.
fn solar_noon_near(point: TimePoint) -> TimePoint {
    let prior_midnight = point - time_past_midnight(point);
    prior_midnight + HOUR * 12
}

/// The sun's right ascension and declination.
///
/// Reference: `calendar.cpp:123-166`.
fn sun_ra_declination(
    point: TimePoint,
    timezone: TimeDuration,
    config: &CalendarConfig,
) -> (Angle, Angle) {
    let days_since_epoch = (point - TimePoint::TURN_ZERO - timezone).to_days_f64();

    let angle_per_day = Angle::from_degrees(360.0) / f64::from(config.year_length().to_days());

    let mean_long = angle_per_day * days_since_epoch;
    let mean_anomaly = Angle::from_degrees(77.0) + mean_long;
    let ecliptic_longitude = mean_long
        + Angle::from_degrees(1.915) * mean_anomaly.sin()
        + Angle::from_degrees(0.020) * (mean_anomaly * 2.0).sin();

    let obliquity = Angle::from_degrees(23.439_279);

    let eclip = Vec2f {
        x: ecliptic_longitude.cos() as f32,
        y: ecliptic_longitude.sin() as f32,
    };
    let rot = Vec3f {
        x: eclip.x,
        y: (f64::from(eclip.y) * obliquity.cos()) as f32,
        z: (f64::from(eclip.y) * obliquity.sin()) as f32,
    };
    let right_ascension = atan2_angle(f64::from(rot.y), f64::from(rot.x));
    let declination = asin_angle(f64::from(rot.z));
    (right_ascension, declination)
}

/// Sidereal time at the given point.
///
/// Reference: `calendar.cpp:168-184`.
fn sidereal_time_at(
    point: TimePoint,
    longitude: Angle,
    timezone: TimeDuration,
    config: &CalendarConfig,
) -> Angle {
    let days_since_epoch = (point - TimePoint::TURN_ZERO - timezone).to_days_f64();
    let angle_per_day = Angle::from_degrees(360.0) / f64::from(config.year_length().to_days());

    let base = Angle::from_degrees(180.0);
    let advance = Angle::from_degrees(360.0) + angle_per_day;
    base + advance * days_since_epoch + longitude
}

/// The sun's azimuth and altitude.
///
/// Reference: `calendar.cpp:186-235` (`sun_azimuth_altitude`). Azimuth is
/// measured from the south, positive towards the west.
fn sun_azimuth_altitude(point: TimePoint, config: &CalendarConfig) -> (Angle, Angle) {
    let t = season_effective_time(point, config);
    let longitude = Angle::from_degrees(f64::from(config.longitude));
    let timezone = angle_to_time(longitude);
    let (right_ascension, declination) = sun_ra_declination(t, timezone, config);
    let sidereal_time = sidereal_time_at(t, longitude, timezone, config);

    let hour_angle = sidereal_time - right_ascension;

    let intermediate = Vec3f {
        x: (hour_angle.cos() * declination.cos()) as f32,
        y: (hour_angle.sin() * declination.cos()) as f32,
        z: declination.sin() as f32,
    };

    let latitude = Angle::from_degrees(f64::from(config.latitude));
    let horizontal = Vec3f {
        x: (-f64::from(intermediate.x) * latitude.sin()
            + f64::from(intermediate.z) * latitude.cos()) as f32,
        y: intermediate.y,
        z: (f64::from(intermediate.x) * latitude.cos() + f64::from(intermediate.z) * latitude.sin())
            as f32,
    };

    let azimuth = normalize(
        -atan2_angle(f64::from(horizontal.y), f64::from(horizontal.x)) + Angle::from_degrees(180.0),
        Angle::from_degrees(360.0),
    );
    let mut altitude = asin_angle(f64::from(horizontal.z));

    if config.eternal_day {
        altitude = Angle::from_degrees(90.0);
    }
    if config.eternal_night {
        altitude = Angle::from_degrees(-18.0) - Angle::from_degrees(10.0);
    }

    (azimuth, altitude)
}

/// The sun's altitude in degrees.
///
/// Reference: `calendar.cpp:237-240` (`sun_altitude`), returning the altitude
/// half of [`sun_azimuth_altitude`].
#[must_use]
pub fn sun_altitude_degrees(point: TimePoint, config: &CalendarConfig) -> f64 {
    sun_azimuth_altitude(point, config).1.to_degrees()
}

/// The sun's azimuth in degrees, measured from the south towards the west.
///
/// Reference: `calendar.cpp:186-235`.
#[must_use]
pub fn sun_azimuth_degrees(point: TimePoint, config: &CalendarConfig) -> f64 {
    sun_azimuth_altitude(point, config).0.to_degrees()
}

/// The angle between the current sidereal time and the time the sun reaches
/// `altitude`, in the direction asked for.
///
/// Reference: `calendar.cpp:267-291` (`offset_to_sun_altitude`). When the
/// requested altitude is never reached, the cosine of the hour angle is
/// clamped to plus or minus one, which pins the result to the sun's maximum or
/// minimum altitude.
fn offset_to_sun_altitude(
    altitude: Angle,
    longitude: Angle,
    approx_time: TimePoint,
    evening: bool,
    config: &CalendarConfig,
) -> Angle {
    let timezone = angle_to_time(longitude);
    let (right_ascension, declination) = sun_ra_declination(approx_time, timezone, config);
    let latitude = Angle::from_degrees(f64::from(config.latitude));
    let mut cos_hour_angle =
        (altitude.sin() - latitude.sin() * declination.sin()) / latitude.cos() / declination.cos();
    if cos_hour_angle.abs() > 1.0 {
        cos_hour_angle = if cos_hour_angle > 0.0 { 1.0 } else { -1.0 };
    }
    let mut hour_angle = acos_angle(cos_hour_angle);
    if !evening {
        hour_angle = -hour_angle;
    }
    let target_sidereal_time = hour_angle + right_ascension;
    let sidereal_time_at_approx_time = sidereal_time_at(approx_time, longitude, timezone, config);
    normalize(
        target_sidereal_time - sidereal_time_at_approx_time,
        Angle::from_degrees(360.0),
    )
}

/// The time of day at which the sun reaches `altitude`.
///
/// Reference: `calendar.cpp:293-314` (`sun_at_altitude`): one approximation
/// from solar noon, then one correction step.
fn sun_at_altitude(
    altitude: Angle,
    longitude: Angle,
    point: TimePoint,
    evening: bool,
    config: &CalendarConfig,
) -> TimePoint {
    let solar_noon = solar_noon_near(point);
    let mut initial_offset = offset_to_sun_altitude(
        altitude,
        longitude,
        season_effective_time(solar_noon, config),
        evening,
        config,
    );
    if !evening {
        initial_offset = initial_offset - Angle::from_degrees(360.0);
    }
    let initial_offset_time = angle_to_time(initial_offset);
    let initial_approximation = solar_noon + initial_offset_time;
    let mut correction_offset = offset_to_sun_altitude(
        altitude,
        longitude,
        season_effective_time(initial_approximation, config),
        evening,
        config,
    );
    if correction_offset > Angle::from_degrees(180.0) {
        correction_offset = correction_offset - Angle::from_degrees(360.0);
    }
    let correction_offset_time = angle_to_time(correction_offset);
    initial_approximation + correction_offset_time
}

/// The six numbers the sun's position is computed from, for one time point.
///
/// Reference: `harness_sun_internals` (`src/calendar.cpp:192`), compiled only under `CATA_HARNESS`.
/// It exists because the chain from a time point to a sunrise is six steps long, and a disagreement
/// at the end of it says nothing about which step is fed wrongly. Comparing the six numbers instead
/// of the one answer is what turns "sunrise is 3,422 turns out" into "this input is wrong".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunInternals {
    /// Right ascension, in degrees.
    pub right_ascension: f64,
    /// Declination, in degrees.
    pub declination: f64,
    /// Sidereal time, in degrees.
    pub sidereal_time: f64,
    /// Sidereal time minus right ascension, in degrees.
    pub hour_angle: f64,
    /// Days since the epoch, with the timezone removed.
    pub days_since_epoch: f64,
    /// The location's latitude, in degrees.
    pub latitude: f64,
}

/// The six numbers the sun's position is computed from at `point`.
#[must_use]
pub fn sun_internals(point: TimePoint, config: &CalendarConfig) -> SunInternals {
    let time = season_effective_time(point, config);
    let timezone = angle_to_time(longitude_of(config));
    let (right_ascension, declination) = sun_ra_declination(time, timezone, config);
    let sidereal_time = sidereal_time_at(time, longitude_of(config), timezone, config);
    SunInternals {
        right_ascension: right_ascension.to_degrees(),
        declination: declination.to_degrees(),
        sidereal_time: sidereal_time.to_degrees(),
        hour_angle: (sidereal_time - right_ascension).to_degrees(),
        days_since_epoch: (time - TimePoint::TURN_ZERO - timezone).to_days_f64(),
        latitude: f64::from(config.latitude),
    }
}

/// The angle the reference defines sunrise at, -1 degree.
///
/// Reference: `calendar.cpp:42` (`sunrise_angle`).
fn sunrise_angle() -> Angle {
    Angle::from_degrees(-1.0)
}

/// The angle the reference defines civil dawn at, -6 degrees.
///
/// Reference: `calendar.cpp:41` (`civil_dawn`).
fn civil_dawn() -> Angle {
    Angle::from_degrees(-6.0)
}

fn longitude_of(config: &CalendarConfig) -> Angle {
    Angle::from_degrees(f64::from(config.longitude))
}

/// The time the sun crosses one degree below the horizon in the morning.
///
/// Reference: `calendar.cpp:316-319`.
#[must_use]
pub fn sunrise(point: TimePoint, config: &CalendarConfig) -> TimePoint {
    sun_at_altitude(sunrise_angle(), longitude_of(config), point, false, config)
}

/// The time the sun crosses one degree below the horizon in the evening.
///
/// Reference: `calendar.cpp:321-324`.
#[must_use]
pub fn sunset(point: TimePoint, config: &CalendarConfig) -> TimePoint {
    sun_at_altitude(sunrise_angle(), longitude_of(config), point, true, config)
}

/// The time it gets dark: the sun crossing six degrees below the horizon in
/// the evening.
///
/// Reference: `calendar.cpp:326-329`.
#[must_use]
pub fn night_time(point: TimePoint, config: &CalendarConfig) -> TimePoint {
    sun_at_altitude(civil_dawn(), longitude_of(config), point, true, config)
}

/// The time it gets light: the sun crossing six degrees below the horizon in
/// the morning.
///
/// Reference: `calendar.cpp:331-334`.
#[must_use]
pub fn daylight_time(point: TimePoint, config: &CalendarConfig) -> TimePoint {
    sun_at_altitude(civil_dawn(), longitude_of(config), point, false, config)
}

/// Whether the sun is below civil dawn.
///
/// Reference: `calendar.cpp:343-346`.
#[must_use]
pub fn is_night(point: TimePoint, config: &CalendarConfig) -> bool {
    sun_azimuth_altitude(point, config).1 <= civil_dawn()
}

/// Whether the sun is at or above sunrise.
///
/// Reference: `calendar.cpp:348-351`.
#[must_use]
pub fn is_day(point: TimePoint, config: &CalendarConfig) -> bool {
    sun_azimuth_altitude(point, config).1 >= sunrise_angle()
}

/// Whether the sun is between civil dawn and sunrise.
///
/// Reference: `calendar.cpp:353-357` (`is_twilight`), whose bounds are both
/// inclusive.
fn is_twilight(point: TimePoint, config: &CalendarConfig) -> bool {
    let altitude = sun_azimuth_altitude(point, config).1;
    altitude >= civil_dawn() && altitude <= sunrise_angle()
}

/// Whether it is dusk: twilight after midday.
///
/// Reference: `calendar.cpp:359-363`.
#[must_use]
pub fn is_dusk(point: TimePoint, config: &CalendarConfig) -> bool {
    let now = time_past_midnight(point);
    now > HOUR * 12 && is_twilight(point, config)
}

/// Whether it is dawn: twilight before midday.
///
/// Reference: `calendar.cpp:365-369`.
#[must_use]
pub fn is_dawn(point: TimePoint, config: &CalendarConfig) -> bool {
    let now = time_past_midnight(point);
    now < HOUR * 12 && is_twilight(point, config)
}

/// The light provided in full daylight.
///
/// Reference: `calendar.cpp:44-47`.
#[must_use]
pub fn default_daylight_level() -> f32 {
    100.0
}

/// The irradiance of clear daylight with the sun at 90 degrees, in watts per
/// square metre.
///
/// Reference: `calendar.cpp:49-52`.
#[must_use]
pub fn max_sun_irradiance() -> f32 {
    1000.0
}

/// The angle the reference defines astronomical dawn at, -18 degrees.
///
/// Reference: `calendar.cpp:39` (`astronomical_dawn`).
fn astronomical_dawn() -> Angle {
    Angle::from_degrees(-18.0)
}

/// The angle the reference defines nautical dawn at, -12 degrees.
///
/// Reference: `calendar.cpp:40` (`nautical_dawn`).
fn nautical_dawn() -> Angle {
    Angle::from_degrees(-12.0)
}

/// The sunlight at a time point.
///
/// Reference: `calendar.cpp:381-402`. Each branch raises an exponential in
/// degrees relative to one of the dawn angles, and the top branch is linear up
/// to 60 degrees of altitude, where it is clamped to 125.
#[must_use]
pub fn sun_light_at(point: TimePoint, config: &CalendarConfig) -> f32 {
    let solar_altitude = sun_azimuth_altitude(point, config).1;

    if solar_altitude < astronomical_dawn() {
        0.0
    } else if solar_altitude <= nautical_dawn() {
        // Rises exponentially from 0 to 3.7 while the sun goes from -18 to -12.
        (f64::from(3.7_f32)
            * (((solar_altitude - astronomical_dawn()).to_degrees() / 6.0).exp2() - 1.0))
            as f32
    } else if solar_altitude <= civil_dawn() {
        // Rises exponentially from 3.7 to 5.0 while the sun goes from -12 to -6.
        (f64::from(5.0_f32 - 3.7_f32)
            * (((solar_altitude - nautical_dawn()).to_degrees() / 6.0).exp2() - 1.0)
            + f64::from(3.7_f32)) as f32
    } else if solar_altitude <= sunrise_angle() {
        // Rises exponentially from 5.0 to 60 while the sun goes from -6 to -1.
        (f64::from(60.0_f32 - 5.0_f32)
            * (((solar_altitude - civil_dawn()).to_degrees() / 5.0).exp2() - 1.0)
            + f64::from(5.0_f32)) as f32
    } else if solar_altitude <= Angle::from_degrees(60.0) {
        // Grows linearly from 60 at -1 degree to 125 at 60 degrees.
        let slope = f64::from(65.0_f32 / 61.0_f32);
        (slope * solar_altitude.to_degrees() + slope + 60.0) as f32
    } else {
        125.0
    }
}

/// The irradiance on a flat surface at a time point, in watts per square metre.
///
/// Reference: `calendar.cpp:404-412`.
#[must_use]
pub fn sun_irradiance(point: TimePoint, config: &CalendarConfig) -> f32 {
    let solar_altitude = sun_azimuth_altitude(point, config).1;
    if solar_altitude < Angle::from_degrees(0.0) {
        return 0.0;
    }
    (f64::from(max_sun_irradiance()) * solar_altitude.sin()) as f32
}

/// The light the moon provides at a time point.
///
/// Reference: `calendar.cpp:371-379`. Waning phases are folded back onto the
/// waxing ones, so the light depends only on how many quarters are lit.
#[must_use]
pub fn moon_light_at(point: TimePoint, config: &CalendarConfig) -> f32 {
    let mut phase = get_moon_phase(point, config) as i32;
    if phase > MoonPhase::MAX / 2 {
        phase = MoonPhase::MAX - phase;
    }
    (1.0 + f64::from(1.5_f32 * phase as f32)) as f32
}

/// The sunlight plus moonlight at a time point.
///
/// Reference: `calendar.cpp:414-417`.
#[must_use]
pub fn sun_moon_light_at(point: TimePoint, config: &CalendarConfig) -> f32 {
    sun_light_at(point, config) + moon_light_at(point, config)
}

/// The sunlight plus moonlight at the solar noon of the current day.
///
/// Reference: `calendar.cpp:419-423`.
#[must_use]
pub fn sun_moon_light_at_noon_near(point: TimePoint, config: &CalendarConfig) -> f32 {
    sun_moon_light_at(solar_noon_near(point), config)
}
