//! Growing a town onto a map, and drawing it.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::overmap::{CitySettings, Overmap, grow_town, render};
use cdda_core_types::rng::Rolls;

fn grown(seed: u32, wanted: i32) -> Overmap {
    let mut overmap = Overmap::filled("field");
    let mut rolls = Rolls::new(seed);
    grow_town(&mut overmap, CitySettings::default(), wanted, &mut rolls);
    overmap
}

#[test]
fn a_grown_world_has_roads_on_it() {
    let overmap = grown(7, 3);
    let roads =
        overmap.count("road") + overmap.count("road_nesw") + overmap.count("road_nesw_manhole");
    assert!(
        roads > 20,
        "three cities lay more than a few tiles: {roads}"
    );
    assert!(
        overmap.count("field") < 180 * 180,
        "and the map is no longer all field"
    );
}

#[test]
fn a_world_that_was_asked_for_no_cities_has_no_roads() {
    let mut overmap = Overmap::filled("field");
    let mut rolls = Rolls::new(7);
    grow_town(
        &mut overmap,
        CitySettings {
            size: 0,
            ..CitySettings::default()
        },
        0,
        &mut rolls,
    );
    assert_eq!(overmap.count("road"), 0);
    assert_eq!(overmap.count("field"), 180 * 180);
}

#[test]
fn the_same_seed_grows_the_same_world() {
    let picture = |seed: u32| render(&grown(seed, 3), (60, 60), 60, 30);
    assert_eq!(picture(31337), picture(31337));
}

#[test]
fn different_seeds_grow_different_worlds() {
    let picture = |seed: u32| render(&grown(seed, 3), (60, 60), 60, 30);
    assert_ne!(picture(11), picture(12), "a seed has to mean something");
}

#[test]
fn the_picture_is_the_size_it_was_asked_for() {
    let overmap = grown(7, 2);
    let picture = render(&overmap, (60, 60), 40, 12);
    assert_eq!(picture.lines().count(), 12);
    assert!(picture.lines().all(|line| line.chars().count() == 40));
}

#[test]
fn a_road_shows_up_in_the_picture() {
    let overmap = grown(7, 3);
    let picture = render(&overmap, (0, 0), 180, 180);
    assert!(picture.contains('#'), "a grown world shows its streets");
}

/// Writes the picture out, so a person can look at a generated world without reading code.
#[test]
fn the_picture_of_a_world_can_be_written_out() {
    let overmap = grown(31337, 3);
    // The whole map: a window would be a guess about where the cities landed, and the picture is for looking at.
    let picture = render(&overmap, (0, 0), 180, 180);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/town-seed-31337.txt");
    std::fs::create_dir_all(path.parent().expect("a directory")).expect("the corpus directory");
    std::fs::write(&path, &picture).expect("the picture is written");
    assert!(picture.contains('#'), "and it is a picture of something");
}

#[test]
fn the_junctions_of_a_town_are_visible_in_the_picture() {
    let overmap = grown(7, 4);
    let picture = render(&overmap, (0, 0), 180, 180);
    // Junctions are crossings in the reference's own vocabulary now, not a separate symbol.
    let mut seen: std::collections::BTreeMap<char, usize> = std::collections::BTreeMap::new();
    for glyph in picture.chars().filter(|glyph| *glyph != '\n') {
        *seen.entry(glyph).or_insert(0) += 1;
    }
    assert!(
        picture.contains('┼')
            || picture.contains('├')
            || picture.contains('┤')
            || picture.contains('┬'),
        "junctions are drawn differently from plain road: {seen:?}"
    );
}
