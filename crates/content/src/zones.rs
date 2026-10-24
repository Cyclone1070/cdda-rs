//! Zone types: the catalogue of areas a character can mark out.
//!
//! Reference: `zone_type` (`src/clzones.h:40-72`) and its loader (`src/clzones.cpp:157-166`) — an id, a translated name and description, the **field** the map draws the
//! zone with, whether a character may own one, and whether the interface hides it.
//!
//! The one thing about this registry that does not read the way it looks is the JSON tag: the data writes zone types with `"type": "LOOT_ZONE"` (`src/init.cpp:370`), not
//! `"zone_type"`, so a loader that looks for its own name finds nothing and a gate that counts what it found is happy with zero.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One zone type the data defines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneType {
    /// Its id, such as `LOOT_FOOD`.
    pub id: String,
    /// The name a player sees.
    pub name: String,
    /// What the zone is for, as one sentence.
    pub description: String,
    /// The field the map draws the zone with, such as `fd_food_loot_zone`.
    pub display_field: String,
    /// Whether a character may own the zone, as opposed to an NPC faction or the camp.
    pub can_be_personal: bool,
    /// Whether the interface leaves the zone out of its list.
    pub hidden: bool,
}

/// Why the zone types could not be loaded.
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

/// Loads every zone type, in the reference's file order.
///
/// # Errors
///
/// Fails when a directory cannot be walked. A zone type defined twice is **not** an error: the reference's factory lets a mod redefine one, and the last definition wins,
/// which is what the data relies on for the zones it overrides.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<BTreeMap<String, ZoneType>, LoadError> {
    let mut out: BTreeMap<String, ZoneType> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut out);
        }
    }
    Ok(out)
}

fn collect(value: &Value, out: &mut BTreeMap<String, ZoneType>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out);
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "LOOT_ZONE") {
                return;
            }
            let Some(Value::Str(id)) = object.get("id") else {
                return;
            };
            let zone = ZoneType {
                id: id.clone(),
                name: name_of(object, "name").unwrap_or_else(|| id.clone()),
                description: name_of(object, "description").unwrap_or_default(),
                display_field: match object.get("display_field") {
                    Some(Value::Str(text)) => text.clone(),
                    _ => String::new(),
                },
                can_be_personal: matches!(object.get("can_be_personal"), Some(Value::Bool(true))),
                hidden: matches!(object.get("hidden"), Some(Value::Bool(true))),
            };
            if let Some(previous) = out.get_mut(id) {
                *previous = zone;
            } else {
                out.insert(id.clone(), zone);
            }
        }
        _ => {}
    }
}

/// A translated string, which the data writes either plainly or as a `{"str": ...}` object.
///
/// The plural forms (`str_sp`, `str_pl`) are not read: a zone's name and description never use them in this data set, and a loader that guessed would put the wrong
/// form in front of a player.
fn name_of(object: &Object, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::Str(text)) => Some(text.clone()),
        Some(Value::Object(inner)) => match inner.get("str") {
            Some(Value::Str(text)) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}
