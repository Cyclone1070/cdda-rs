//! Mapgen keys: which overmap terrain ids have a map to generate.
//!
//! Reference: `mapgen::load_mapgen` and `mapgen_factory` (`src/mapgen.cpp:1138-1450`). A mapgen is
//! registered under the ids in its `om_terrain` field, which may be a string, a list, or a grid of
//! lists; nested and update mapgens register elsewhere and are kept apart here.
//!
//! The key the game looks up is the terrain's **mapgen id**, not its own id: 24,191 of 24,218
//! terrains have a map under that key while only 325 match the terrain id. Getting that wrong is
//! what made this family look blocked for several rounds, so it is written down here.

use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The keys one mapgen object registers, split by the registry they belong to.
#[derive(Debug, Default, Clone)]
pub struct MapgenKeys {
    /// Overmap terrain ids (`om_terrain`), the ones `mapgen_factory` holds.
    pub terrains: Vec<String>,
    /// Nested mapgen ids.
    pub nested: Vec<String>,
    /// Update mapgen ids, which are applied to an existing map.
    pub update: Vec<String>,
}

/// Every mapgen key in a set of data directories.
#[derive(Debug, Default, Clone)]
pub struct MapgenIndex {
    /// Overmap terrain mapgen keys, deduplicated and sorted.
    pub terrains: BTreeSet<String>,
    /// Nested mapgen ids.
    pub nested: BTreeSet<String>,
    /// Update mapgen ids.
    pub update: BTreeSet<String>,
}

/// Why a mapgen index could not be read.
#[derive(Debug)]
pub struct LoadError {
    /// The problem.
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}

/// Reads every mapgen object under a set of data directories.
///
/// # Errors
/// Returns an error when a directory cannot be walked.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<MapgenIndex, LoadError> {
    let mut index = MapgenIndex::default();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(objects) = value.as_ref() else {
                continue;
            };
            for object in objects {
                let Value::Object(object) = object else {
                    continue;
                };
                let keys = keys_of(object);
                index.terrains.extend(keys.terrains);
                index.nested.extend(keys.nested);
                index.update.extend(keys.update);
            }
        }
    }
    Ok(index)
}

/// The keys one JSON object registers, when it is a mapgen at all.
#[must_use]
pub fn keys_of(object: &Object) -> MapgenKeys {
    let mut keys = MapgenKeys::default();
    let is_mapgen = matches!(object.get("type"), Some(Value::Str(kind)) if kind == "mapgen");
    if !is_mapgen {
        return keys;
    }
    if let Some(Value::Str(id)) = object.get("om_terrain") {
        keys.terrains.push(id.clone());
    } else if let Some(Value::Array(rows)) = object.get("om_terrain") {
        for row in rows {
            match row {
                // A list of ids for one map.
                Value::Str(id) => keys.terrains.push(id.clone()),
                // A grid row: several ids, one per column.
                Value::Array(columns) => {
                    for column in columns {
                        if let Value::Str(id) = column {
                            keys.terrains.push(id.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(Value::Str(id)) = object.get("nested_mapgen_id") {
        keys.nested.push(id.clone());
    }
    if let Some(Value::Str(id)) = object.get("update_mapgen_id") {
        keys.update.push(id.clone());
    }
    keys
}

/// One mapgen object as the JSON states it: the rows of glyphs and what each glyph means.
///
/// Reference: `mapgen::load` for `method: "json"` (`src/mapgen.cpp`), which reads `rows` and resolves each
/// character through `terrain` or `furniture`. The port keeps the raw rows and the symbol table, because
/// resolving a symbol to an id is the terrain loader's job and doing it twice is how two loaders drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapgenObject {
    /// The overmap terrain this builds, or the nested id it is filed under.
    pub om_terrain: String,
    /// The rows, top to bottom.
    pub rows: Vec<String>,
    /// What each character means, as terrain ids.
    pub terrain: BTreeMap<char, String>,
    /// What each character means, as furniture ids.
    pub furniture: BTreeMap<char, String>,
    /// How likely this one is when several build the same terrain.
    pub weight: i32,
}

impl MapgenObject {
    /// How wide the rows are, which is the map's own width when the rows agree.
    #[must_use]
    pub fn width(&self) -> usize {
        // Characters, not bytes: a map may use a non-ASCII glyph, and counting bytes would call every
        // row of such a map ragged.
        self.rows.first().map_or(0, |row| row.chars().count())
    }

    /// How many rows there are.
    #[must_use]
    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// Whether every row is the same width, which the reference requires.
    #[must_use]
    pub fn rows_agree(&self) -> bool {
        let width = self.width();
        self.rows.iter().all(|row| row.chars().count() == width)
    }

    /// The terrain id at a square, if the rows cover it.
    #[must_use]
    pub fn terrain_at(&self, x: usize, y: usize) -> Option<&str> {
        let symbol = self.rows.get(y)?.chars().nth(x)?;
        self.terrain.get(&symbol).map(String::as_str)
    }
}

/// Reads every JSON mapgen in load order.
#[must_use]
pub fn load_objects(value: &cdda_json::Value) -> Vec<MapgenObject> {
    let mut out = Vec::new();
    // A file may hold one object or a list of them; the reference accepts both.
    let single = [value.clone()];
    let entries: &[cdda_json::Value] = match value {
        cdda_json::Value::Array(entries) => entries,
        cdda_json::Value::Object(_) => &single,
        _ => &[],
    };
    for entry in entries {
        let cdda_json::Value::Object(object) = entry else {
            continue;
        };
        // Only the JSON method builds from rows; the others are code.
        // Only the JSON method builds from rows; the others are code (`src/mapgen.cpp`).
        if let Some(cdda_json::Value::Str(method)) = object.get("method")
            && method != "json"
        {
            continue;
        }
        let Some(cdda_json::Value::Str(om_terrain)) = object.get("om_terrain") else {
            continue;
        };
        // The rows live under `object`, not beside `om_terrain`: a mapgen entry looks like
        // {"type":"mapgen","method":"json","om_terrain":"shelter","object":{"rows":[...]}}.
        // Reading the top level instead finds nothing and looks like a broken loader.
        let Some(cdda_json::Value::Object(inner)) = object.get("object") else {
            continue;
        };
        let Some(cdda_json::Value::Array(rows)) = inner.get("rows") else {
            continue;
        };
        let rows: Vec<String> = rows
            .iter()
            .filter_map(|row| match row {
                cdda_json::Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        let symbols = |key: &str| {
            let mut table = BTreeMap::new();
            if let Some(cdda_json::Value::Object(map)) = inner.get(key) {
                for (symbol, id) in map.members() {
                    if let (Some(character), cdda_json::Value::Str(id)) =
                        (symbol.chars().next(), id)
                    {
                        table.insert(character, id.clone());
                    }
                }
            }
            table
        };
        out.push(MapgenObject {
            om_terrain: om_terrain.clone(),
            rows,
            terrain: symbols("terrain"),
            furniture: symbols("furniture"),
            weight: match object.get("weight") {
                Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(1000),
                _ => 1000,
            },
        });
    }
    out
}
