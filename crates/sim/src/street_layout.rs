//! **Laying out a street.**
//!
//! Reference: `overmap::lay_out_street` (`overmap.cpp:6628-6695`). A street is not drawn in one go: it is **walked**, one tile at a time, and the walk stops at the first thing it
//! may not cross - the map's edge, a river or ravine, ground the connection has no subtype for, a place where three roads already crowd it, or another road of its own kind.
//! The path it answers is what `build_connection` then writes terrain along (`:6697`), which is why the walk and the write are two rules rather than one.
//!
//! **What is ported** is the walk. **What is not** is what a walk is *for*: `build_city_street` (`:6232`), which sprouts side streets and hangs buildings off them, and
//! `build_connection`, which turns a path into directional terrain ids (`road_ns`, `road_ew`, and the corners). Those are named here rather than half-done.

use bevy_ecs::prelude::*;
use cdda_content::overmap_connection::{Connection, Location, pick_subtype};
use std::collections::BTreeMap;

/// The four ways a street can go in an overmap, which is what `om_direction`'s cardinals are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Towards smaller `y`.
    North,
    /// Towards larger `x`.
    East,
    /// Towards larger `y`.
    South,
    /// Towards smaller `x`.
    West,
}

impl Direction {
    /// One step this way, `steps` times: `om_direction::displace( dir, steps )`.
    #[must_use]
    pub fn displace(self, steps: i32) -> (i32, i32) {
        match self {
            Self::North => (0, -steps),
            Self::East => (steps, 0),
            Self::South => (0, steps),
            Self::West => (-steps, 0),
        }
    }

    /// The way back: `om_direction::opposite`.
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

/// The side of an overmap, in overmap tiles (`map_scale_constants.h:37`).
const OVERMAP_SIDE: i32 = 180;

/// How far from the edge a street may come: `inbounds( pos, 1 )` leaves one tile of margin (`overmap.cpp:6643`).
const EDGE_MARGIN: i32 = 1;

/// How many roads beside a street tile are too many: `if( collisions >= 3 )` (`overmap.cpp:6678`).
const TOO_CROWDED: i32 = 3;

/// The ground a tile has when nothing has set it: the regional settings' default overmap terrain for the surface (`region_settings.json`'s `default_oter`).
const DEFAULT_GROUND: &str = "field";

/// The terrains a street never crosses, whatever the connection says (`overmap.cpp:6649-6650`): a river, a ravine, and the edge of a ravine.
const IMPASSABLE: [&str; 6] = [
    "river",
    "river_center",
    "ravine",
    "ravine_edge",
    "ravine_center",
    "ravine_end",
];

/// Whether a tile is inside the overmap with the street's margin (`inbounds( pos, EDGE_MARGIN )`).
fn in_bounds(at: (i32, i32)) -> bool {
    (EDGE_MARGIN..OVERMAP_SIDE - EDGE_MARGIN).contains(&at.0)
        && (EDGE_MARGIN..OVERMAP_SIDE - EDGE_MARGIN).contains(&at.1)
}

/// Walks a street from a tile in a direction, answering the tiles it reached - the source included, as `straight_path` does.
///
/// Reference: `overmap::lay_out_street` (`overmap.cpp:6628-6695`). The stops, in the reference's order:
///
/// - the map's edge, with a tile of margin;
/// - a river or a ravine, or ground the connection has no subtype for;
/// - **three roads beside the tile**, which is what keeps streets from running alongside each other;
/// - and another road of the connection's own kind, once the walk is past its first tile - the two streets meet there.
///
/// `length` is the reference's `len`, which the caller derives from the city's size (`cs + 1`) and extends by one when the tile beyond the end is already a road
/// (`:6632-6636`).
#[must_use]
pub fn lay_out_street(
    world: &World,
    connection: &Connection,
    locations: &BTreeMap<String, Location>,
    from: (i32, i32),
    direction: Direction,
    length: i32,
) -> Vec<(i32, i32)> {
    // **A street that would end on a road reaches one tile further**, so it joins rather than stopping short (`:6632-6636`).
    let mut length = length;
    let beyond = (
        from.0 + direction.displace(length + 1).0,
        from.1 + direction.displace(length + 1).1,
    );
    if in_bounds(beyond)
        && let Some(terrain) = crate::overmap::terrain_at(world, beyond)
        && is_road(connection, locations, terrain)
    {
        length += 1;
    }
    let mut walked = 0;
    while walked < length {
        let step = direction.displace(walked);
        let at = (from.0 + step.0, from.1 + step.1);
        if !in_bounds(at) {
            break;
        }
        let terrain = crate::overmap::terrain_at(world, at);
        if let Some(terrain) = terrain {
            if IMPASSABLE.contains(&terrain) {
                break;
            }
            if pick_subtype(connection, locations, terrain).is_none() {
                break;
            }
        }
        if crowded(world, connection, locations, at, direction) {
            break;
        }
        walked += 1;
        // **Reaching a road of the same connection ends the walk on it** (`:6687-6689`), but not at the very first tile: the source may itself be a road.
        if walked > 1 && terrain.is_some_and(|terrain| is_road(connection, locations, terrain)) {
            break;
        }
    }
    let mut path = Vec::with_capacity(usize::try_from(walked).unwrap_or(0));
    for step in 0..walked {
        let offset = direction.displace(step);
        path.push((from.0 + offset.0, from.1 + offset.1));
    }
    path
}

/// Whether a terrain is one the connection could lay, which is what `connection.has( ter )` asks.
fn is_road(connection: &Connection, locations: &BTreeMap<String, Location>, terrain: &str) -> bool {
    connection
        .subtypes
        .iter()
        .any(|subtype| cdda_content::overmap_connection::terrain_is(terrain, &subtype.terrain))
        && pick_subtype(connection, locations, terrain).is_some()
}

/// Whether three roads stand beside a tile, off the street's own line (`overmap.cpp:6654-6683`).
fn crowded(
    world: &World,
    connection: &Connection,
    locations: &BTreeMap<String, Location>,
    at: (i32, i32),
    direction: Direction,
) -> bool {
    let ahead = direction.displace(1);
    let behind = direction.opposite().displace(1);
    let mut collisions = 0;
    for i in -1..=1 {
        for j in -1..=1 {
            let check = (at.0 + i, at.1 + j);
            let is_ahead = check == (at.0 + ahead.0, at.1 + ahead.1);
            let is_behind = check == (at.0 + behind.0, at.1 + behind.1);
            if check == at || is_ahead || is_behind {
                continue;
            }
            if crate::overmap::terrain_at(world, check)
                .is_some_and(|terrain| is_road(connection, locations, terrain))
            {
                collisions += 1;
            }
        }
        // The reference checks the count after every column of the 3x3, not after the whole square (`:6677-6681`).
        if collisions >= TOO_CROWDED {
            return true;
        }
    }
    false
}

/// The segment a direction contributes to a line, as `om_lines`' own bit: north one, east two, south four, west eight (`omdata.h`).
fn segment(direction: Direction) -> usize {
    match direction {
        Direction::North => 1,
        Direction::East => 2,
        Direction::South => 4,
        Direction::West => 8,
    }
}

/// The shape a line mask names: the sixteenth part of `om_lines::all` (`overmap.cpp:266-287`).
fn shape(line: usize) -> &'static str {
    cdda_content::overmap::LINE_SUFFIXES
        .get(line)
        .copied()
        .unwrap_or("_nesw")
}

/// The line an existing terrain already carries, or nothing when it is not one of the connection's linear shapes.
///
/// Reference: `connection.has( ter_id ) ? ter_id->get_line() : 0` (`overmap.cpp:6713`). The port reads the shape back out of the id's suffix, which is what the id is made of
/// ([`cdda_content::overmap::LINE_SUFFIXES`]).
fn line_of(existing: &str) -> Option<usize> {
    let position = cdda_content::overmap::LINE_SUFFIXES
        .iter()
        .position(|suffix| existing.ends_with(suffix))?;
    Some(position)
}

/// Lays a path as a connection: every tile gets the line's **linear shape** for the segments it has, and the count of tiles laid is answered.
///
/// Reference: `overmap::build_connection` (`overmap.cpp:6697-6740`). For each node in the path the connection's subtype for that ground is chosen; the line is the tile's own if
/// it already carries one, plus the segment towards the next node, plus the segment back towards the previous one. The shape is then the subtype's terrain with the suffix that
/// mask names ([`cdda_content::overmap::LINE_SUFFIXES`], `om_lines::all`, `overmap.cpp:266-287`).
///
/// **What is not ported** is the reference's neighbour pass (`:6717-6725`), which also reaches into the roads **around** a tile, turning their shapes to meet it. A line laid
/// here joins the tile's own existing road but not its neighbours', which is a named gap rather than a silent difference.
pub fn write_connection(
    world: &mut World,
    connection: &Connection,
    locations: &BTreeMap<String, Location>,
    path: &[(i32, i32)],
) -> usize {
    let mut laid = 0;
    for (index, at) in path.iter().enumerate() {
        // A tile the port has not set is the overmap's **default ground** - `region_settings.default_oter[0]`, which the data sets to "field" - and a road crosses a field.
        let terrain = crate::overmap::terrain_at(world, *at).unwrap_or(DEFAULT_GROUND);
        let Some(subtype) = pick_subtype(connection, locations, terrain) else {
            // **Ground the connection may not cross ends the write** (`:6707-6711`), rather than being skipped over.
            return laid;
        };
        let existing = line_of(terrain).unwrap_or(0);
        let line = self_line(path, index, existing);
        crate::overmap::set_terrain(world, *at, &format!("{}{}", subtype.terrain, shape(line)));
        laid += 1;
    }
    laid
}

/// The line a path's tile carries: its own existing segments, the way onward, and the way back (`overmap.cpp:6713-6718`).
fn self_line(path: &[(i32, i32)], index: usize, existing: usize) -> usize {
    let mut line = existing;
    let Some(here) = path.get(index).copied() else {
        return existing;
    };
    if let Some(ahead) = path.get(index + 1).copied() {
        line |= segment(step_direction(here, ahead));
    }
    if index > 0
        && let Some(behind) = path.get(index - 1).copied()
    {
        line |= segment(step_direction(here, behind));
    }
    line
}

/// The direction from one tile of a path to the next, which the walk guarantees to be one of the four cardinals.
fn step_direction(from: (i32, i32), to: (i32, i32)) -> Direction {
    match (to.0 - from.0, to.1 - from.1) {
        (0, y) if y < 0 => Direction::North,
        (x, 0) if x > 0 => Direction::East,
        (0, y) if y > 0 => Direction::South,
        _ => Direction::West,
    }
}
