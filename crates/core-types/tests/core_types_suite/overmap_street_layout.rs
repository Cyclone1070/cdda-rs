//! Laying a street out across a map.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{Direction, Overmap, StreetView, lay_out_street};

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

fn field() -> Overmap {
    Overmap::filled("field")
}

#[test]
fn a_street_over_open_ground_runs_as_far_as_it_was_asked() {
    let map = field();
    // Six, not five: open ground means the tile past the end could carry a road too, and the reference makes the
    // street one longer rather than leaving a stub end. My first expectation said five and the code was right.
    let path = lay_out_street(&map, (90, 90), Direction::East, 5, &view(&road, &servable));
    assert_eq!(
        path.len(),
        6,
        "five asked for, one more because the ground allows it"
    );
    assert_eq!(path.first().copied(), Some((90, 90)));
    assert_eq!(path.last().copied(), Some((95, 90)));
}

#[test]
fn a_street_stops_where_the_map_runs_out_rather_than_running_to_the_edge() {
    let map = field();
    // Starting two tiles from the edge, heading for it: the margin stops it before the last tile.
    let path = lay_out_street(
        &map,
        (177, 90),
        Direction::East,
        20,
        &view(&road, &servable),
    );
    assert_eq!(
        path.last().copied(),
        Some((178, 90)),
        "the margin of one tile holds"
    );
}

#[test]
fn a_street_stops_at_water() {
    let mut map = field();
    map.set_terrain(93, 90, "river");
    let path = lay_out_street(&map, (90, 90), Direction::East, 20, &view(&road, &servable));
    assert_eq!(
        path.last().copied(),
        Some((92, 90)),
        "up to the bank, not across it"
    );
}

#[test]
fn a_street_is_made_one_longer_when_the_tile_past_its_end_could_carry_a_road() {
    let map = field();
    let path = lay_out_street(&map, (90, 90), Direction::East, 4, &view(&road, &servable));
    assert_eq!(path.len(), 5, "the extra tile is taken");
    assert_eq!(path.last().copied(), Some((94, 90)));
}

#[test]
fn a_street_that_reaches_an_existing_road_stops_on_it() {
    let mut map = field();
    map.set_terrain(95, 90, "road_nesw");
    // The requested length already covers the road, so the walk stops the moment it lands on it.
    let path = lay_out_street(&map, (90, 90), Direction::East, 10, &view(&road, &servable));
    assert_eq!(path.last().copied(), Some((95, 90)), "it joins the road");
    assert_eq!(path.len(), 6);
}

#[test]
fn a_street_is_stopped_before_it_runs_alongside_another() {
    let mut map = field();
    // Three road tiles in the row beside the street: the corridor check fires on the first step.
    map.set_terrain(90, 89, "road_ew");
    map.set_terrain(91, 89, "road_ew");
    map.set_terrain(92, 89, "road_ew");
    let path = lay_out_street(&map, (90, 90), Direction::East, 10, &view(&road, &servable));
    assert_eq!(path.len(), 1, "one tile and no more");
    assert_eq!(path.first().copied(), Some((90, 90)));
}

#[test]
fn two_roads_side_by_side_are_allowed_where_a_third_would_not_be() {
    let mut map = field();
    map.set_terrain(90, 89, "road_ew");
    map.set_terrain(91, 89, "road_ew");
    let path = lay_out_street(&map, (90, 90), Direction::East, 4, &view(&road, &servable));
    assert!(
        path.len() > 1,
        "two neighbours do not stop it: {}",
        path.len()
    );
}

#[test]
fn the_same_map_lays_out_the_same_street() {
    let map = field();
    let once = lay_out_street(&map, (10, 10), Direction::South, 7, &view(&road, &servable));
    let twice = lay_out_street(&map, (10, 10), Direction::South, 7, &view(&road, &servable));
    assert_eq!(once, twice);
}
