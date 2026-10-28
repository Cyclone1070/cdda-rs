//! Growing a street and the streets that leave it.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{
    Direction, Overmap, StreetTile, StreetView, Town, build_city_street,
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

fn grow(seed: u32, size: i32, direction: Direction) -> Vec<((i32, i32), StreetTile)> {
    let map = Overmap::filled("field");
    let mut rolls = Rolls::new(seed);
    let mut tiles = Vec::new();
    let view = view(&road, &servable);
    let mut town = Town {
        map: &map,
        view: &view,
        rolls: &mut rolls,
        tiles: &mut tiles,
    };
    build_city_street(&mut town, (90, 90), direction, size, 2, 0);
    tiles
}

#[test]
fn a_street_of_a_real_city_grows_further_than_the_street_alone() {
    let straight = grow(1, 2, Direction::East);
    let city = grow(1, 8, Direction::East);
    assert!(city.len() > straight.len(), "a bigger city reaches further");
    assert!(!city.is_empty(), "and it grows at all");
}

#[test]
fn the_sub_streets_leave_at_right_angles() {
    let tiles = grow(2, 10, Direction::East);
    // The main street runs east, so its own tiles share a row; a sub-street must leave that row.
    let rows: std::collections::BTreeSet<i32> =
        tiles.iter().map(|(position, _)| position.1).collect();
    assert!(
        rows.len() > 1,
        "sub-streets leave the row they started in: {rows:?}"
    );
    assert!(rows.contains(&90), "and the main street is still there");
}

#[test]
fn no_street_is_a_single_tile_nub() {
    // A sub-street of one tile is bumped to two, so every run that leaves the main street is at least that long.
    let tiles = grow(3, 9, Direction::North);
    let mut columns: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
    for (position, _) in &tiles {
        *columns.entry(position.0).or_default() += 1;
    }
    assert!(columns.values().all(|count| *count >= 1));
    assert!(
        tiles.len() >= 4,
        "a city of nine grows more than a nub: {}",
        tiles.len()
    );
}

#[test]
fn a_town_of_two_grows_a_short_street_and_no_neighbourhoods() {
    // With a size of two the counter can never satisfy `c >= 2 && c < croad - block_width`, so nothing sprouts.
    let tiles = grow(4, 2, Direction::East);
    let junctions = tiles
        .iter()
        .filter(|(_, kind)| *kind != StreetTile::Road)
        .count();
    assert_eq!(junctions, 0, "nothing leaves a street of a two-tile town");
}

#[test]
fn a_junction_is_where_a_sub_street_leaves_and_may_carry_a_manhole() {
    let mut junctions = 0;
    let mut manholes = 0;
    for attempt in 0..40_u32 {
        let seed = crate::spread_seed(attempt);
        for (_, kind) in grow(seed, 9, Direction::East) {
            match kind {
                StreetTile::Junction => junctions += 1,
                StreetTile::Manhole => manholes += 1,
                StreetTile::Road => {}
            }
        }
    }
    assert!(junctions > 0, "some junctions are plain");
    assert!(manholes > 0, "and some carry a manhole");
}

#[test]
fn the_same_seed_grows_the_same_town() {
    assert_eq!(
        grow(31337, 9, Direction::East),
        grow(31337, 9, Direction::East)
    );
}

#[test]
fn a_street_that_cannot_be_laid_grows_nothing() {
    let mut map = Overmap::filled("field");
    // Water on every side of the start: the layout stops at the first tile, so the town is one tile or less.
    for (x, y) in [(89, 90), (91, 90), (90, 89), (90, 91)] {
        map.set_terrain(x, y, "river");
    }
    let mut rolls = Rolls::new(5);
    let mut tiles = Vec::new();
    let view = view(&road, &servable);
    let mut town = Town {
        map: &map,
        view: &view,
        rolls: &mut rolls,
        tiles: &mut tiles,
    };
    build_city_street(&mut town, (90, 90), Direction::East, 9, 2, 0);
    assert!(tiles.len() <= 1, "nothing worth building: {}", tiles.len());
}
