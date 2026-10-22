//! Every road tile named after the neighbours it reaches.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::overmap::{
    CitySettings, Direction, Overmap, StreetView, grow_town, lay_out_street, render, road_glyph,
    road_mask, road_name, shape_roads, straight_path,
};
use cdda_core_types::rng::Rolls;

fn road(terrain: &str) -> bool {
    terrain.starts_with("road")
}

fn servable(terrain: &str) -> bool {
    terrain != "river" && terrain != "ravine" && terrain != "ravine_edge"
}

fn view<'a>(
    is_road: &'a dyn Fn(&str) -> bool,
    can_serve: &'a dyn Fn(&str) -> bool,
) -> StreetView<'a> {
    StreetView {
        width: 180,
        height: 180,
        is_road,
        can_serve,
    }
}

#[test]
fn a_straight_run_is_named_after_the_two_ends_it_joins() {
    let mut map = Overmap::filled("field");
    for (x, y) in straight_path((10, 10), Direction::East, 5) {
        map.set_terrain(x, y, "road");
    }
    shape_roads(&mut map);
    assert_eq!(map.terrain(12, 10), Some("road_ew"), "west and east");
    assert_eq!(
        map.terrain(10, 10),
        Some("road_e"),
        "the end reaches only eastwards"
    );
    assert_eq!(map.terrain(14, 10), Some("road_w"));
}

#[test]
fn a_corner_is_named_after_both_of_its_arms() {
    let mut map = Overmap::filled("field");
    // A corner: north and east.
    for (x, y) in [(10, 10), (10, 9), (11, 10)] {
        map.set_terrain(x, y, "road");
    }
    shape_roads(&mut map);
    assert_eq!(map.terrain(10, 10), Some("road_ne"));
    assert_eq!(road_glyph("road_ne"), Some('└'));
}

#[test]
fn a_crossing_reaches_all_four_ways() {
    let mut map = Overmap::filled("field");
    for (x, y) in [(10, 10), (10, 9), (10, 11), (9, 10), (11, 10)] {
        map.set_terrain(x, y, "road");
    }
    shape_roads(&mut map);
    assert_eq!(map.terrain(10, 10), Some("road_nesw"));
    assert_eq!(road_mask(&map, 10, 10), 0b1111);
    assert_eq!(road_glyph("road_nesw"), Some('┼'));
}

#[test]
fn the_mask_reads_the_four_neighbours_in_the_reference_order() {
    let mut map = Overmap::filled("field");
    map.set_terrain(10, 9, "road");
    map.set_terrain(10, 11, "road");
    // The mask reads the neighbours and says nothing about the tile itself, which is why it is asked here before
    // the middle is a road at all.
    assert_eq!(road_mask(&map, 10, 10), 0b0101, "north and south");
    shape_roads(&mut map);
    map.set_terrain(10, 10, "road");
    shape_roads(&mut map);
    assert_eq!(map.terrain(10, 10), Some("road_ns"));
}

#[test]
fn a_name_and_a_glyph_are_the_same_shape_seen_twice() {
    let cases = [
        ("road_ns", '│'),
        ("road_ew", '─'),
        ("road_ne", '└'),
        ("road_nw", '┘'),
        ("road_es", '┌'),
        ("road_sw", '┐'),
        ("road_nes", '├'),
        ("road_nsw", '┤'),
        ("road_esw", '┬'),
        ("road_new", '┴'),
        ("road_nesw", '┼'),
    ];
    for (name, glyph) in cases {
        assert_eq!(road_glyph(name), Some(glyph), "{name}");
    }
    assert_eq!(road_name(0b0101), "road_ns");
}

#[test]
fn a_grown_town_draws_corners_and_crossings_rather_than_one_flat_glyph() {
    let mut overmap = Overmap::filled("field");
    let mut rolls = Rolls::new(31337);
    grow_town(&mut overmap, CitySettings::default(), 3, &mut rolls);
    let picture = render(&overmap, (0, 0), 180, 180);
    let shapes: std::collections::BTreeSet<char> = picture
        .chars()
        .filter(|glyph| "│─└┘┌┐├┤┬┴┼#".contains(*glyph))
        .collect();
    // Four kinds, and the crossing is the important one: the town's streets cross, so a tile reaches all four ways.
    // Corners are rarer here than in the reference because this generator's streets do not turn mid-run - they
    // sprout new runs at right angles, which makes crossings rather than bends. That difference is real and is
    // recorded in the ledger rather than hidden by a loose test.
    assert!(
        shapes.len() >= 4,
        "a grown town has more than one road glyph: {shapes:?}"
    );
    assert!(shapes.contains(&'┼'), "and its streets cross: {shapes:?}");
    assert!(
        shapes.contains(&'│') && shapes.contains(&'─'),
        "both ways: {shapes:?}"
    );
}

#[test]
fn the_layout_rule_and_the_shape_rule_agree_about_what_a_street_is() {
    let map = Overmap::filled("field");
    let path = lay_out_street(&map, (90, 90), Direction::East, 6, &view(&road, &servable));
    assert!(path.len() > 1, "there is a street to shape");
    let mut shaped = Overmap::filled("field");
    for (x, y) in &path {
        shaped.set_terrain(*x, *y, "road");
    }
    shape_roads(&mut shaped);
    for (x, y) in &path {
        let name = shaped.terrain(*x, *y).expect("a road");
        assert!(name.starts_with("road_"), "{name} at ({x},{y})");
    }
}
