//! Weather prototypes, against the reference's own dump.

#![allow(clippy::expect_used, clippy::panic, clippy::float_cmp)]

use cdda_content::weather::{self, Precipitation, WeatherType};
use cdda_json::parse;
use std::path::Path;

fn dump() -> Vec<WeatherType> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/weather.json");
    let text = std::fs::read_to_string(path).expect("the weather dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    weather::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_weather_type_the_reference_defines() {
    assert_eq!(dump().len(), 20, "weather types");
}

#[test]
fn the_sight_penalty_is_the_multiplier_the_light_code_applies() {
    // The number is **at or above one** because `calc_transp` multiplies every square's transparency by it (`lightmap.cpp:150-154`): higher is more absorbing. Fog is the worst in
    // the data at 1.7 and clear weather is the identity at 1.0, so a loader that took these for a percentage would show as a value below one.
    let types = dump();
    assert_eq!(
        weather::find(&types, "clear").expect("clear").sight_penalty,
        1.0
    );
    assert_eq!(
        weather::find(&types, "fog").expect("fog").sight_penalty,
        1.7
    );
    // **One type carries zero**, which means "no penalty written" rather than "better than open air": the reference's multiplier is `1.0` for unaffected weather, so a reader that
    // treats zero literally would divide sight by it. Pinned here so the weather resource maps it to the identity instead.
    let unset: Vec<&str> = types
        .iter()
        .filter(|weather| weather.sight_penalty < 1.0)
        .map(|weather| weather.id.as_str())
        .collect();
    assert_eq!(unset, vec!["null"], "the one type with no penalty written");
    assert!(
        types
            .iter()
            .filter(|weather| weather.id != "null")
            .all(|weather| weather.sight_penalty >= 1.0),
        "every real weather makes sight worse or equal"
    );
}

#[test]
fn the_precipitation_class_and_the_rain_flag_are_separate_facts() {
    // `rains` says which of rain or snow this is; `precip` says how much of it. Snow is light precipitation that does not rain, which is what a loader folding the two together
    // would get wrong.
    let types = dump();
    let snowing = weather::find(&types, "snowing").expect("snowing");
    // **Snowing is heavy precipitation**, not light: the data decides, and this test first guessed otherwise. Snow is `rains == false`, which is the fact that survives.
    assert_eq!(snowing.precip, Precipitation::Heavy);
    assert!(!snowing.rains, "snow is not rain");
    // **Rain is heavy and drizzle is light**; the data draws the line where it wants it, and this test first guessed the other way round. Snow's heaviest type is `snowing`, which
    // is why the two flags are separate facts: heaviness is not rain, and rain is not heaviness.
    let drizzle = weather::find(&types, "drizzle").expect("drizzle");
    assert_eq!(drizzle.precip, Precipitation::Light);
    assert!(drizzle.rains);
    let rain = weather::find(&types, "rain").expect("rain");
    assert_eq!(rain.precip, Precipitation::Heavy);
    assert!(rain.rains);
    let flurries = weather::find(&types, "flurries").expect("flurries");
    assert_eq!(flurries.precip, Precipitation::Light);
    assert!(!flurries.rains, "flurries are snow");
    assert_eq!(
        weather::find(&types, "clear").expect("clear").precip,
        Precipitation::None
    );
    assert_eq!(
        weather::find(&types, "thunder").expect("thunder").precip,
        Precipitation::Heavy
    );
}
