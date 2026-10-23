//! What the weather costs the light, and what a clear sky costs.
//!
//! Source: `weather_type::sight_penalty` (`src/weather_type.h:89-90`) and its use in
//! `calc_transp` (`src/lightmap.cpp:150-154`), which charges it to every square under open sky.

#![allow(clippy::panic, clippy::float_cmp)]

use cdda_sim::Weather;

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
