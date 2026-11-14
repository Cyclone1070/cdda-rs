//! The weather generator against the reference's own recorded hours.
//!
//! Source: `--dump-weather-series` on seed `weatherwatch`, which writes, for each hour, the generator's intermediate values (location, day and year
//! fraction, season, the modded seed) and the temperature it produced. The port feeds the same intermediates to `weather_gen::temperature_k` and has
//! to land on the same kelvin.
//!
//! Gating the formula before the calendar is honest rather than lazy: the calendar that produces a day fraction and a season is a separate piece of
//! machinery with its own rules, and a single all-or-nothing test would say only "something between the turn counter and the thermometer is wrong".

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::struct_field_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use cdda_json::{Value, parse};
use std::path::Path;

fn corpus() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/series-24h.json")
}

/// One recorded hour: the generator's inputs and the temperature it produced.
struct Hour {
    hour: i64,
    temperature_k: f64,
    humidity: f64,
    pressure: f64,
    windpower: f64,
    winddirection: f64,
    x: f64,
    y: f64,
    z: f64,
    year_fraction: f64,
    day_fraction: f64,
    season: usize,
    mod_seed: u32,
}

fn number(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(Value::Float(value)) => *value,
        Some(Value::Int(value)) => *value as f64,
        other => panic!("{key} is a number, not {other:?}"),
    }
}

fn hours() -> Vec<Hour> {
    let text = std::fs::read_to_string(corpus()).expect("the recorded weather");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(Value::Array(steps)) = root.get("steps") else {
        panic!("no steps")
    };
    steps
        .iter()
        .map(|step| {
            let Value::Object(step) = step else {
                panic!("a step is an object")
            };
            Hour {
                hour: number(step, "hour") as i64,
                temperature_k: number(step, "temperature_k"),
                humidity: number(step, "humidity"),
                pressure: number(step, "pressure"),
                windpower: number(step, "windpower"),
                winddirection: number(step, "winddirection"),
                x: number(step, "common_x"),
                y: number(step, "common_y"),
                z: number(step, "common_z"),
                year_fraction: number(step, "year_fraction"),
                day_fraction: number(step, "day_fraction"),
                season: number(step, "season") as usize,
                mod_seed: number(step, "mod_seed") as u32,
            }
        })
        .collect()
}

#[test]
fn the_generator_reproduces_the_recorded_temperatures() {
    let hours = hours();
    assert_eq!(hours.len(), 24, "a day of recorded weather");
    // The default region's settings, from `data/json/regional_map_settings.json`: an average of 6.5 C and no seasonal manual adjustment.
    let generator = cdda_sim::weather_gen::Generator::default_region();
    let mut worst = 0.0f64;
    let mut wrong = Vec::new();
    for hour in &hours {
        let common = cdda_sim::weather_gen::CommonData {
            x: hour.x,
            y: hour.y,
            z: hour.z,
            cyf: cdda_sim::weather_gen::year_cosine(hour.year_fraction),
            mod_seed: hour.mod_seed,
            season: hour.season,
        };
        let ours = cdda_sim::weather_gen::temperature_k(&generator, &common, hour.day_fraction);
        let difference = (ours - hour.temperature_k).abs();
        worst = worst.max(difference);
        if difference > 1e-6 && wrong.len() < 6 {
            wrong.push((hour.hour, ours, hour.temperature_k));
        }
    }
    assert!(
        wrong.is_empty(),
        "every recorded temperature, within a millionth of a kelvin; worst {worst:.9} K, wrong {wrong:?}"
    );
}

#[test]
fn the_year_cosine_peaks_at_midwinter_and_troughs_at_midsummer() {
    // Reference: `weather_gen.cpp:75-79` - one eighth is added to the year fraction so that the cosine is `+1` at midwinter even though a Cataclysm year
    // starts in spring. That puts the peak at seven-eighths of a year and the trough at three-eighths, which is midsummer, not the start of the summer
    // quarter: the eighth shifts the whole curve, it does not renumber the quarters. Getting that wrong makes every winter warm.
    let spring_start = cdda_sim::weather_gen::year_cosine(0.0);
    let midsummer = cdda_sim::weather_gen::year_cosine(0.375);
    let midwinter = cdda_sim::weather_gen::year_cosine(0.875);
    assert!(midsummer < -0.99, "midsummer is the trough: {midsummer}");
    assert!(midwinter > 0.99, "midwinter is the peak: {midwinter}");
    assert!(
        (spring_start - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12,
        "the year opens at the eighth: {spring_start}"
    );
}

#[test]
fn the_generator_reproduces_the_recorded_humidity_and_pressure() {
    // Both come from the same noise field at seeds shifted by 101 and 211 (`weather_gen.cpp:141-160`), and both are recorded to nine decimal places,
    // so the same millionth is the right tolerance. They are doubles in the reference - unlike the temperature - so there is no storage rounding to
    // account for: a difference here is a difference in the formula or in the seed arithmetic.
    let hours = hours();
    let generator = cdda_sim::weather_gen::Generator::default_region();
    let mut wrong = Vec::new();
    for hour in &hours {
        let common = cdda_sim::weather_gen::CommonData {
            x: hour.x,
            y: hour.y,
            z: hour.z,
            cyf: cdda_sim::weather_gen::year_cosine(hour.year_fraction),
            mod_seed: hour.mod_seed,
            season: hour.season,
        };
        let humidity = cdda_sim::weather_gen::humidity(&generator, &common);
        let pressure = cdda_sim::weather_gen::pressure(&generator, &common);
        let off =
            (humidity - hour.humidity).abs() > 1e-6 || (pressure - hour.pressure).abs() > 1e-6;
        if off && wrong.len() < 6 {
            wrong.push((hour.hour, humidity, hour.humidity, pressure, hour.pressure));
        }
    }
    assert!(
        wrong.is_empty(),
        "every recorded humidity and pressure, within a millionth: {wrong:?}"
    );
}

#[test]
fn humidity_is_clamped_to_a_percentage() {
    // Reference: `std::min( 100., std::max( 0., ... ) )` - a region with a high base humidity and a cold, wet season would otherwise report more than
    // a hundred percent, and the recorded series never reaches the clamp, so only a case built on purpose can show it works.
    let generator = cdda_sim::weather_gen::Generator {
        base_humidity: 100.0,
        ..cdda_sim::weather_gen::Generator::default_region()
    };
    let common = cdda_sim::weather_gen::CommonData {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        cyf: -1.0,
        mod_seed: 0,
        season: 0,
    };
    let wet = cdda_sim::weather_gen::humidity(&generator, &common);
    assert!(
        (0.0..=100.0).contains(&wet),
        "humidity stays a percentage: {wet}"
    );
}

/// The dice the reference rolled, hour by hour, from the trace the weather series was recorded with.
///
/// The trace carries a `mark weather.hour.N` before each update, so each hour's draws are the ones between its mark and the next: the wind's three, the
/// wind direction's check, and the duration the weather was rolled to last.
fn recorded_dice_by_hour() -> std::collections::BTreeMap<i64, Vec<(i32, i32, i32)>> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/series-24h.rolls.txt");
    let text = std::fs::read_to_string(path).expect("the recorded dice");
    let mut by_hour: std::collections::BTreeMap<i64, Vec<(i32, i32, i32)>> =
        std::collections::BTreeMap::new();
    let mut current: Option<i64> = None;
    for line in text.lines() {
        if let Some(hour) = line.strip_prefix("mark weather.hour.") {
            current = Some(hour.parse().expect("an hour number"));
            by_hour
                .entry(hour.parse().expect("an hour number"))
                .or_default();
        } else if let Some(rest) = line.strip_prefix("int ") {
            let mut parts = rest.split(' ');
            let Some(low) = parts.next().and_then(|text| text.parse::<i32>().ok()) else {
                continue;
            };
            let Some(high) = parts.next().and_then(|text| text.parse::<i32>().ok()) else {
                continue;
            };
            let Some(drawn) = parts.next().and_then(|text| text.parse::<i32>().ok()) else {
                continue;
            };
            if let Some(hour) = current {
                by_hour.entry(hour).or_default().push((low, high, drawn));
            }
        }
    }
    by_hour
}

#[test]
fn the_generator_reproduces_the_recorded_wind_power_from_the_recorded_dice() {
    // Reference: `weather_gen.cpp:160-164`. The wind is the one number in the generator that rolls: three dice go in - a multiplier, the exponent the
    // pressure is raised to, and the seasonal term's multiplier - and the port takes them from the caller rather than drawing its own. The trace gives
    // exactly the dice the reference used, hour by hour, so any difference here is the formula and not the luck.
    let hours = hours();
    let dice = recorded_dice_by_hour();
    assert!(
        dice.values().flatten().count() > 100,
        "a day of weather rolls more than a hundred times"
    );
    let generator = cdda_sim::weather_gen::Generator::default_region();
    let mut checked = 0;
    let mut wrong = Vec::new();
    for hour in &hours {
        // Hour zero's weather was decided before the recording began, so it rolled nothing.
        let Some(drawn) = dice.get(&hour.hour) else {
            continue;
        };
        if drawn.len() < 3 {
            continue;
        }
        let common = cdda_sim::weather_gen::CommonData {
            x: hour.x,
            y: hour.y,
            z: hour.z,
            cyf: cdda_sim::weather_gen::year_cosine(hour.year_fraction),
            mod_seed: hour.mod_seed,
            season: hour.season,
        };
        let mut supplied = drawn.iter();
        let mut roll = |low: i32, high: i32| {
            let (want_low, want_high, value) = *supplied
                .next()
                .expect("the formula asks for three dice and no more");
            assert_eq!(
                (low, high),
                (want_low, want_high),
                "hour {} asks for the dice in the reference's order",
                hour.hour
            );
            value
        };
        let ours = cdda_sim::weather_gen::wind_power(&generator, &common, hour.pressure, &mut roll);
        checked += 1;
        // The wind is an integer held in a double on the reference's side, so this compares two whole numbers and needs no margin.
        if (f64::from(ours) - hour.windpower).abs() > f64::EPSILON && wrong.len() < 6 {
            wrong.push((hour.hour, ours, hour.windpower));
        }
    }
    assert!(checked >= 20, "most of a day's hours rolled: {checked}");
    assert!(
        wrong.is_empty(),
        "every recorded wind power, from the recorded dice: {wrong:?}"
    );
}

#[test]
fn a_wind_direction_index_becomes_an_angle() {
    // Reference: `weather_gen.cpp:250-255`. The distribution picks one of sixteen sectors and `convert_winddir` multiplies by 22.5 and truncates, so the
    // sectors are 0, 22, 45, ..., 337 degrees - not 0, 22.5, 45. Truncation is the whole point of the test: rounding would put every odd sector one
    // degree out and nothing else in the system would notice.
    for (index, angle) in [
        (0usize, 0),
        (1, 22),
        (4, 90),
        (9, 202),
        (14, 315),
        (15, 337),
    ] {
        assert_eq!(
            cdda_sim::weather_gen::wind_direction_angle(index),
            angle,
            "sector {index} is {angle} degrees"
        );
    }
}

#[test]
fn the_direction_is_redrawn_exactly_when_the_reference_redrew_it() {
    // Reference: `weather_gen.cpp:165-178`. The direction is a `static` that starts unset, is drawn once, and is drawn again only when
    // `one_in( W * 2160 )` fires. `one_in` is true without rolling whenever its chance is one or less, so a dead calm - W of zero - redraws the direction
    // every time it happens, and the trace shows exactly that: every hour rolls a check except the hour whose wind is zero.
    let hours = hours();
    let dice = recorded_dice_by_hour();
    let mut previous: Option<i32> = None;
    let mut changed = Vec::new();
    let mut redrawn = Vec::new();
    for hour in &hours {
        let Some(drawn) = dice.get(&hour.hour) else {
            continue;
        };
        if drawn.len() < 3 {
            continue;
        }
        let wind = hour.windpower as i32;
        // The direction check is the draw that starts at zero, when the reference made one at all.
        let check = drawn
            .iter()
            .find(|(low, _, _)| *low == 0)
            .map(|(_, _, value)| *value);
        let must_redraw = cdda_sim::weather_gen::needs_new_direction(wind, check);
        if must_redraw {
            redrawn.push(hour.hour);
        }
        if previous.is_some_and(|was| was != hour.winddirection as i32) {
            changed.push(hour.hour);
        }
        previous = Some(hour.winddirection as i32);
    }
    assert_eq!(
        redrawn, changed,
        "the hours the port would redraw are the hours the direction changed"
    );
    assert!(
        !changed.is_empty(),
        "the recording contains at least one change, or this proves nothing"
    );
}

#[test]
fn the_first_update_draws_a_direction_and_the_next_calm_one_draws_again() {
    // The reference's `static int current_winddir = 1000` (`weather_gen.cpp:47`) is a sentinel, not an angle: the first update sees it and draws. After that the
    // direction only moves when the wind is calm or the check fires. A recorded day cannot show the first case, because its first hour was already decided
    // before the recording began.
    let mut wind = cdda_sim::weather_gen::WindDirection::default();
    assert!(
        wind.needs_redraw(4, None),
        "an unset direction is drawn whatever the wind is doing"
    );
    wind.set_from_sector(9);
    assert_eq!(wind.degrees, Some(202), "sector nine is 202 degrees");
    assert!(
        !wind.needs_redraw(4, Some(517)),
        "a steady wind with an unfired check keeps the direction"
    );
    assert!(
        wind.needs_redraw(0, None),
        "a dead calm redraws without rolling"
    );
    assert!(wind.needs_redraw(4, Some(0)), "a fired check redraws");
}

#[test]
fn the_temperature_s_terms_match_the_reference_bit_for_bit() {
    // **The arithmetic under the rounding.** The reference records the double it computed and the three terms that went into it, so a difference of one part in 10^15 can be
    // traced to the day's cosine, the seasonality or the noise instead of being blamed on the `float` the answer is stored in. Without this, an hour whose double sits within
    // an ulp of a float boundary looks exactly like a formula error - and one of the ten recorded days has such an hour.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/manager-240h.json");
    let text = std::fs::read_to_string(path).expect("the recorded manager");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON")
    else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(steps)) = root.get("steps") else {
        panic!("steps")
    };
    let generator = cdda_sim::weather_gen::Generator::default_region();
    let number = |object: &cdda_json::Object, key: &str| -> f64 {
        match object.get(key) {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => *value as f64,
            other => panic!("{key} is a number: {other:?}"),
        }
    };
    let mut worst: Vec<(i64, &str, f64, f64)> = Vec::new();
    for step in steps {
        let cdda_json::Value::Object(step) = step else {
            panic!("a step")
        };
        let common = cdda_sim::weather_gen::CommonData {
            x: number(step, "common_x"),
            y: number(step, "common_y"),
            z: number(step, "common_z"),
            cyf: cdda_sim::weather_gen::year_cosine(number(step, "year_fraction")),
            mod_seed: number(step, "mod_seed") as u32,
            season: number(step, "season") as usize,
        };
        let terms = cdda_sim::weather_gen::temperature_terms(
            &generator,
            &common,
            number(step, "day_fraction"),
        );
        let hour = number(step, "hour") as i64;
        for (name, ours, theirs) in [
            ("celsius", terms.celsius, number(step, "celsius_double")),
            ("day", terms.day, number(step, "dayv")),
            (
                "seasonality",
                terms.seasonality,
                number(step, "seasonality"),
            ),
            ("noise", terms.noise, number(step, "noise")),
            (
                "humidity_noise",
                cdda_sim::weather_gen::noise_for(&common, 101),
                number(step, "humidity_noise"),
            ),
            (
                "pressure_noise",
                cdda_sim::weather_gen::noise_for(&common, 211),
                number(step, "pressure_noise"),
            ),
        ] {
            if (ours - theirs).abs() > 1e-13 && worst.len() < 8 {
                worst.push((hour, name, ours, theirs));
            }
        }
    }
    assert!(
        worst.is_empty(),
        "every term matches the reference's double: {worst:?}"
    );
}
