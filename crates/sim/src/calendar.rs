//! What time it is, and which season that makes it.
//!
//! Reference: `calendar::turn` (`calendar.h`) is one number - the turn count since the game's epoch - and everything else about the date
//! is **derived** from it: the hour, the day, the season, the year. That is the shape the port keeps, and it is the reason this is a
//! component rather than a struct of fields: the store holds the turn, and every question about the calendar is answered from it.
//!
//! The season matters before the rest of the calendar does because two systems already want it and have been passing it in by hand: the
//! weather generator takes a season (`weather_gen.rs`) and the `is_season` guard asks for one.

use bevy_ecs::prelude::*;

/// One turn is one second, so a day is 86,400 turns.
///
/// Reference: `time_duration::from_seconds( t )` is `time_duration( t )` and `from_days( d )` is `from_hours( d * 24 )` through
/// `from_minutes( m * 60 )` (`calendar.h:214-234`) - every step multiplies by sixty, so a day is `24 * 60 * 60` turns exactly.
///
/// The port used to say `24 * 60 * 60 / 6`, from an older Cataclysm whose turn was six seconds. That made every duration measured in
/// days six times too short, and the corpse-rotten rule - which asks for ten days - was the one place it showed.
pub const TURNS_PER_DAY: i64 = 24 * 60 * 60;

/// Days in a season, from the data's `SEASON_LENGTH` option.
///
/// Reference: `external_options.json:620-625`, whose stub value is 91 days.
pub const DEFAULT_SEASON_LENGTH_DAYS: i64 = 91;

/// The turn count, and the season length that turns it into a date.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calendar {
    /// Turns since the epoch.
    pub turn: i64,
    /// Days in a season.
    pub season_length_days: i64,
}

impl Default for Calendar {
    fn default() -> Self {
        Self {
            turn: 0,
            season_length_days: DEFAULT_SEASON_LENGTH_DAYS,
        }
    }
}

impl Calendar {
    /// Which season a turn falls in, counted from spring.
    ///
    /// Reference: `season_of_year( const time_point & )` (`calendar.cpp`), which divides the day of the year by the season length and
    /// takes the remainder in fours - so the seasons repeat every four seasons' worth of days and nothing else is needed to know one.
    #[must_use]
    pub fn season(&self) -> Season {
        let days = self.turn.div_euclid(TURNS_PER_DAY);
        let length = self.season_length_days.max(1);
        match days.div_euclid(length).rem_euclid(4) {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }
}

/// The four seasons, in the order the reference's enumerator counts them.
///
/// Reference: `season_type` (`calendar.h:57-63`), whose names are the strings the `is_season` guard compares against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    /// The year's first season.
    Spring,
    /// The second.
    Summer,
    /// The third.
    Autumn,
    /// The fourth.
    Winter,
}

impl Season {
    /// The season's name, as the data writes it.
    ///
    /// Reference: `season_type`'s spellings, which lower case: `"spring"`, `"summer"`, `"autumn"`, `"winter"`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Spring => "spring",
            Self::Summer => "summer",
            Self::Autumn => "autumn",
            Self::Winter => "winter",
        }
    }
}

/// The calendar, if the world has one.
#[must_use]
pub fn of(world: &World) -> Option<&Calendar> {
    let mut query = world.try_query::<&Calendar>()?;
    query.iter(world).next()
}

/// Sets the turn count, creating the calendar if this is the first time the world has been told what time it is.
pub fn set_turn(world: &mut World, turn: i64) {
    let entity =
        crate::singletons::singleton::<Calendar, TheCalendar>(world);
    if let Some(mut calendar) = world.get_mut::<Calendar>(entity) {
        calendar.turn = turn;
    }
}

/// Sets how many days a season lasts, which the data's `SEASON_LENGTH` option decides.
pub fn set_season_length(world: &mut World, days: i64) {
    let entity = crate::singletons::singleton::<Calendar, TheCalendar>(world);
    if let Some(mut calendar) = world.get_mut::<Calendar>(entity) {
        calendar.season_length_days = days.max(1);
    }
}

/// Which season it is now, or nothing when nothing has set the clock.
#[must_use]
pub fn season_of_year(world: &World) -> Option<Season> {
    of(world).map(Calendar::season)
}

/// Marks the calendar, the way every other singleton is marked.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TheCalendar;
