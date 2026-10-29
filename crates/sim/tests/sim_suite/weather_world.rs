//! The weather as part of a running world, not a value a test holds.
//!
//! The pieces were joined in round 511 into a tick; this is the same tick living where the game keeps it - a resource the turn reads - and handing the map what it needs: the wind
//! the fields and the fires are spread by, and the sight penalty the light is cast under. Reference: `weather_manager` as a global, read by `map::build_map_cache` and by the
//! field sweep through `get_weather().windspeed` (`weather.cpp:250-262`).

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp
)]

use bevy_ecs::prelude::*;
use cdda_content::weather::{Rules, WeatherType};
use cdda_core_types::level::Level;
use cdda_sim::weather_gen::{CommonData, Generator};
use cdda_sim::{
    Climate, Forecast, Map, Prototypes, Weather, climate, climate_read, set_climate, set_weather,
    tick_climate, weather,
};

fn world(rules: Rules) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    cdda_sim::load_level(&mut world, level);
    cdda_sim::set_prototypes(&mut world, Prototypes::default());
    set_weather(&mut world, Weather::clear());
    set_climate(&mut world, Climate::new(rules));
    world
}

fn fog_rules() -> Rules {
    // One type whose condition always holds, so the decision is `fog` on every update: the point is not what this gate is about.
    let kinds = vec![WeatherType { id: "fog".to_string(), sight_penalty: 1.7, ..Default::default() }];
    let mut conditions = std::collections::BTreeMap::new();
    conditions.insert("fog".to_string(), cdda_content::condition::Condition::Always);
    Rules::new(kinds, conditions, cdda_content::jmath::Registry::default())
}

fn forecast(turn: i64) -> Forecast<'static> {
    // A generator with no seasonal adjustment, at a fixed place: the numbers do not matter here, only that the tick runs.
    static GENERATOR: std::sync::OnceLock<Generator> = std::sync::OnceLock::new();
    Forecast {
        generator: GENERATOR.get_or_init(Generator::default_region),
        common: CommonData { x: 0.0, y: 0.0, z: 0.0, cyf: 0.6, mod_seed: 12_345, season: 0 },
        day_fraction: 0.5,
        turn,
    }
}

#[test]
fn a_sky_that_dims_the_light_reaches_the_world_s_weather_and_its_map() {
    // `fog` absorbs 1.7 where clear air absorbs 1.0, so the world's weather resource has to say so and the map has to know its light is stale. Both are what the render path reads;
    // a weather system that computed the right id and never told anyone would pass every test written so far.
    let mut world = world(fog_rules());
    let mut roll = |_low: i32, _high: i32| 300;
    let mut sector = |_season: usize| 9;
    tick_climate(&mut world, &forecast(0), &mut roll, &mut sector);
    let weather = weather(&world).expect("the weather").sight_penalty();
    assert_eq!(weather, 1.7, "the world is foggy");
    assert!(world.get_resource::<Map>().expect("the map").light_is_stale(), "and the light knows it");
    let climate = climate_read(&world).expect("the climate");
    assert_eq!(climate.weather(), Some("fog"), "the id is the world's, not the test's");
}

#[test]
fn a_sky_with_the_same_absorption_leaves_the_light_alone() {
    // The other direction, at the level a running world sees it: `sunny` after `clear` is a new id and the same penalty, so nothing is redrawn.
    let mut world = world(fog_rules());
    let mut roll = |_low: i32, _high: i32| 300;
    let mut sector = |_season: usize| 9;
    tick_climate(&mut world, &forecast(0), &mut roll, &mut sector);
    // Build the light so the staleness question has an answer that means something.
    world.get_resource_mut::<Map>().expect("the map").cached_visibility((5, 5), 2, |_code| false);
    assert!(!world.get_resource::<Map>().expect("the map").light_is_stale(), "the build cleared the mark");
    tick_climate(&mut world, &forecast(400), &mut roll, &mut sector);
    assert!(
        !world.get_resource::<Map>().expect("the map").light_is_stale(),
        "fog to fog: a new update with the same penalty does not dirty the light"
    );
}

#[test]
fn the_world_s_wind_comes_from_the_weather_and_not_from_the_caller() {
    // The field sweep takes the wind as an argument today; a world with a climate has one answer, and it is this one.
    let mut world = world(fog_rules());
    let mut roll = |low: i32, high: i32| low.midpoint(high);
    let mut sector = |_season: usize| 9;
    tick_climate(&mut world, &forecast(0), &mut roll, &mut sector);
    let climate = climate_read(&world).expect("the climate");
    let expected = cdda_sim::weather_gen::wind_power(
        forecast(0).generator,
        &forecast(0).common,
        cdda_sim::weather_gen::pressure(forecast(0).generator, &forecast(0).common),
        &mut |low, high| low.midpoint(high),
    );
    assert_eq!(climate.windpower(), expected, "the world's wind is the generator's wind");
}

#[test]
fn the_world_s_wind_wins_over_the_one_a_caller_passes() {
    // The field sweep keeps its wind parameter because a session replay has no climate and drives the wind from its fixture. Once a climate exists the parameter is a fallback, and a
    // world that quietly used both would spread half its smoke one way and half the other.
    let mut with_climate = world(fog_rules());
    let mut roll = |low: i32, high: i32| low.midpoint(high);
    let mut sector = |_season: usize| 9;
    tick_climate(&mut with_climate, &forecast(0), &mut roll, &mut sector);
    let from_climate = climate_read(&with_climate).expect("the climate").windpower();
    assert_eq!(cdda_sim::wind_of(&with_climate, 99), from_climate, "the climate's wind");
    let without = World::new();
    assert_eq!(cdda_sim::wind_of(&without, 99), 99, "and the caller's, where there is no climate");
}

/// The weather rules the shipped data declares, through the loader a world would use.
fn shipped_rules() -> Rules {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/weather_type.json");
    let text = std::fs::read_to_string(path).expect("the weather data");
    Rules::read_json(text.as_bytes()).expect("the weather rules")
}

fn recorded_hours() -> Vec<cdda_json::Object> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/manager-240h.json");
    let text = std::fs::read_to_string(path).expect("the recorded manager");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else { panic!("root") };
    let Some(cdda_json::Value::Array(steps)) = root.get("steps") else { panic!("steps") };
    steps
        .iter()
        .map(|step| match step {
            cdda_json::Value::Object(object) => object.clone(),
            _ => panic!("a step"),
        })
        .collect()
}

fn dice_by_hour() -> std::collections::BTreeMap<i64, Vec<(i32, i32, i32)>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weather/manager-240h.rolls.txt");
    let text = std::fs::read_to_string(path).expect("the recorded dice");
    let mut by_hour: std::collections::BTreeMap<i64, Vec<(i32, i32, i32)>> = std::collections::BTreeMap::new();
    let mut hour: Option<i64> = None;
    for line in text.lines() {
        if let Some(number) = line.strip_prefix("mark weather.manager.hour.") {
            let parsed: i64 = number.parse().expect("an hour");
            hour = Some(parsed);
            by_hour.entry(parsed).or_default();
        } else if let Some(rest) = line.strip_prefix("int ") {
            let mut parts = rest.split(' ');
            let (Some(low), Some(high), Some(drawn)) = (parts.next(), parts.next(), parts.next()) else { continue };
            if let (Ok(low), Ok(high), Ok(drawn), Some(hour)) =
                (low.parse::<i32>(), high.parse::<i32>(), drawn.parse::<i32>(), hour)
            {
                by_hour.entry(hour).or_default().push((low, high, drawn));
            }
        }
    }
    by_hour
}

#[test]
fn ten_recorded_days_replay_through_the_world_hour_by_hour() {
    // **The manager's own path, over weather that actually changes.** The earlier gate ran a day of `sunny` and `clear`, whose sight penalties are both one; this one runs ten days of
    // the reference's manager with fog, drizzle and rain in them - fourteen changes of absorption - and asks the world for the same id, the same numbers and the same next turn every
    // hour, plus a flush exactly where the sky's absorption moved.
    //
    // Hour zero is seeded from the recording for the reason it was in round 511: the reference's first state was chosen during world generation, before the recording began, and its
    // trace shows the first hour drawing nothing.
    let hours = recorded_hours();
    let dice = dice_by_hour();
    assert!(hours.len() >= 200, "ten days of hours: {}", hours.len());
    let mut world = world(shipped_rules());
    let generator = Generator::default_region();

    let number = |object: &cdda_json::Object, key: &str| -> f64 {
        match object.get(key) {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => *value as f64,
            other => panic!("{key} is a number: {other:?}"),
        }
    };
    let id_of = |object: &cdda_json::Object| -> String {
        match object.get("weather") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            other => panic!("an id: {other:?}"),
        }
    };
    let rules = shipped_rules();
    let penalty = |id: &str| -> f32 { cdda_content::weather::WeatherRules::sight_penalty(&rules, id) };
    let first = &hours[0];
    {
        let mut climate = climate(&mut world).expect("the climate");
        climate.system.manager.weather = Some(id_of(first));
        climate.system.manager.point = Some(cdda_content::weather::Point {
            turn: number(first, "turn") as i64,
            temperature: number(first, "temperature"),
            humidity: number(first, "humidity"),
            pressure: number(first, "pressure"),
            windpower: number(first, "windpower") as i32,
            winddirection: number(first, "winddirection") as i32,
        });
        climate.system.manager.next_turn = number(first, "next") as i64;
        climate.system.wind.degrees = Some(number(first, "winddirection") as i32);
    }
    // The sector draws, in the order the world will ask for them: the recorded direction divided by 22.5, for every hour whose direction differs from the hour before.
    let mut sectors: std::collections::VecDeque<usize> = hours
        .windows(2)
        .filter(|pair| number(&pair[1], "winddirection") != number(&pair[0], "winddirection"))
        .map(|pair| (number(&pair[1], "winddirection") / 22.5).round() as usize)
        .collect();
    let expected_sectors = sectors.len();

    let mut wrong: Vec<(i64, String, String, String)> = Vec::new();
    let mut flushes = 0;
    let mut checked = 0;
    for step in &hours[1..] {
        let hour = number(step, "hour") as i64;
        let recorded = dice.get(&hour).expect("every hour after the first rolls");
        let mut supplied = recorded.iter();
        let mut roll = |low: i32, high: i32| {
            let (want_low, want_high, drawn) = *supplied.next().expect("no more dice than the reference asked for");
            assert_eq!((low, high), (want_low, want_high), "hour {hour} asks for the dice the reference asked for");
            drawn
        };
        let mut sector = |_season: usize| sectors.pop_front().expect("a sector for every redraw");
        let turn = number(step, "turn") as i64;
        let change = tick_climate(
            &mut world,
            &Forecast {
                generator: &generator,
                common: CommonData {
                    x: number(step, "common_x"),
                    y: number(step, "common_y"),
                    z: number(step, "common_z"),
                    cyf: cdda_sim::weather_gen::year_cosine(number(step, "year_fraction")),
                    mod_seed: number(step, "mod_seed") as u32,
                    season: number(step, "season") as usize,
                },
                day_fraction: number(step, "day_fraction"),
                turn,
            },
            &mut roll,
            &mut sector,
        );
        checked += 1;
        if let cdda_content::weather::Change::Weather { sight_changed: true } = change {
            flushes += 1;
        }
        let climate = climate_read(&world).expect("the climate");
        let point = climate.system.manager.point.expect("a point");
        let mut note = |what: &str, ours: String, theirs: String| {
            if ours != theirs && wrong.len() < 8 {
                wrong.push((hour, what.to_string(), ours, theirs));
            }
        };
        note("weather", climate.weather().unwrap_or("none").to_string(), id_of(step));
        note("next", climate.system.manager.next_turn.to_string(), number(step, "next").to_string());
        note("windpower", point.windpower.to_string(), number(step, "windpower").to_string());
        note("winddirection", point.winddirection.to_string(), number(step, "winddirection").to_string());
        // Both sides of every comparison here are values the reference **stored**, so exactness is the right demand: the temperature is a `float` and the port narrows to one
        // too, which means the two are equal or one of them is wrong. The earlier one-ulp tolerance was hiding a fixture written to ten digits - the port was computing a cosine
        // of a rounded fraction.
        let near = |what: &str, ours: f64, theirs: f64, wrong: &mut Vec<(i64, String, String, String)>| {
            // Every number in the recording is written to seventeen significant digits, so nothing here is rounded by the file. What is left is the last bit of
            // `cos`: the recording was made on macOS and these tests run on Linux, and the two C libraries' trigonometric functions differ by one unit in the last
            // place on some angles. The temperature is compared exactly because it is stored as a `float`, which absorbs that bit; the humidity and the pressure are
            // `double` in the reference, so one ulp is the tightest claim that survives two platforms.
            let tolerance = if what == "temperature" { 0.0 } else { theirs.abs() * f64::EPSILON };
            if (ours - theirs).abs() > tolerance && wrong.len() < 8 {
                wrong.push((hour, what.to_string(), format!("{ours}"), format!("{theirs}")));
            }
        };
        // The generator's numbers as well as the manager's bookkeeping: with the recording's own place, season and seed, the whole point has to match.
        near("temperature", point.temperature, number(step, "temperature"), &mut wrong);
        near("humidity", point.humidity, number(step, "humidity"), &mut wrong);
        near("pressure", point.pressure, number(step, "pressure"), &mut wrong);
    }
    assert_eq!(sectors.len(), 0, "every sector the world asked for was a recorded change: {expected_sectors} of them");
    assert!(checked >= 200, "the whole recording was replayed: {checked}");
    assert!(wrong.is_empty(), "the world disagrees with the recording: {wrong:?}");
    let expected_flushes = hours
        .windows(2)
        .filter(|pair| penalty(&id_of(&pair[0])) != penalty(&id_of(&pair[1])))
        .count();
    assert!(expected_flushes >= 10, "the recording contains real transitions: {expected_flushes}");
    assert_eq!(flushes, expected_flushes, "a flush exactly where the absorption moved");
}
