//! Expectations for the time and calendar layer.
//!
//! Reference: `tests/calendar_test.cpp` (the reference's own assertions, cited
//! per test) and `src/calendar.h` / `src/calendar.cpp` for everything the
//! reference test file does not cover.

use cdda_core_types::calendar::{
    CalendarConfig, INDEFINITELY_LONG_DURATION, MoonPhase, Season, TimeDuration, TimePoint,
    Weekday, day_of_season, day_of_week, daylight_time, default_daylight_level, get_moon_phase,
    hour_of_day, is_dawn, is_day, is_dusk, is_night, lunar_month, max_sun_irradiance,
    minute_of_hour, moon_light_at, night_time, noon, season_of_year, sun_altitude_degrees,
    sun_azimuth_degrees, sun_irradiance, sun_light_at, sun_moon_light_at,
    sun_moon_light_at_noon_near, sunrise, sunset, time_past_midnight, time_past_new_year,
    to_string, year,
};

#[test]
fn seconds_are_stored_as_turns() {
    // calendar.h:220-222: from_seconds stores the value unchanged as turns.
    // calendar.h:256-259: to_seconds returns the stored turn count unchanged.
    let d = TimeDuration::from_seconds(3661);
    assert_eq!(d.to_turns(), 3661);
    assert_eq!(d.to_seconds(), 3661);
}

#[test]
fn integer_unit_conversions_truncate_toward_zero() {
    // calendar.h:260-271: to_minutes/to_hours divide the stored turn count in
    // the requested type, so integer conversions truncate toward zero.
    let ninety_minutes = TimeDuration::from_minutes(90);
    assert_eq!(ninety_minutes.to_minutes(), 90);
    assert_eq!(ninety_minutes.to_hours(), 1);
    assert_eq!(TimeDuration::from_minutes(-90).to_hours(), -1);
    assert_eq!(TimeDuration::from_seconds(-59).to_minutes(), 0);
}

#[test]
fn larger_units_are_whole_multiples_of_seconds() {
    // calendar.h:227-238: hours multiply minutes by 60, days multiply hours by
    // 24, weeks multiply days by 7, all through from_turns.
    let day = TimeDuration::from_days(1);
    assert_eq!(day.to_turns(), 86_400);
    assert_eq!(TimeDuration::from_hours(25).to_days(), 1);
    assert_eq!(TimeDuration::from_weeks(8).to_days(), 56);
    assert_eq!(TimeDuration::from_days(2).to_weeks(), 0);
    // calendar.h:276-279: moves are turns times 100.
    assert_eq!(TimeDuration::from_moves(25_000).to_moves(), 25_000);
    assert_eq!(TimeDuration::from_moves(-150).to_turns(), -1);
}

#[test]
fn floating_point_conversions_keep_the_fraction() {
    // calendar.h:260-271 with T = double: the same turn count is converted
    // before dividing, so no truncation happens.
    let ninety_minutes = TimeDuration::from_minutes(90);
    assert!((ninety_minutes.to_hours_f64() - 1.5).abs() < f64::EPSILON);
    assert!((ninety_minutes.to_days_f64() - 90.0 / 1_440.0).abs() < f64::EPSILON);
    assert!((TimeDuration::from_days(363).to_days_f64() - 363.0).abs() < f64::EPSILON);
}

#[test]
fn duration_arithmetic_matches_operator_definitions() {
    // calendar.h:301-342: unary minus, addition, subtraction, scaling and
    // remainder all act on the stored turn count.
    let ninety = TimeDuration::from_minutes(90);
    assert_eq!(
        ninety + TimeDuration::from_minutes(30),
        TimeDuration::from_hours(2)
    );
    assert_eq!(
        ninety - TimeDuration::from_hours(1),
        TimeDuration::from_minutes(30)
    );
    assert_eq!(-ninety, TimeDuration::from_minutes(-90));
    assert_eq!(ninety * 2, TimeDuration::from_hours(3));
    assert_eq!(ninety / 2, TimeDuration::from_minutes(45));
    // calendar.h:317-319: duration / duration is a double ratio.
    assert!(
        (TimeDuration::from_hours(1) / TimeDuration::from_minutes(90) - 2.0 / 3.0).abs()
            < f64::EPSILON
    );
    // calendar.h:340-342: remainder follows the dividend's sign, as C++ % does.
    assert_eq!(
        TimeDuration::from_days(465) % TimeDuration::from_days(364),
        TimeDuration::from_days(101)
    );
    assert_eq!(
        TimeDuration::from_seconds(-5) % TimeDuration::from_seconds(3),
        TimeDuration::from_seconds(-2)
    );
}

#[test]
fn durations_compare_by_turn_count() {
    // calendar.h:282-299: every comparison is a comparison of turn counts.
    assert!(TimeDuration::from_seconds(59) < TimeDuration::from_minutes(1));
    assert!(TimeDuration::from_minutes(1) <= TimeDuration::from_seconds(60));
    assert_eq!(
        TimeDuration::from_minutes(1),
        TimeDuration::from_seconds(60)
    );
    assert!(TimeDuration::from_minutes(1) >= TimeDuration::from_seconds(60));
    assert!(TimeDuration::from_seconds(61) > TimeDuration::from_minutes(1));
    assert_ne!(
        TimeDuration::from_seconds(61),
        TimeDuration::from_minutes(1)
    );
    // calendar.h:320-323: integer division truncates toward zero, and so does
    // scaling by a floating-point factor when the result is stored as an int.
    assert_eq!(
        TimeDuration::from_seconds(-5) / 2,
        TimeDuration::from_seconds(-2)
    );
    assert_eq!(
        TimeDuration::from_seconds(3).mul_f64(0.5),
        TimeDuration::from_seconds(1)
    );
    assert_eq!(
        TimeDuration::from_seconds(-3).mul_f64(0.5),
        TimeDuration::from_seconds(-1)
    );
}

#[test]
fn time_points_add_and_subtract_durations() {
    // calendar.h:495-512: point - point is a duration, point +/- duration is a
    // point, and all of them operate on the integer turn.
    let day = TimePoint::from_turn(86_400);
    assert_eq!(day.to_turn(), 86_400);
    assert_eq!(
        TimePoint::from_turn(0) + TimeDuration::from_hours(25),
        TimePoint::from_turn(90_000)
    );
    assert_eq!(
        day - TimeDuration::from_hours(1),
        TimePoint::from_turn(82_800)
    );
    assert_eq!(
        day - TimePoint::from_turn(3_600),
        TimeDuration::from_hours(23)
    );
    assert_eq!(TimePoint::default(), TimePoint::from_turn(0));
    // calendar.h:514-526: the reference also divides and multiplies a point by
    // a duration, keeping integer division semantics.
    assert_eq!(
        TimePoint::from_turn(100) / TimeDuration::from_seconds(3),
        TimePoint::from_turn(33)
    );
    assert_eq!(
        TimePoint::from_turn(100) * TimeDuration::from_seconds(3),
        TimePoint::from_turn(300)
    );
}

#[test]
fn time_points_compare_by_turn() {
    // calendar.h:476-493.
    let earlier = TimePoint::from_turn(-1);
    let midnight = TimePoint::from_turn(0);
    assert!(earlier < midnight);
    assert!(earlier <= midnight);
    assert!(midnight > earlier);
    assert!(midnight >= midnight);
    assert_eq!(midnight, TimePoint::from_turn(0));
    assert_ne!(midnight, earlier);
}

#[test]
fn duration_strings_use_the_largest_whole_unit_pair() {
    // tests/calendar_test.cpp:9-19, with the season length of 91 days set on
    // tests/calendar_test.cpp:8.
    let config = CalendarConfig::default();
    assert_eq!(
        to_string(TimeDuration::from_seconds(10), &config),
        "10 seconds"
    );
    assert_eq!(
        to_string(TimeDuration::from_seconds(60), &config),
        "1 minute"
    );
    assert_eq!(
        to_string(TimeDuration::from_seconds(70), &config),
        "1 minute and 10 seconds"
    );
    assert_eq!(to_string(TimeDuration::from_minutes(60), &config), "1 hour");
    assert_eq!(
        to_string(TimeDuration::from_minutes(70), &config),
        "1 hour and 10 minutes"
    );
    assert_eq!(to_string(TimeDuration::from_hours(24), &config), "1 day");
    assert_eq!(
        to_string(
            TimeDuration::from_hours(24) + TimeDuration::from_seconds(1),
            &config
        ),
        "1 day and 1 second"
    );
    assert_eq!(
        to_string(TimeDuration::from_hours(25), &config),
        "1 day and 1 hour"
    );
    assert_eq!(
        to_string(
            TimeDuration::from_hours(25) + TimeDuration::from_seconds(1),
            &config
        ),
        "1 day and 1 hour"
    );
    assert_eq!(to_string(TimeDuration::from_days(7), &config), "1 week");
    assert_eq!(
        to_string(TimeDuration::from_days(8), &config),
        "1 week and 1 day"
    );
}

#[test]
fn duration_strings_promote_to_season_and_year() {
    // tests/calendar_test.cpp:20-26, season length 91 days.
    let config = CalendarConfig::default();
    assert_eq!(to_string(TimeDuration::from_days(91), &config), "1 season");
    assert_eq!(
        to_string(TimeDuration::from_days(92), &config),
        "1 season and 1 day"
    );
    assert_eq!(
        to_string(TimeDuration::from_days(99), &config),
        "1 season and 1 week"
    );
    assert_eq!(to_string(TimeDuration::from_days(364), &config), "1 year");
    assert_eq!(
        to_string(TimeDuration::from_days(365), &config),
        "1 year and 1 day"
    );
    assert_eq!(
        to_string(TimeDuration::from_days(465), &config),
        "1 year and 1 season"
    );
    assert_eq!(
        to_string(TimeDuration::from_days(3650), &config),
        "10 years and 1 week"
    );
    // tests/calendar_test.cpp:43: 91 days is the default season length, so the
    // ratio of configured to default season length is exactly 1.
    assert!((config.season_from_default_ratio() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn eternal_season_counts_weeks_instead_of_seasons_or_years() {
    // tests/calendar_test.cpp:29-41: with eternal season on, weeks are the
    // largest unit the reference is willing to name.
    let config = CalendarConfig {
        season_length_days: 91,
        eternal_season: true,
        ..CalendarConfig::default()
    };
    assert_eq!(to_string(TimeDuration::from_days(7), &config), "1 week");
    assert_eq!(
        to_string(TimeDuration::from_days(8), &config),
        "1 week and 1 day"
    );
    assert_eq!(to_string(TimeDuration::from_days(91), &config), "13 weeks");
    assert_eq!(
        to_string(TimeDuration::from_days(92), &config),
        "13 weeks and 1 day"
    );
    assert_eq!(
        to_string(TimeDuration::from_days(99), &config),
        "14 weeks and 1 day"
    );
    assert_eq!(to_string(TimeDuration::from_days(364), &config), "52 weeks");
    assert_eq!(
        to_string(TimeDuration::from_days(365), &config),
        "52 weeks and 1 day"
    );
    assert_eq!(
        to_string(TimeDuration::from_days(465), &config),
        "66 weeks and 3 days"
    );
    assert_eq!(
        to_string(TimeDuration::from_days(3650), &config),
        "521 weeks and 3 days"
    );
}

#[test]
fn season_length_comes_from_config_and_is_clamped_to_one_day() {
    // calendar.cpp:717-720: season length is the option value in days, but
    // never less than one day.
    let config = CalendarConfig {
        season_length_days: 14,
        ..CalendarConfig::default()
    };
    assert_eq!(config.season_length(), TimeDuration::from_days(14));
    assert_eq!(config.year_length(), TimeDuration::from_days(56));
    let zero = CalendarConfig {
        season_length_days: 0,
        ..CalendarConfig::default()
    };
    assert_eq!(zero.season_length(), TimeDuration::from_days(1));
    let negative = CalendarConfig {
        season_length_days: -5,
        ..CalendarConfig::default()
    };
    assert_eq!(negative.season_length(), TimeDuration::from_days(1));
}

#[test]
fn season_of_year_uses_the_configured_season_length() {
    // calendar.cpp:812-830: season is the turn divided by the configured
    // season length, modulo four.
    let config = CalendarConfig {
        season_length_days: 14,
        ..CalendarConfig::default()
    };
    let day = |n: i32| TimePoint::from_turn(n * 86_400);
    assert_eq!(season_of_year(day(0), &config), Season::Spring);
    assert_eq!(season_of_year(day(13), &config), Season::Spring);
    assert_eq!(season_of_year(day(14), &config), Season::Summer);
    assert_eq!(season_of_year(day(28), &config), Season::Autumn);
    assert_eq!(season_of_year(day(42), &config), Season::Winter);
    assert_eq!(season_of_year(day(56), &config), Season::Spring);
    // calendar.cpp:819-823: eternal season returns the initial season for
    // every time point.
    let eternal = CalendarConfig {
        eternal_season: true,
        initial_season: Season::Autumn,
        ..CalendarConfig::default()
    };
    assert_eq!(season_of_year(day(100), &eternal), Season::Autumn);
}

#[test]
fn year_counts_configured_years_from_one() {
    // calendar.cpp:834-835: year is the turn divided by the configured year
    // length (truncating) plus one.
    let config = CalendarConfig::default();
    let day = |n: i32| TimePoint::from_turn(n * 86_400);
    assert_eq!(year(day(0), &config), 1);
    assert_eq!(year(day(363), &config), 1);
    assert_eq!(year(day(364), &config), 2);
    assert_eq!(year(day(727), &config), 2);
    assert_eq!(year(day(728), &config), 3);
    let short = CalendarConfig {
        season_length_days: 14,
        ..CalendarConfig::default()
    };
    assert_eq!(year(day(55), &short), 1);
    assert_eq!(year(day(56), &short), 2);
}

#[test]
fn time_of_day_helpers_keep_the_remainder() {
    // calendar.h:566-586: these are the duration modulo a day, a configured
    // year, or an hour, with integer truncation for the component accessors.
    let config = CalendarConfig::default();
    let afternoon = TimePoint::from_turn(86_400 + 13 * 3_600 + 45 * 60 + 30);
    assert_eq!(
        time_past_midnight(afternoon),
        TimeDuration::from_seconds(49_530)
    );
    assert_eq!(hour_of_day(afternoon), 13);
    assert_eq!(minute_of_hour(afternoon), 45);
    assert_eq!(day_of_season(afternoon, &config), 1);
    // One day thirteen hours into the year, not into the day.
    assert_eq!(
        time_past_new_year(afternoon, &config),
        TimeDuration::from_seconds(135_930)
    );
    let short = CalendarConfig {
        season_length_days: 14,
        ..CalendarConfig::default()
    };
    assert_eq!(
        time_past_new_year(TimePoint::from_turn(100 * 86_400), &short),
        TimeDuration::from_days(44)
    );
    assert_eq!(day_of_season(TimePoint::from_turn(100 * 86_400), &short), 2);
    // The reference does not normalize remainders of times before turn zero.
    assert_eq!(
        time_past_midnight(TimePoint::from_turn(-1)),
        TimeDuration::from_seconds(-1)
    );
    assert_eq!(hour_of_day(TimePoint::from_turn(-1)), 0);
}

#[test]
fn moon_phase_changes_at_noon() {
    // calendar.cpp:54-57: a lunar month is 29.530588853 days, truncated to
    // whole turns when the duration is built.
    assert_eq!(lunar_month(), TimeDuration::from_turns(2_551_442));
    // calendar.cpp:95-106: the phase is computed from the number of days to
    // the nearest midnight, and the switch happens at noon so that a phase
    // lasts the whole night.
    let config = CalendarConfig::default();
    let at = TimePoint::from_turn;
    assert_eq!(get_moon_phase(at(0), &config), MoonPhase::New);
    assert_eq!(
        get_moon_phase(at(86_400 + 43_200 - 1), &config),
        MoonPhase::New
    );
    assert_eq!(
        get_moon_phase(at(86_400 + 43_200), &config),
        MoonPhase::WaxingCrescent
    );
    assert_eq!(
        get_moon_phase(at(12 * 86_400 + 43_200 - 1), &config),
        MoonPhase::WaxingGibbous
    );
    assert_eq!(
        get_moon_phase(at(12 * 86_400 + 43_200), &config),
        MoonPhase::Full
    );
    assert_eq!(
        get_moon_phase(at(16 * 86_400 + 43_200), &config),
        MoonPhase::WaningGibbous
    );
}

#[test]
fn moon_phase_scales_with_season_length() {
    // calendar.cpp:97 multiplies the lunar month by season_from_default_ratio,
    // so shorter seasons run through the phases faster.
    let config = CalendarConfig {
        season_length_days: 14,
        ..CalendarConfig::default()
    };
    assert!((config.season_from_default_ratio() - 14.0 / 91.0).abs() < 1e-6);
    let noon = |day: i32| TimePoint::from_turn(day * 86_400 + 43_200);
    assert_eq!(get_moon_phase(noon(0), &config), MoonPhase::HalfMoonWaxing);
    assert_eq!(get_moon_phase(noon(7), &config), MoonPhase::HalfMoonWaning);
    assert_eq!(get_moon_phase(noon(14), &config), MoonPhase::HalfMoonWaxing);
    assert_eq!(get_moon_phase(noon(28), &config), MoonPhase::WaxingGibbous);
    assert_eq!(get_moon_phase(noon(56), &config), MoonPhase::Full);
}

#[test]
fn day_of_week_counts_whole_days_from_game_start() {
    // calendar.cpp:664-695: day zero is a Thursday, and the count is whole
    // days since start_of_game with the same negative adjustment as the
    // reference.
    let config = CalendarConfig::default();
    let day = TimePoint::from_turn;
    assert_eq!(day_of_week(day(0), &config), Weekday::Thursday);
    assert_eq!(day_of_week(day(86_400), &config), Weekday::Friday);
    assert_eq!(day_of_week(day(6 * 86_400), &config), Weekday::Wednesday);
    assert_eq!(day_of_week(day(7 * 86_400), &config), Weekday::Thursday);
    assert_eq!(day_of_week(day(-86_400), &config), Weekday::Wednesday);
    let shifted = CalendarConfig {
        start_of_game: TimePoint::from_turn(3 * 86_400),
        ..CalendarConfig::default()
    };
    assert_eq!(day_of_week(day(3 * 86_400), &shifted), Weekday::Thursday);
    assert_eq!(day_of_week(day(0), &shifted), Weekday::Monday);
}

#[test]
fn noon_returns_the_twelve_hundred_point_of_the_current_day() {
    // calendar.cpp:336-341. Note the reference quirk the formula produces: for
    // times before 12:00 it adds "time of day - 12h" to the original time
    // point instead of subtracting it, so the result is not the noon of that
    // day. The harness output below comes from executing that formula.
    let h = |hours: i32| TimePoint::from_turn(hours * 3_600);
    assert_eq!(noon(h(0)), TimePoint::from_turn(-43_200));
    assert_eq!(noon(h(6)), TimePoint::from_turn(0));
    assert_eq!(noon(h(12)), TimePoint::from_turn(43_200));
    assert_eq!(noon(h(13)), TimePoint::from_turn(43_200));
    assert_eq!(noon(h(18)), TimePoint::from_turn(43_200));
}

#[test]
fn sun_altitude_follows_the_reference_astronomy() {
    // calendar.cpp:123-240. Expected values were generated by compiling the
    // reference formulas from calendar.cpp (see the report for the harness).
    let config = CalendarConfig::default();
    let altitude = |turn: i32| sun_altitude_degrees(TimePoint::from_turn(turn), &config);
    assert!((altitude(0) - -46.788_820_579).abs() < 1e-6);
    assert!((altitude(6 * 3_600) - -0.625_363_032).abs() < 1e-6);
    assert!((altitude(12 * 3_600) - 48.634_423_167).abs() < 1e-6);
    assert!((altitude(18 * 3_600) - 1.974_411_462).abs() < 1e-6);
    assert!((altitude(90 * 86_400 + 43_200) - 71.075_592_934).abs() < 1e-6);
    assert!((altitude(270 * 86_400 + 43_200) - 24.226_418_378).abs() < 1e-6);
}

#[test]
fn sun_altitude_honours_eternal_time_and_polar_clamping() {
    // calendar.cpp:218-223: eternal day pins the altitude at 90 degrees and
    // eternal night ten degrees below astronomical dawn, i.e. -28 degrees.
    let mut config = CalendarConfig {
        eternal_day: true,
        ..CalendarConfig::default()
    };
    assert!((sun_altitude_degrees(TimePoint::from_turn(0), &config) - 90.0).abs() < 1e-12);
    config.eternal_day = false;
    config.eternal_night = true;
    assert!((sun_altitude_degrees(TimePoint::from_turn(0), &config) + 28.0).abs() < 1e-12);
    // calendar.cpp:278-282: when the sun never reaches the requested angle the
    // hour angle is clamped; at latitude 89 in winter the noon altitude is
    // therefore the maximum possible one.
    config.eternal_night = false;
    config.latitude = 89.0;
    assert!(
        (sun_altitude_degrees(TimePoint::from_turn(280 * 86_400 + 43_200), &config)
            - -22.230_944_605)
            .abs()
            < 1e-6
    );
}

#[test]
fn sunrise_and_sunset_match_the_reference_iteration() {
    // calendar.cpp:267-334: the reference solves for the moment the sun crosses
    // -1 degree with one correction step. Expected turn values come from the
    // compiled reference formulas.
    let config = CalendarConfig::default();
    let at = |turn: i32| TimePoint::from_turn(turn);
    let day = 86_400;
    assert_eq!(sunrise(at(43_200), &config), at(21_478));
    assert_eq!(sunset(at(43_200), &config), at(65_767));
    // Civil twilight (-6 degrees) bounds the light and the dark.
    assert_eq!(daylight_time(at(43_200), &config), at(19_851));
    assert_eq!(night_time(at(43_200), &config), at(67_398));
    assert_eq!(sunrise(at(day * 90 + 43_200), &config), at(7_791_727));
    assert_eq!(sunset(at(day * 90 + 43_200), &config), at(7_846_887));
    assert_eq!(sunrise(at(day * 270 + 43_200), &config), at(23_354_608));
    assert_eq!(sunset(at(day * 270 + 43_200), &config), at(23_387_423));
    assert_eq!(
        daylight_time(at(day * 270 + 43_200), &config),
        at(23_352_751)
    );
    assert_eq!(night_time(at(day * 270 + 43_200), &config), at(23_389_280));
}

#[test]
fn sunrise_uses_season_length_eternal_season_and_the_polar_clamp() {
    // calendar.cpp:141-173: the year length comes from the configured season
    // length, so a 14-day season moves the sun's declination faster.
    let mut config = CalendarConfig {
        season_length_days: 14,
        ..CalendarConfig::default()
    };
    let at = |turn: i32| TimePoint::from_turn(turn);
    assert_eq!(sunrise(at(43_200), &config), at(21_224));
    assert_eq!(sunset(at(43_200), &config), at(66_128));
    assert_eq!(daylight_time(at(43_200), &config), at(19_605));
    assert_eq!(night_time(at(43_200), &config), at(67_775));
    assert_eq!(sunrise(at(7 * 86_400 + 43_200), &config), at(621_820));
    assert_eq!(sunset(at(7 * 86_400 + 43_200), &config), at(674_003));
    // calendar.cpp:113-121: in eternal season the solar calculation uses the
    // pinned day, not the actual date.
    let eternal = CalendarConfig {
        eternal_season: true,
        start_of_game: at(5 * 86_400 + 7 * 3_600),
        ..CalendarConfig::default()
    };
    assert_eq!(
        sunrise(at(200 * 86_400 + 18 * 3_600), &eternal),
        at(17_300_955)
    );
    assert_eq!(
        sunset(at(200 * 86_400 + 18 * 3_600), &eternal),
        at(17_346_109)
    );
    // calendar.cpp:278-282: at latitude 89 in winter the sun never rises, so
    // both twilight crossings are clamped to the same instant. Back to the
    // 91-day season the harness value was produced with.
    config.season_length_days = 91;
    config.latitude = 89.0;
    assert_eq!(sunrise(at(280 * 86_400 + 43_200), &config), at(24_148_886));
    assert_eq!(sunset(at(280 * 86_400 + 43_200), &config), at(24_235_314));
    assert_eq!(
        daylight_time(at(280 * 86_400 + 43_200), &config),
        at(24_148_886)
    );
    assert_eq!(
        night_time(at(280 * 86_400 + 43_200), &config),
        at(24_235_314)
    );
}

#[test]
fn day_night_and_twilight_follow_the_sun_altitude() {
    // calendar.cpp:343-369. The boundaries are strict: is_day at -1 degrees,
    // is_night at -6 degrees, and twilight in between, split into dawn before
    // noon and dusk after it.
    let config = CalendarConfig::default();
    let at = TimePoint::from_turn;
    assert!(is_night(at(0), &config));
    assert!(!is_day(at(0), &config));
    assert!(!is_dawn(at(0), &config));
    assert!(!is_dusk(at(0), &config));
    // -6.0032 degrees: fully night, not yet twilight.
    assert!(is_night(at(19_850), &config));
    assert!(!is_dawn(at(19_850), &config));
    // -1.0043 degrees: twilight, so dawn (and not day yet).
    assert!(is_dawn(at(21_477), &config));
    assert!(!is_day(at(21_477), &config));
    assert!(!is_night(at(21_477), &config));
    assert!(!is_dusk(at(21_477), &config));
    // The reference's sunrise turn itself still sits just below -1 degrees,
    // because the angle is truncated to whole turns.
    assert!(is_dawn(at(21_478), &config));
    assert!(!is_day(at(21_478), &config));
    // -0.6254 degrees: day.
    assert!(is_day(at(21_600), &config));
    assert!(!is_dawn(at(21_600), &config));
    assert!(is_day(at(43_200), &config));
    // -0.9966 degrees: still day on the way down.
    assert!(is_day(at(65_766), &config));
    assert!(!is_dusk(at(65_766), &config));
    // -5.9957 degrees: dusk, not night yet.
    assert!(is_dusk(at(67_397), &config));
    assert!(!is_dawn(at(67_397), &config));
    assert!(!is_night(at(67_397), &config));
    assert!(!is_day(at(67_397), &config));
    // -13.855 degrees: night.
    assert!(is_night(at(70_000), &config));
    assert!(!is_dusk(at(70_000), &config));
}

#[test]
fn eternal_time_of_day_overrides_the_altitude_checks() {
    // calendar.cpp:218-223 pins the altitude, so the checks follow.
    let mut config = CalendarConfig {
        eternal_day: true,
        ..CalendarConfig::default()
    };
    assert!(is_day(TimePoint::from_turn(0), &config));
    assert!(!is_night(TimePoint::from_turn(0), &config));
    config.eternal_day = false;
    config.eternal_night = true;
    assert!(is_night(TimePoint::from_turn(0), &config));
    assert!(!is_day(TimePoint::from_turn(0), &config));
}

#[test]
fn daylight_and_sunlight_follow_the_altitude_curve() {
    // calendar.cpp:44-52 for the two constants, calendar.cpp:381-412 for the
    // piecewise sunlight and irradiance curves.
    assert!((default_daylight_level() - 100.0).abs() < f32::EPSILON);
    assert!((max_sun_irradiance() - 1000.0).abs() < f32::EPSILON);
    let config = CalendarConfig::default();
    let at = TimePoint::from_turn;
    assert!(sun_light_at(at(0), &config).abs() < 1e-6);
    assert!((sun_light_at(at(6 * 3_600), &config) - 60.399_204).abs() < 1e-4);
    assert!((sun_light_at(at(12 * 3_600), &config) - 112.889_145).abs() < 1e-4);
    assert!((sun_light_at(at(18 * 3_600), &config) - 63.169_457).abs() < 1e-4);
    // Above 60 degrees of altitude the curve is clamped to 125.
    assert!((sun_light_at(at(90 * 86_400 + 43_200), &config) - 125.0).abs() < 1e-4);
    assert!((sun_light_at(at(270 * 86_400 + 43_200), &config) - 86.880_61).abs() < 1e-4);
    assert!(sun_irradiance(at(0), &config).abs() < 1e-6);
    assert!((sun_irradiance(at(12 * 3_600), &config) - 750.508_24).abs() < 1e-3);
    assert!((sun_irradiance(at(18 * 3_600), &config) - 34.453_16).abs() < 1e-3);
    assert!((sun_irradiance(at(90 * 86_400 + 43_200), &config) - 945.947_27).abs() < 1e-3);
    // Eternal day and night pin the altitude, and the light follows it.
    let mut eternal = CalendarConfig {
        eternal_day: true,
        ..CalendarConfig::default()
    };
    assert!((sun_light_at(at(0), &eternal) - 125.0).abs() < 1e-4);
    assert!((sun_irradiance(at(0), &eternal) - 1000.0).abs() < 1e-3);
    eternal.eternal_day = false;
    eternal.eternal_night = true;
    assert!(sun_light_at(at(0), &eternal).abs() < 1e-6);
    assert!(sun_irradiance(at(0), &eternal).abs() < 1e-6);
}

#[test]
fn moon_light_is_one_plus_one_and_a_half_per_lit_quarter() {
    // calendar.cpp:371-379: moonlight is 1 plus 1.5 per lit quarter, counting
    // the waning phases back down from the full moon.
    let config = CalendarConfig::default();
    let at = TimePoint::from_turn;
    // Day 0 and day 90 noon are within the first quarter.
    assert!((moon_light_at(at(12 * 3_600), &config) - 1.0).abs() < 1e-6);
    assert!((moon_light_at(at(90 * 86_400 + 43_200), &config) - 2.5).abs() < 1e-6);
    // Full moon: four quarters lit.
    assert!((moon_light_at(at(45 * 86_400), &config) - 7.0).abs() < 1e-6);
    // Waning crescent (phase 7) folds back to one quarter.
    assert!((moon_light_at(at(23 * 86_400 + 43_200), &config) - 2.5).abs() < 1e-6);
    assert!((sun_moon_light_at(at(12 * 3_600), &config) - 113.889_145).abs() < 1e-4);
    // calendar.cpp:419-423 uses the solar noon of the current day, which at
    // turn zero is the noon that follows.
    assert!((sun_moon_light_at_noon_near(at(0), &config) - 113.889_145).abs() < 1e-4);
}

#[test]
fn reporting_pins_defaults_thresholds_and_the_azimuth() {
    // calendar.cpp:31 and `data/core/external_options.json:550-562`: the
    // default location is Boston, 42.36 north and 71.06 west.
    let config = CalendarConfig::default();
    assert!((config.latitude - 42.36).abs() < f32::EPSILON);
    assert!((config.longitude - -71.06).abs() < f32::EPSILON);
    // calendar.cpp:186-235: azimuth is measured from the south, positive to
    // the west; at the equinox noon it is just east of south.
    let at = |turn: i32| TimePoint::from_turn(turn);
    assert!((sun_azimuth_degrees(at(43_200), &config) - 357.474_107).abs() < 1e-4);
    // calendar.cpp:544-548: anything at or above INT_MAX turns prints
    // "forever" instead of a number.
    assert_eq!(
        to_string(INDEFINITELY_LONG_DURATION, &config),
        "forever".to_string()
    );
    assert_ne!(
        to_string(
            INDEFINITELY_LONG_DURATION - TimeDuration::from_seconds(1),
            &config
        ),
        "forever"
    );
}
