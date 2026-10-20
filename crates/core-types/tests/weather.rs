//! Local wind: how the weather's wind speed becomes the number a field processor sees.
//!
//! Transcribed from `get_local_windpower` (`src/weather.cpp:785-810`) and
//! `convert_wind_to_coord` (`src/weather.cpp:852-872`).

#![allow(clippy::panic)]

use cdda_core_types::weather::{local_wind_power, wind_vector};

#[test]
fn being_sheltered_stops_the_wind_entirely() {
    assert_eq!(local_wind_power(20, false, 0, true, false), 0);
    assert_eq!(
        local_wind_power(20, true, 3, true, true),
        0,
        "shelter wins over everything"
    );
}

#[test]
fn forest_halves_the_wind() {
    assert_eq!(local_wind_power(7, true, 0, false, false), 3);
    assert_eq!(
        local_wind_power(7, false, 0, false, false),
        7,
        "open ground takes all of it"
    );
}

#[test]
fn height_adds_at_most_five_per_level() {
    // windpower + z * min(5, windpower)
    assert_eq!(local_wind_power(7, false, 2, false, false), 17);
    assert_eq!(
        local_wind_power(3, false, 2, false, false),
        9,
        "and a weak wind adds only itself"
    );
}

#[test]
fn a_wall_upwind_cuts_it_to_a_tenth() {
    assert_eq!(local_wind_power(20, false, 0, false, true), 2);
    // The reference applies the modifiers in order: forest first, then height, then the wall.
    assert_eq!(local_wind_power(20, true, 0, false, true), 1);
}

#[test]
fn the_wind_angle_becomes_one_of_eight_directions() {
    // The reference's table, thresholds and all: an angle at or above a threshold takes that
    // row's vector, which is why 330 maps to north and 0 also maps to north.
    assert_eq!(wind_vector(0), (0, -1));
    assert_eq!(wind_vector(31), (1, -1));
    assert_eq!(wind_vector(60), (1, 0));
    assert_eq!(wind_vector(121), (1, 1));
    assert_eq!(wind_vector(150), (0, 1));
    assert_eq!(wind_vector(211), (-1, 1));
    assert_eq!(wind_vector(240), (-1, 0));
    assert_eq!(wind_vector(301), (-1, -1));
    assert_eq!(wind_vector(330), (0, -1));
    assert_eq!(wind_vector(359), (0, -1));
}
