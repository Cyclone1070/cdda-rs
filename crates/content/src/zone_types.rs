//! The zone types the data defines.
//!
//! Reference: `zone_type` (`clzones.h`) as the game loads it from JSON under the type `LOOT_ZONE` (`clzones.cpp:1013-1040`, `zone_type::load`). A zone type is what a **zone** is a
//! pudding of: its id, the name the player sees in the zone manager, the field it displays, whether it can be **personal** (a zone one character owns rather than the faction's), and
//! the description that says what it is for.
//!
//! The id is the whole reason to read them: the multiple-activity rules name a zone type by id (`get_zone_for_act`, `activity_item_handling.cpp:2642-2688`), and a port that
//! invented those names would be mapping activities to zones the data does not have.

use std::collections::BTreeMap;

use cdda_json::Value;

use crate::monsters::LoadError;

/// One zone type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneType {
    /// Its id, as the data writes it, e.g. `"LOOT_CORPSE"`.
    pub id: String,
    /// The name the player sees.
    pub name: String,
    /// The field drawn where a zone of this type is, empty when the data gives none.
    pub display_field: String,
    /// Whether a character can own zones of this type.
    pub can_be_personal: bool,
    /// What the type is for, as the data describes it.
    pub description: String,
}

/// Every zone type the data defines, by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneTypes {
    /// In load order, one per id.
    pub kinds: Vec<ZoneType>,
    /// The same ones, by id.
    pub by_id: BTreeMap<String, ZoneType>,
}

/// Loads every zone type under `dirs`.
///
/// # Errors
///
/// Reports a file that cannot be read or a zone type without an id.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<ZoneTypes, LoadError> {
    let mut by_id: BTreeMap<String, ZoneType> = BTreeMap::new();
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
                let Some(kind) = read_zone_type(object) else {
                    continue;
                };
                by_id.insert(kind.id.clone(), kind);
            }
        }
    }
    let kinds = by_id.values().cloned().collect();
    Ok(ZoneTypes { kinds, by_id })
}

fn read_zone_type(object: &Value) -> Option<ZoneType> {
    let Value::Object(fields) = object else {
        return None;
    };
    // **The JSON type is `LOOT_ZONE`, not `zone_type`**: the reference loads a zone type under the tag of the game's original loot zones, and every zone type in the data -
    // farming, mining, vehicle repair - is written with it (`data/json/loot_zones.json` and its siblings).
    match fields.get("type") {
        Some(Value::Str(kind)) if kind == "LOOT_ZONE" => {}
        _ => return None,
    }
    let Some(Value::Str(id)) = fields.get("id") else {
        return None;
    };
    Some(ZoneType {
        id: id.clone(),
        name: match fields.get("name") {
            Some(Value::Str(name)) => name.clone(),
            _ => String::new(),
        },
        display_field: match fields.get("display_field") {
            Some(Value::Str(field)) => field.clone(),
            _ => String::new(),
        },
        can_be_personal: matches!(fields.get("can_be_personal"), Some(Value::Bool(true))),
        description: match fields.get("description") {
            Some(Value::Str(description)) => description.clone(),
            _ => String::new(),
        },
    })
}
