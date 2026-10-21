//! How many cities an overmap holds.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{
    CitySettings, cities_for, city_coverage_ratio, max_city_size, tiles_per_city,
};

#[test]
fn a_world_with_no_cities_has_none() {
    let settings = CitySettings {
        size: 0,
        ..CitySettings::default()
    };
    assert_eq!(cities_for(settings, 0.99), 0);
}

#[test]
fn each_step_of_spacing_halves_the_urban_share() {
    assert!((city_coverage_ratio(0) - 1.0).abs() < f64::EPSILON);
    assert!((city_coverage_ratio(1) - 0.5).abs() < f64::EPSILON);
    assert!((city_coverage_ratio(4) - 0.0625).abs() < f64::EPSILON);
}

#[test]
fn a_city_takes_up_its_own_square_and_three_quarters_of_it_again() {
    // (8 * 2 + 1) squared, times three quarters.
    let expected = 17.0 * 17.0 * 0.75;
    assert!((tiles_per_city(8, 8) - expected).abs() < f64::EPSILON);
}

#[test]
fn the_default_world_keeps_the_size_it_was_given() {
    let settings = CitySettings::default();
    assert_eq!(
        max_city_size(settings),
        8,
        "no urbanity, no forest: the size stands"
    );
}

#[test]
fn urbanity_may_not_grow_a_city_and_forest_may_not_shrink_it_below_the_size() {
    // The adjustment is capped at `-size + 2`, so a city never falls below two however wooded the world is.
    let wooded = CitySettings {
        forestosity: 100,
        ..CitySettings::default()
    };
    assert_eq!(
        max_city_size(wooded),
        8,
        "the floor holds it at the size it was given"
    );
    // With a ceiling of one, urbanity cannot grow it either.
    let flat = CitySettings {
        urbanity: 5,
        maximum_urbanity: 1,
        ..CitySettings::default()
    };
    assert_eq!(max_city_size(flat), 8);
}

#[test]
fn the_default_world_has_cities_and_the_count_is_the_coverage_formula() {
    let settings = CitySettings::default();
    // 32400 tiles, 1/16 of them urban, each city covering 216.75 tiles: 9.3425, so nine cities and a 34 percent
    // chance of a tenth.
    let without_the_remainder = cities_for(settings, 0.99);
    let with_the_remainder = cities_for(settings, 0.0);
    assert_eq!(without_the_remainder, 9);
    assert_eq!(
        with_the_remainder, 10,
        "the fraction is a chance, not a rounding"
    );
}

#[test]
fn more_spacing_means_fewer_cities() {
    let close = cities_for(
        CitySettings {
            spacing: 0,
            ..CitySettings::default()
        },
        0.0,
    );
    let far = cities_for(
        CitySettings {
            spacing: 8,
            ..CitySettings::default()
        },
        0.0,
    );
    assert!(close > far, "close {close} should outnumber far {far}");
    assert_eq!(far, 1, "the widest spacing still allows one");
}

#[test]
fn the_same_draw_gives_the_same_count() {
    let settings = CitySettings::default();
    assert_eq!(cities_for(settings, 0.4), cities_for(settings, 0.4));
    // The exact figure is 9.3425..., so a draw below that fraction earns the tenth city and one above it does not.
    assert_eq!(
        cities_for(settings, 0.3),
        10,
        "0.3 is inside the remainder's chance"
    );
    assert_eq!(cities_for(settings, 0.4), 9, "0.4 is outside it");
}
