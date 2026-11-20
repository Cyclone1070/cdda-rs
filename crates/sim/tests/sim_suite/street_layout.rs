//! **Laying out a street.**
//!
//! Reference: `overmap::lay_out_street` (`overmap.cpp:6628-6695`). A street is not drawn in one go: it is **walked**, one tile at a time, and the walk stops at the first thing it may
//! not cross - the map's edge, a river, ground the connection has no subtype for, or a place where three roads already crowd it. The path it answers is what `build_connection` then
//! writes terrain along (`:6697`), which is why the walk and the write are two rules rather than one.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

/// A road that may be laid on a field (as a road) and on a river (as a bridge), in the reference's order: the bridge comes first, so water is crossed by one.
fn a_road() -> cdda_content::overmap_connection::Connection {
    cdda_content::overmap_connection::Connection {
        id: "local_road".to_owned(),
        subtypes: vec![
            cdda_content::overmap_connection::Subtype {
                terrain: "bridge".to_owned(),
                locations: vec!["water".to_owned()],
                basic_cost: 120,
                flags: vec![],
            },
            cdda_content::overmap_connection::Subtype {
                terrain: "road".to_owned(),
                locations: vec!["field".to_owned()],
                basic_cost: 5,
                flags: vec![],
            },
        ],
    }
}

/// The places the road may cross, by id.
fn the_locations() -> BTreeMap<String, cdda_content::overmap_connection::Location> {
    [
        (
            "water".to_owned(),
            cdda_content::overmap_connection::Location {
                id: "water".to_owned(),
                terrains: vec!["river".to_owned()],
            },
        ),
        (
            "field".to_owned(),
            cdda_content::overmap_connection::Location {
                id: "field".to_owned(),
                terrains: vec!["field".to_owned()],
            },
        ),
    ]
    .into_iter()
    .collect()
}

/// A world whose overmap is empty except for the terrains a test names.
fn a_world(terrain: &[((i32, i32), &str)]) -> World {
    let mut world = World::new();
    for (at, id) in terrain {
        cdda_sim::overmap::set_terrain(&mut world, *at, id);
    }
    world
}

/// **A street is as long as it was asked for, when nothing is in the way.** Reference: `overmap.cpp:6640-6689` - the walk goes one tile at a time from the source, and
/// `straight_path( source, dir, actual_len )` answers the tiles it reached.
#[test]
fn a_street_walks_the_length_it_was_asked_for() {
    let world = a_world(&[]);
    let path = cdda_sim::street_layout::lay_out_street(
        &world,
        &a_road(),
        &the_locations(),
        (10, 10),
        cdda_sim::street_layout::Direction::East,
        5,
    );
    assert_eq!(
        path,
        vec![(10, 10), (11, 10), (12, 10), (13, 10), (14, 10)],
        "**five tiles east, starting where it was told**"
    );
}

/// **A street stops at the map's edge.** Reference: `overmap.cpp:6643-6645` - `if( !inbounds( pos, 1 ) ) break;`, one tile of margin.
#[test]
fn a_street_stops_at_the_edge_of_the_map() {
    let world = a_world(&[]);
    let path = cdda_sim::street_layout::lay_out_street(
        &world,
        &a_road(),
        &the_locations(),
        (177, 90),
        cdda_sim::street_layout::Direction::East,
        5,
    );
    assert_eq!(
        path,
        vec![(177, 90), (178, 90)],
        "**the walk stops one short of the edge**: {path:?}"
    );
}

/// **A street stops before ground it may not cross.** Reference: `overmap.cpp:6647-6652` - a tile with no subtype for the connection stops the walk, and the tile itself is
/// not part of the path.
#[test]
fn a_street_stops_before_ground_it_may_not_cross() {
    let world = a_world(&[((13, 10), "forest")]);
    let path = cdda_sim::street_layout::lay_out_street(
        &world,
        &a_road(),
        &the_locations(),
        (10, 10),
        cdda_sim::street_layout::Direction::East,
        8,
    );
    assert_eq!(
        path,
        vec![(10, 10), (11, 10), (12, 10)],
        "**the forest is not crossed, and the street ends before it**: {path:?}"
    );
}

/// **A street stops at a river even when the connection could bridge it.** Reference: `overmap.cpp:6649-6650` - `ter_id->is_river()` breaks the walk **before** the connection is
/// asked, because a city street is not a road that crosses water: the bridge subtype exists for the inter-city roads that do (`build_connection` over a pathfound route).
#[test]
fn a_street_stops_at_a_river() {
    let world = a_world(&[((12, 10), "river")]);
    let path = cdda_sim::street_layout::lay_out_street(
        &world,
        &a_road(),
        &the_locations(),
        (10, 10),
        cdda_sim::street_layout::Direction::East,
        5,
    );
    assert_eq!(
        path,
        vec![(10, 10), (11, 10)],
        "**the river is not crossed by a street**: {path:?}"
    );
}

/// **A street stops when it reaches another road.** Reference: `overmap.cpp:6687-6689` - "Stop here" once the walk is past its first tile and lands on the connection's own
/// terrain, so two streets meet rather than run alongside each other.
#[test]
fn a_street_stops_when_it_reaches_a_road() {
    let world = a_world(&[((13, 10), "road_nesw")]);
    let path = cdda_sim::street_layout::lay_out_street(
        &world,
        &a_road(),
        &the_locations(),
        (10, 10),
        cdda_sim::street_layout::Direction::East,
        8,
    );
    assert_eq!(
        path,
        vec![(10, 10), (11, 10), (12, 10), (13, 10)],
        "**the existing road is the last tile walked**: {path:?}"
    );
}

/// **A street does not run right next to another.** Reference: `overmap.cpp:6654-6683` - three road neighbours off the street's own line stop the walk, "stop roads from
/// running right next to each other". The check runs **before** a tile is taken, so a crowd around the source stops the street at nothing - and `build_city_street` bails on a
/// path of one node or fewer (`:6246-6248`).
#[test]
fn a_street_turns_away_from_a_crowd_of_roads() {
    let crowd = [
        ((10, 9), "road_nesw"),
        ((11, 9), "road_nesw"),
        ((11, 11), "road_nesw"),
    ];
    let world = a_world(&crowd);
    let path = cdda_sim::street_layout::lay_out_street(
        &world,
        &a_road(),
        &the_locations(),
        (10, 10),
        cdda_sim::street_layout::Direction::East,
        8,
    );
    assert!(
        path.is_empty(),
        "**three roads beside the source stop the street before it starts**: {path:?}"
    );
}
