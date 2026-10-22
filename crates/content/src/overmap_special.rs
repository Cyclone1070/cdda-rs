//! Overmap specials: the buildings and landmarks that are placed into the world as a whole.
//!
//! Reference: `overmap_special.h` / `overmap_special.cpp` and the loader in `overmap_specials.cpp`. A special is a
//! **shape**: a list of overmap terrain tiles at offsets, plus the rules that decide whether it may be placed - where
//! it can sit, how far from a city, how rare it is.
//!
//! This module reads the data and nothing else. Placement is a separate job, and it is the job the world layer has
//! been waiting for: the port can grow a town and put streets and woodland down, and until now could not say what a
//! building *is*.

use cdda_json::Value;

/// One terrain tile of a special, at an offset from the special's own origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialTerrain {
    /// The offset, in overmap tiles, including the z level.
    pub offset: (i32, i32, i32),
    /// The terrain placed there.
    pub terrain: String,
}

/// A connection a special makes to a road or a subway, which the world has to join up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// Where the connection's road tile sits.
    pub offset: (i32, i32, i32),
    /// The terrain kind it connects as, e.g. `road`.
    pub terrain: String,
    /// Which connection it plugs into.
    pub connection: String,
    /// Where the road it joins comes from.
    pub from: (i32, i32, i32),
}

/// One special, as loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OvermapSpecial {
    /// The id the JSON gives it.
    pub id: String,
    /// The tiles it occupies.
    pub overmaps: Vec<SpecialTerrain>,
    /// How many times it may appear, as a percentage range of the overmap's capacity.
    pub occurrences: (i32, i32),
    /// Flags such as `OVERMAP_UNIQUE` and `GLOBALLY_UNIQUE`.
    pub flags: Vec<String>,
    /// The kinds of ground it may be placed on.
    pub locations: Vec<String>,
    /// How far from a city it may be, in overmap tiles.
    pub city_distance: (i32, i32),
    /// The range of city sizes it belongs to.
    pub city_sizes: (i32, i32),
    /// Whether the placement may rotate it.
    pub rotate: bool,
    /// Larger priorities are placed first.
    pub priority: i32,
    /// The roads and subways it connects to.
    pub connections: Vec<Connection>,
    /// `mutable` specials are edited by the world, which the port does not do yet.
    pub subtype: Option<String>,
}

#[allow(clippy::cast_possible_truncation)]
fn number(value: Option<&Value>, fallback: i32) -> i32 {
    match value {
        Some(Value::Int(n)) => i32::try_from(*n).unwrap_or(fallback),
        // A count written as `75.0` is a count; truncating it is the reference's own reading of a float in an
        // integer field.
        Some(Value::Float(n)) => *n as i32,
        _ => fallback,
    }
}

fn pair(value: Option<&Value>, fallback: (i32, i32)) -> (i32, i32) {
    let Some(Value::Array(items)) = value else {
        return fallback;
    };
    if items.len() != 2 {
        return fallback;
    }
    (
        number(items.first(), fallback.0),
        number(items.get(1), fallback.1),
    )
}

fn point(value: Option<&Value>) -> (i32, i32, i32) {
    match value {
        Some(Value::Array(items)) if items.len() == 3 => (
            number(items.first(), 0),
            number(items.get(1), 0),
            number(items.get(2), 0),
        ),
        _ => (0, 0, 0),
    }
}

fn text(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::Str(text)) => text.clone(),
        _ => fallback.to_string(),
    }
}

fn list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Reads one special, or nothing when the entry is not one.
#[must_use]
pub fn read_special(value: &Value) -> Option<OvermapSpecial> {
    let Value::Object(fields) = value else {
        return None;
    };
    if fields.get("type") != Some(&Value::Str("overmap_special".to_string())) {
        return None;
    }
    // Two specials describe their shape as a **graph of named nodes** for the mutable generator rather than as a list
    // of tiles. They are still specials and are still counted; their tiles arrive when the mutable generator does.
    let overmaps = match fields.get("overmaps") {
        Some(Value::Array(tiles)) => tiles,
        _ => &Vec::new(),
    };
    let overmaps = overmaps
        .iter()
        .filter_map(|tile| {
            let Value::Object(tile) = tile else {
                return None;
            };
            Some(SpecialTerrain {
                offset: point(tile.get("point")),
                terrain: text(tile.get("overmap"), ""),
            })
        })
        .collect();
    let connections = match fields.get("connections") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                let Value::Object(entry) = item else {
                    return None;
                };
                Some(Connection {
                    offset: point(entry.get("point")),
                    terrain: text(entry.get("terrain"), ""),
                    connection: text(entry.get("connection"), ""),
                    from: point(entry.get("from")),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(OvermapSpecial {
        id: text(fields.get("id"), ""),
        overmaps,
        occurrences: pair(fields.get("occurrences"), (0, 100)),
        flags: list(fields.get("flags")),
        locations: list(fields.get("locations")),
        city_distance: pair(fields.get("city_distance"), (-1, 1000)),
        city_sizes: pair(fields.get("city_sizes"), (0, 1000)),
        // The reference's default is true: a special rotates unless it says otherwise.
        rotate: !matches!(fields.get("rotate"), Some(Value::Bool(false))),
        priority: number(fields.get("priority"), 0),
        connections,
        subtype: match fields.get("subtype") {
            Some(Value::Str(text)) => Some(text.clone()),
            _ => None,
        },
    })
}

/// Reads every special in a list of parsed JSON documents.
#[must_use]
pub fn load_specials(documents: &[Value]) -> Vec<OvermapSpecial> {
    let mut specials = Vec::new();
    for document in documents {
        let entries: &[Value] = match document {
            Value::Array(entries) => entries,
            other => std::slice::from_ref(other),
        };
        for entry in entries {
            if let Some(special) = read_special(entry) {
                specials.push(special);
            }
        }
    }
    specials
}

/// A named kind of ground a special may be placed on.
///
/// Reference: `overmap_location` in `special_locations.json` - just a name and the terrains that count as it. The
/// indirection matters: a special says it goes on `forest`, and what counts as forest is stated once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OvermapLocation {
    /// The id a special refers to.
    pub id: String,
    /// The terrains that count as this location.
    pub terrains: Vec<String>,
    /// Terrain flags that also count, for locations written against flags rather than ids.
    pub flags: Vec<String>,
}

impl OvermapLocation {
    /// Whether a terrain counts as this location.
    ///
    /// Reference: `overmap_location::test`, which matches the terrain's id against the list and then its flags.
    #[must_use]
    pub fn test(&self, terrain: &str, terrain_flags: &[&str]) -> bool {
        self.terrains.iter().any(|known| known == terrain)
            || self
                .flags
                .iter()
                .any(|flag| terrain_flags.contains(&flag.as_str()))
    }
}

/// Reads one location, or nothing when the entry is not one.
#[must_use]
pub fn read_location(value: &Value) -> Option<OvermapLocation> {
    let Value::Object(fields) = value else {
        return None;
    };
    if fields.get("type") != Some(&Value::Str("overmap_location".to_string())) {
        return None;
    }
    Some(OvermapLocation {
        id: text(fields.get("id"), ""),
        terrains: list(fields.get("terrains")),
        flags: list(fields.get("flags")),
    })
}

/// Reads every location in a list of parsed JSON documents.
#[must_use]
pub fn load_locations(documents: &[Value]) -> Vec<OvermapLocation> {
    let mut locations = Vec::new();
    for document in documents {
        let entries: &[Value] = match document {
            Value::Array(entries) => entries,
            other => std::slice::from_ref(other),
        };
        for entry in entries {
            if let Some(location) = read_location(entry) {
                locations.push(location);
            }
        }
    }
    locations
}
