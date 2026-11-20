//! **Roads and rails as the data states them.**
//!
//! Reference: `overmap_connection::load` and `overmap_connection::pick_subtype_for` (`overmap_connection.cpp`), with the data at `data/json/overmap/overmap_connections.json` and
//! the locations at `data/json/overmap/special_locations.json`. The pick is "the first subtype whose location is this tile's terrain", and the order of the list is the rule.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

/// The reference tree's overmap data directory.
fn data_directory() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/overmap")
}

/// Every connection and location the reference tree files.
fn the_reference_connections() -> (
    Vec<cdda_content::overmap_connection::Connection>,
    Vec<cdda_content::overmap_connection::Location>,
) {
    let mut values = Vec::new();
    for name in ["overmap_connections.json", "special_locations.json"] {
        let path = data_directory().join(name);
        let Ok(text) = std::fs::read(&path) else {
            continue;
        };
        if let Ok(value) = cdda_json::parse(&text) {
            values.push(value);
        }
    }
    cdda_content::overmap_connection::read_all(&values)
}

/// **The data's connections and locations are read whole.** The reference tree files five connections and thirty-five locations; a loader that silently dropped a subtype
/// would leave a road unable to cross a stream, which is the kind of hole that only shows up as a wall in the world.
#[test]
fn the_reference_tree_holds_the_connections_and_locations() {
    let (connections, locations) = the_reference_connections();
    if connections.is_empty() {
        eprintln!("the reference tree is not here, so there are no connections to read");
        return;
    }
    let mut names: Vec<&str> = connections
        .iter()
        .map(|connection| connection.id.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names.len(),
        5,
        "**the data files five connections**: {names:?}"
    );
    assert_eq!(
        locations.len(),
        35,
        "and thirty-five locations: {}",
        locations.len()
    );
    let road = connections
        .iter()
        .find(|connection| connection.id == "local_road")
        .expect("the local road");
    assert!(
        road.subtypes.len() > 5,
        "**the road has a subtype for each place it may cross**: {}",
        road.subtypes.len()
    );
    assert!(
        road.subtypes
            .iter()
            .any(|subtype| subtype.terrain == "bridge"),
        "and one of them is a bridge"
    );
}

/// **The first subtype whose location matches is the one laid**, and a bridge comes before a plain road because the data says so.
#[test]
fn a_connection_picks_the_first_subtype_that_fits_the_ground() {
    let (connections, locations) = the_reference_connections();
    if connections.is_empty() {
        eprintln!("the reference tree is not here, so there is nothing to pick from");
        return;
    }
    let roads: BTreeMap<String, cdda_content::overmap_connection::Location> = locations
        .into_iter()
        .map(|location| (location.id.clone(), location))
        .collect();
    let road = connections
        .iter()
        .find(|connection| connection.id == "local_road")
        .expect("the local road");
    // The `water` location names the **river** terrains, which is what a bridge is built over.
    let on_water = cdda_content::overmap_connection::pick_subtype(road, &roads, "river");
    assert_eq!(
        on_water.map(|subtype| subtype.terrain.as_str()),
        Some("bridge"),
        "**a road across water is a bridge**"
    );
    let on_field = cdda_content::overmap_connection::pick_subtype(road, &roads, "field");
    assert_eq!(
        on_field.map(|subtype| subtype.terrain.as_str()),
        Some("road"),
        "and a road across a field is a road"
    );
    // **A place the data does not name allows nothing**, so the connection has no subtype there - which is what stops a street at a ravine.
    let nowhere =
        cdda_content::overmap_connection::pick_subtype(road, &roads, "t_that_is_not_a_place");
    assert_eq!(
        nowhere.map(|subtype| subtype.terrain.as_str()),
        None,
        "**ground the road may not cross is not crossed**"
    );
    // And a tile that is **already** a road may be built on again: `oter->type_is( terrain )`, "can be built on similar terrains".
    let already = cdda_content::overmap_connection::pick_subtype(road, &roads, "road_nesw");
    assert_eq!(
        already.map(|subtype| subtype.terrain.as_str()),
        Some("road"),
        "**a road may be laid on a road**"
    );
}
