//! The weather as part of a running world, not a value a test holds.
//!
//! The pieces were joined in round 511 into a tick; this is the same tick living where the game keeps it - a resource the turn reads - and handing the map what it needs: the wind
//! the fields and the fires are spread by, and the sight penalty the light is cast under. Reference: `weather_manager` as a global, read by `map::build_map_cache` and by the
//! field sweep through `get_weather().windspeed` (`weather.cpp:250-262`).

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp
)]

use bevy_ecs::prelude::*;
use cdda_content::weather::{Rules, WeatherType};
use cdda_core_types::level::Level;
use cdda_sim::weather_gen::{CommonData, Generator};
use cdda_sim::{Climate, Forecast, Map, Prototypes, Weather, tick_climate};

fn world(rules: Rules) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    world.insert_resource(Map(level));
    world.insert_resource(Prototypes::default());
    world.insert_resource(Weather::clear());
    world.insert_resource(Climate::new(rules));
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
    let weather = world.get_resource::<Weather>().expect("the weather").sight_penalty();
    assert_eq!(weather, 1.7, "the world is foggy");
    assert!(world.get_resource::<Map>().expect("the map").light_is_stale(), "and the light knows it");
    let climate = world.get_resource::<Climate>().expect("the climate");
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
    let climate = world.get_resource::<Climate>().expect("the climate");
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
    let from_climate = with_climate.get_resource::<Climate>().expect("the climate").windpower();
    assert_eq!(cdda_sim::wind_of(&with_climate, 99), from_climate, "the climate's wind");
    let without = World::new();
    assert_eq!(cdda_sim::wind_of(&without, 99), 99, "and the caller's, where there is no climate");
}
