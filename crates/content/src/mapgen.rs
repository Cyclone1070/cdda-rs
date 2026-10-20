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
use std::collections::BTreeSet;

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
