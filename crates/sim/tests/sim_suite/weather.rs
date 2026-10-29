//! What the weather costs the light, and what a clear sky costs.
//!
//! Source: `weather_type::sight_penalty` (`src/weather_type.h:89-90`) and its use in
//! `calc_transp` (`src/lightmap.cpp:150-154`), which charges it to every square under open sky.

#![allow(clippy::panic, clippy::float_cmp, clippy::expect_used)]

use cdda_sim::Weather;
use cdda_sim::{light};

#[test]
fn a_clear_sky_costs_nothing() {
    assert_eq!(Weather::clear().sight_penalty(), 1.0);
    assert_eq!(Weather::default().sight_penalty(), 1.0);
}

#[test]
fn fog_costs_more_than_rain_which_costs_nothing_at_all() {
    // The numbers are the data's (`weather_type.json`): rain 1.1, snow 1.3, fog 1.7, and a clear sky 1.0. They are all at least one because the value **raises a
    // square's absorption** rather than lowering a transmission.
    let clear = Weather::clear().sight_penalty();
    let rain = Weather::with_sight_penalty(1.1).sight_penalty();
    let snow = Weather::with_sight_penalty(1.3).sight_penalty();
    let fog = Weather::with_sight_penalty(1.7).sight_penalty();
    assert!(clear < rain && rain < snow && snow < fog);
}

#[test]
#[should_panic(expected = "a sight penalty below one")]
fn weather_that_brightens_the_world_is_refused() {
    // A penalty below one has the direction of the multiplier backwards: it would make fog brighter than clear air. The reference's data never has one, so a caller
    // that passes half is wrong rather than unusual, and the port says so at the door.
    let _ = Weather::with_sight_penalty(0.5);
}

#[test]
fn a_loaded_weather_type_becomes_the_penalty_and_an_unwritten_one_is_clear_air() {
    // The loader hands over what the data says, and the data says zero for `null` - a type nothing wrote a penalty for. Passing that on would trip the resource's own guard, so the
    // conversion treats "unwritten" as the identity, which is what the reference's multiplier means when no weather is in force.
    use cdda_content::weather::{self, WeatherType};
    use cdda_json::parse;
    use cdda_sim::weather::Weather;
    use std::path::Path;

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/weather.json");
    let text = std::fs::read_to_string(path).expect("the weather dump exists");
    let types = weather::from_dump(&parse(text.as_bytes()).expect("valid JSON")).expect("loads");

    let fog = weather::find(&types, "fog").expect("fog");
    assert_eq!(Weather::from_type(fog).sight_penalty(), 1.7);

    let unwritten = weather::find(&types, "null").expect("the unwritten type");
    assert_eq!(
        Weather::from_type(unwritten),
        Weather::clear(),
        "an unwritten penalty is clear air, not a division by zero"
    );
    let _ = WeatherType::clone(fog);
}

#[test]
fn fog_dims_a_lamp_by_the_absorption_it_adds_to_every_square() {
    // The number the loader brings is used the way `calc_transp` uses it (`lightmap.cpp:150-154`): every square under open sky absorbs `sight_penalty` times more, and the cast
    // attenuates by `e^( -transparency * distance )`. So five squares out, fog's extra 0.7 of one unit is `exp( -(1.7 - 1.0) * TRANSPARENCY_OPEN_AIR * 5 )` of the clear-weather
    // light - arithmetic worked out here rather than asked of the port.
    use bevy_ecs::prelude::*;
    use cdda_core_types::level::Level;
    use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;
    use cdda_sim::weather::Weather;
    use cdda_sim::{LightSource, Position, cast_lights_with, set_weather};

    fn lit_in(the_sky: Weather) -> f32 {
        let mut world = World::new();
        let mut level = Level::new();
        level.ensure(0, 0, 1);
        for y in 0..12 {
            for x in 0..12 {
                level.set_terrain(x, y, 1, 1);
            }
        }
        cdda_sim::load_level(&mut world, level);
        set_weather(&mut world, the_sky);
        world.spawn((
            Position { x: 5, y: 5, z: 0 },
            LightSource { luminance: 20.0 },
        ));
        cast_lights_with(&mut world);
        light(&world).max_at(10, 5)
    }

    let clear = lit_in(Weather::clear());
    let foggy = lit_in(Weather::with_sight_penalty(1.7));
    let expected = clear * (-(0.7_f32) * TRANSPARENCY_OPEN_AIR * 5.0).exp();
    assert!(clear > 0.0, "the lamp lights five squares out: {clear}");
    assert!(foggy < clear, "fog dims it: {foggy} against {clear}");
    assert!(
        (foggy - expected).abs() < 0.05 * clear,
        "and by the attenuation it adds: {foggy} against {expected}"
    );
}
