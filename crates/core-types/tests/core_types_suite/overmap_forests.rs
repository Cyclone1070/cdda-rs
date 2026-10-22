//! Woodland placed from the noise field, and what it refuses to paint over.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::noise::forest_noise;
use cdda_core_types::overmap::{
    CitySettings, ForestSettings, Overmap, grow_town, place_forests, render,
};
use cdda_core_types::rng::Rolls;

#[test]
fn a_tile_becomes_forest_exactly_where_the_field_passes_the_threshold() {
    let mut overmap = Overmap::filled("field");
    let planted = place_forests(&mut overmap, ForestSettings::default(), 31337, 0.0);
    assert!(planted > 0, "some of the map is woodland");
    for y in 0..overmap.height() {
        for x in 0..overmap.width() {
            let noise = forest_noise(x, y, 31337);
            let terrain = overmap.terrain(x, y).expect("a terrain");
            // The thresholds the test asks about are the ones it passed in, not a second copy of the numbers: the
            // defaults are the reference's header values, and the regional settings file overrides them with lower
            // ones, so a test that hard-coded either would be testing the wrong thing after a settings change.
            let settings = ForestSettings::default();
            let expected = if noise > settings.threshold_thick {
                "forest_thick"
            } else if noise > settings.threshold {
                "forest"
            } else {
                "field"
            };
            assert_eq!(terrain, expected, "at ({x},{y}) with noise {noise}");
        }
    }
}

#[test]
fn the_size_adjustment_moves_the_whole_field() {
    let mut small = Overmap::filled("field");
    let mut large = Overmap::filled("field");
    let few = place_forests(&mut small, ForestSettings::default(), 7, -0.2);
    let many = place_forests(&mut large, ForestSettings::default(), 7, 0.2);
    assert!(
        few < many,
        "a bigger forest region plants more: {few} against {many}"
    );
}

#[test]
fn woodland_never_paints_over_anything_that_is_not_default_ground() {
    let mut overmap = Overmap::filled("field");
    overmap.set_terrain(10, 10, "lake_surface");
    overmap.set_terrain(11, 10, "road");
    place_forests(&mut overmap, ForestSettings::default(), 31337, 1.0);
    assert_eq!(
        overmap.terrain(10, 10),
        Some("lake_surface"),
        "a lake stays a lake"
    );
    assert_eq!(
        overmap.terrain(11, 10),
        Some("road"),
        "and a road stays a road"
    );
}

#[test]
fn the_same_seed_plants_the_same_woodland() {
    let mut first = Overmap::filled("field");
    let mut second = Overmap::filled("field");
    place_forests(&mut first, ForestSettings::default(), 99, 0.0);
    place_forests(&mut second, ForestSettings::default(), 99, 0.0);
    for y in 0..first.height() {
        for x in 0..first.width() {
            assert_eq!(first.terrain(x, y), second.terrain(x, y), "at ({x},{y})");
        }
    }
    let mut other = Overmap::filled("field");
    place_forests(&mut other, ForestSettings::default(), 100, 0.0);
    let differs = (0..first.height())
        .flat_map(|y| (0..first.width()).map(move |x| (x, y)))
        .any(|(x, y)| first.terrain(x, y) != other.terrain(x, y));
    assert!(differs, "a different seed is a different world");
}

#[test]
fn woodland_is_drawn_as_trees() {
    let mut overmap = Overmap::filled("field");
    place_forests(&mut overmap, ForestSettings::default(), 31337, 0.0);
    let picture = render(&overmap, (0, 0), 180, 180);
    assert!(picture.contains('F'), "forest is drawn as trees");
}

#[test]
fn a_town_grown_after_the_woods_sits_inside_them() {
    // The order matters and is the reference's: forests are placed first, so growing a town afterwards cuts streets
    // through woodland rather than the woodland appearing around the town.
    let mut overmap = Overmap::filled("field");
    place_forests(&mut overmap, ForestSettings::default(), 31337, 0.0);
    let mut rolls = Rolls::new(31337);
    grow_town(&mut overmap, CitySettings::default(), 3, &mut rolls);
    let picture = render(&overmap, (0, 0), 180, 180);
    assert!(picture.contains('F'), "there is woodland");
    assert!(
        picture.contains('#') || picture.contains('│'),
        "and streets through it"
    );
}
