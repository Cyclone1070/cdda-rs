//! **Roads and rails as connections.**
//!
//! Reference: `overmap_connection` (`src/overmap_connection.h`, data at `data/json/overmap/overmap_connections.json`) and `overmap_location`
//! (`data/json/overmap/special_locations.json`). A connection is a **list of subtypes** - each a terrain to lay plus the places it may be laid on - in the data's own order, and
//! the rule that picks one is "the first subtype that may be laid here". That order is the whole meaning: a road crosses a stream as a bridge because the bridge subtype comes
//! before the plain road one, and a port that reordered the list would build roads on water.
//!
//! **What is ported** is the data and the pick. What is not is the pathfinding (`overmap::lay_out_street`, `:6628`) and the width of a street - those belong to the street
//! builder, which is the layer above this one.

use cdda_json::Value;
use std::collections::BTreeMap;

/// One way of laying a connection through a place: the terrain and where it may go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtype {
    /// The connection terrain id (`"road"`, `"bridge"`), which the reference later turns into a directional overmap terrain id by its line.
    pub terrain: String,
    /// The overmap locations it may be laid on, by id.
    pub locations: Vec<String>,
    /// What crossing costs, in the reference's own units - read because the street builder needs it, even though this module does not.
    pub basic_cost: i32,
    /// The data's flags (`ORTHOGONAL`, `PERPENDICULAR_CROSSING`), kept as written.
    pub flags: Vec<String>,
}

/// A connection: the subtypes that may lay it, in the data's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// The id the data uses (`"local_road"`, `"local_rail"`).
    pub id: String,
    /// The subtypes, in order - **the order is the rule**.
    pub subtypes: Vec<Subtype>,
}

/// An overmap location: the terrains that count as being there (`overmap_location`, `data/json/overmap/special_locations.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The id the data uses (`"field"`, `"water"`).
    pub id: String,
    /// The overmap terrain ids that are this place.
    pub terrains: Vec<String>,
}

/// A member of an object value, or nothing when the value is not an object or the member is not there.
fn member<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map.get(key),
        _ => None,
    }
}

/// Reads every connection and location out of a file's objects.
///
/// Reference: `overmap_connection::load` (`overmap_connection.cpp`) and `overmap_location::load`. A file may hold several objects, and objects of other kinds are ignored, which
/// is what lets one walk of the tree feed both loaders.
#[must_use]
pub fn read_all(values: &[Value]) -> (Vec<Connection>, Vec<Location>) {
    let mut connections = Vec::new();
    let mut locations = Vec::new();
    for value in values {
        let objects = if matches!(value, Value::Array(_)) {
            match value {
                Value::Array(items) => items.clone(),
                _ => Vec::new(),
            }
        } else {
            vec![value.clone()]
        };
        for object in objects {
            match member(&object, "type") {
                Some(Value::Str(kind)) if kind == "overmap_connection" => {
                    if let Some(connection) = read_connection(&object) {
                        connections.push(connection);
                    }
                }
                Some(Value::Str(kind)) if kind == "overmap_location" => {
                    if let Some(location) = read_location(&object) {
                        locations.push(location);
                    }
                }
                _ => {}
            }
        }
    }
    (connections, locations)
}

/// One connection, or nothing when the object does not name one.
fn read_connection(object: &Value) -> Option<Connection> {
    let id = match member(object, "id") {
        Some(Value::Str(id)) => id.clone(),
        _ => return None,
    };
    let mut subtypes = Vec::new();
    if let Some(Value::Array(entries)) = member(object, "subtypes") {
        for entry in entries {
            let Some(terrain) = member(entry, "terrain").and_then(|terrain| match terrain {
                Value::Str(id) => Some(id.clone()),
                _ => None,
            }) else {
                continue;
            };
            let mut locations = Vec::new();
            if let Some(Value::Array(names)) = member(entry, "locations") {
                for name in names {
                    if let Value::Str(name) = name {
                        locations.push(name.clone());
                    }
                }
            }
            let basic_cost = member(entry, "basic_cost")
                .and_then(|cost| match cost {
                    Value::Int(cost) => i32::try_from(*cost).ok(),
                    _ => None,
                })
                .unwrap_or(0);
            let mut flags = Vec::new();
            if let Some(Value::Array(names)) = member(entry, "flags") {
                for name in names {
                    if let Value::Str(name) = name {
                        flags.push(name.clone());
                    }
                }
            }
            subtypes.push(Subtype {
                terrain,
                locations,
                basic_cost,
                flags,
            });
        }
    }
    Some(Connection { id, subtypes })
}

/// One location, or nothing when the object does not name one.
fn read_location(object: &Value) -> Option<Location> {
    let id = match member(object, "id") {
        Some(Value::Str(id)) => id.clone(),
        _ => return None,
    };
    let mut terrains = Vec::new();
    if let Some(Value::Array(names)) = member(object, "terrains") {
        for name in names {
            if let Value::Str(name) = name {
                terrains.push(name.clone());
            }
        }
    }
    Some(Location { id, terrains })
}

/// The subtype a connection lays on a tile, by the reference's rule: **the first one that allows this ground**
/// (`overmap_connection::pick_subtype_for` and `subtype::allows_terrain`, `overmap_connection.cpp:38-92`).
///
/// A subtype allows a tile in **two** ways, and both are the reference's: the tile is already of the subtype's own terrain (`oter->type_is( terrain )` - "can be built on similar
/// terrains"), or one of the subtype's locations counts this tile as that place. An empty `locations` list is therefore **not** a fallback that matches everything: a bridge
/// precedes the plain road because water is a place the road may cross as a bridge, and the manhole subtype - which names no place - is reached only on a tile that is already
/// a manhole.
#[must_use]
pub fn pick_subtype<'a>(
    connection: &'a Connection,
    locations: &BTreeMap<String, Location>,
    tile_terrain: &str,
) -> Option<&'a Subtype> {
    connection.subtypes.iter().find(|subtype| {
        // `oter->type_is( terrain )`: the tile is already this kind of thing. The port compares the id and, because overmap terrain ids end in a directional suffix, the id
        // with that suffix still to come - "road_nesw" is of type "road".
        terrain_is(tile_terrain, &subtype.terrain)
            || subtype.locations.iter().any(|name| {
                locations.get(name).is_some_and(|location| {
                    location
                        .terrains
                        .iter()
                        .any(|terrain| terrain_is(tile_terrain, terrain))
                })
            })
    })
}

/// Whether a tile's terrain is of a named terrain type: the id itself, or the id with a directional suffix after it (`road_nesw` is of type `road`).
///
/// The reference asks `oter_t::type_is`, which compares the **type** an overmap terrain declares rather than its id. The port has not loaded that declaration, so it reads the
/// id's shape instead - the same answer for every directional terrain in the data, and a named approximation for the rest.
#[must_use]
pub fn terrain_is(tile_terrain: &str, terrain: &str) -> bool {
    tile_terrain == terrain
        || tile_terrain
            .strip_prefix(terrain)
            .is_some_and(|rest| rest.starts_with('_'))
}
