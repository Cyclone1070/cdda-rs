//! Putting cities on the overmap.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{
    CitySettings, Overmap, cities_for, place_cities, roll_city_position, roll_city_size,
};
use cdda_core_types::rng::Rolls;

#[test]
fn a_city_is_at_least_two_and_never_more_than_fifty_five() {
    let settings = CitySettings::default();
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..200 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        let size = roll_city_size(&mut rolls, settings);
        assert!((2..=55).contains(&size), "seed {seed} gave {size}");
    }
}

#[test]
fn the_sizes_spread_out_rather_than_all_being_the_same() {
    let settings = CitySettings::default();
    let mut sizes = std::collections::BTreeSet::new();
    for attempt in 0..200_u32 {
        // Spread seeds, for the reason `field_spawn` records: counting up samples one end of the distribution.
        let mut rolls = Rolls::new(attempt.wrapping_mul(2_654_435_761).wrapping_add(1));
        sizes.insert(roll_city_size(&mut rolls, settings));
    }
    assert!(sizes.len() > 5, "cities should differ in size: {sizes:?}");
    let smallest = sizes.iter().min().copied().unwrap_or(0);
    let largest = sizes.iter().max().copied().unwrap_or(0);
    assert!(smallest < 8, "some cities are small: {smallest}");
    assert!(largest >= 8, "and some are not: {largest}");
}

#[test]
fn a_city_is_placed_where_it_fits_without_crossing_the_edge() {
    let mut rolls = Rolls::new(1);
    for size in [2, 8, 20, 55] {
        for _ in 0..20 {
            let (x, y) = roll_city_position(&mut rolls, size, 180, 180);
            assert!(x >= size - 1 && x <= 180 - size, "x {x} for size {size}");
            assert!(y >= size - 1 && y <= 180 - size, "y {y} for size {size}");
        }
    }
}

#[test]
fn the_world_gets_the_number_of_cities_it_asked_for() {
    let mut overmap = Overmap::filled("field");
    let settings = CitySettings::default();
    let wanted = cities_for(settings, 0.0);
    let mut rolls = Rolls::new(7);
    let placed = place_cities(&mut overmap, settings, &mut rolls, wanted);
    assert_eq!(
        placed.len(),
        usize::try_from(wanted).unwrap_or(0),
        "all of them"
    );
    assert_eq!(
        overmap.count("road_nesw"),
        placed.len(),
        "each with a crossroads"
    );
    assert!(
        overmap.count("field") < 180 * 180,
        "and the map is no longer all field"
    );
}

#[test]
fn every_city_stands_on_its_own_square() {
    let mut overmap = Overmap::filled("field");
    let settings = CitySettings::default();
    let mut rolls = Rolls::new(11);
    let placed = place_cities(&mut overmap, settings, &mut rolls, 20);
    let mut squares: Vec<(i32, i32)> = placed.iter().map(|city| (city.x, city.y)).collect();
    let before = squares.len();
    squares.sort_unstable();
    squares.dedup();
    assert_eq!(squares.len(), before, "no two cities share a crossroads");
}

#[test]
fn the_same_seed_places_the_same_cities() {
    let place = |seed: u32| {
        let mut overmap = Overmap::filled("field");
        let mut rolls = Rolls::new(seed);
        place_cities(&mut overmap, CitySettings::default(), &mut rolls, 12)
    };
    assert_eq!(place(31337), place(31337));
}

#[test]
fn a_world_that_asks_for_no_cities_gets_none() {
    let mut overmap = Overmap::filled("field");
    let settings = CitySettings {
        size: 0,
        ..CitySettings::default()
    };
    let mut rolls = Rolls::new(5);
    assert!(place_cities(&mut overmap, settings, &mut rolls, 0).is_empty());
    assert_eq!(overmap.count("road_nesw"), 0);
}

#[test]
fn a_taken_square_is_skipped_rather_than_built_over() {
    let mut overmap = Overmap::filled("field");
    // Fill the whole map with something else: no city can find its default square, so the loop gives up.
    for y in 0..overmap.height() {
        for x in 0..overmap.width() {
            overmap.set_terrain(x, y, "forest");
        }
    }
    let mut rolls = Rolls::new(9);
    let placed = place_cities(&mut overmap, CitySettings::default(), &mut rolls, 5);
    assert!(placed.is_empty(), "nowhere to put one");
    assert_eq!(overmap.count("road_nesw"), 0, "and nothing was overwritten");
}
