//! Tool qualities: what makes a hammer a hammer.
//!
//! Two registries, and the reference keeps them apart. `quality` (`src/requirements.h:44-56`) is the catalogue — an id, a translated name and the usages the quality
//! unlocks — while the **level** an item reaches lives on the item's own type, as a map from quality to level (`itype::qualities`). The two are joined only when a
//! requirement asks "is this at least level 3".
//!
//! Levels can be **negative**, and that is not a sentinel. The reference writes a negative level when a tool is worse at the job than doing it with bare hands, so a saw
//! is `BUTCHER -90` and a sledgehammer is a worse butchering tool than a knife. Anything that clamps levels at zero breaks those tools silently.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One quality the data defines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quality {
    /// Its id, such as `HAMMER`.
    pub id: String,
    /// The name a player sees.
    pub name: String,
    /// The actions this quality lends at a given level: `[ [ 1, [ "salvage", "inscribe" ] ] ]` means any item with the quality **at level 1 or above** can
    /// `salvage` and `inscribe`. Filled by the item factory's finalisation pass (`src/item_factory.cpp:333-355`), which is why an item's action list is longer than
    /// its own JSON.
    pub usages: Vec<(i32, Vec<String>)>,
}

/// Loads the quality registry, in the reference's file order.
///
/// # Errors
///
/// Fails when a file cannot be read or a quality is defined twice.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<BTreeMap<String, Quality>, LoadError> {
    let mut out: BTreeMap<String, Quality> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut out, &path)?;
        }
    }
    Ok(out)
}

/// Why the qualities could not be loaded.
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

fn collect(
    value: &Value,
    out: &mut BTreeMap<String, Quality>,
    path: &std::path::Path,
) -> Result<(), LoadError> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, path)?;
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "tool_quality") {
                return Ok(());
            }
            let Some(Value::Str(id)) = object.get("id") else {
                return Ok(());
            };
            let name = name_of(object).unwrap_or_else(|| id.clone());
            let usages = read_usages(object);
            if out
                .insert(
                    id.clone(),
                    Quality {
                        id: id.clone(),
                        name,
                        usages,
                    },
                )
                .is_some()
            {
                return Err(LoadError {
                    message: format!("quality {id} is defined twice: {}", path.display()),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

/// A quality's `usages`: `[ [ level, [ action ids ] ] ]`.
///
/// Reference: `struct quality::usages` (`src/requirements.h`), read by `generic_factory`. The action ids are **legacy names or actor types**, and the reference
/// only adds one whose `usage_from_string` resolves — which is checked at the call site, because that table is the item factory's.
fn read_usages(object: &Object) -> Vec<(i32, Vec<String>)> {
    let Some(Value::Array(entries)) = object.get("usages") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let Value::Array(parts) = entry else {
                return None;
            };
            let level = match parts.first() {
                Some(Value::Int(level)) => i32::try_from(*level).ok()?,
                _ => return None,
            };
            let actions = match parts.get(1) {
                Some(Value::Array(actions)) => actions
                    .iter()
                    .filter_map(|action| match action {
                        Value::Str(action) => Some(action.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            Some((level, actions))
        })
        .collect()
}

/// A name, which the reference writes either as a bare string or as an object with `str` and a context.
fn name_of(object: &Object) -> Option<String> {
    match object.get("name") {
        Some(Value::Str(name)) => Some(name.clone()),
        Some(Value::Object(inner)) => match inner.get("str") {
            Some(Value::Str(name)) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Reads an item's qualities: a list of `[ quality, level ]` pairs.
///
/// Reference: `itype::qualities`, loaded by `generic_factory` from the item's `qualities` member. The reference stores it in a map keyed by quality, so the pairs are
/// read into one here and a repeated quality keeps the last level, which is what an insert into a `std::map` does.
#[must_use]
pub fn item_qualities(object: &Object) -> BTreeMap<String, i32> {
    item_qualities_keyed(object, "qualities")
}

/// Reads a `[ [ quality, level ] ]` member under any name, because `charged_qualities` has the same shape.
#[must_use]
pub fn item_qualities_keyed(object: &Object, member: &str) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    let Some(Value::Array(pairs)) = object.get(member) else {
        return out;
    };
    for pair in pairs {
        let Value::Array(pair) = pair else {
            continue;
        };
        let Some(Value::Str(id)) = pair.first() else {
            continue;
        };
        let level = match pair.get(1) {
            Some(Value::Int(level)) => i32::try_from(*level).unwrap_or(0),
            _ => 0,
        };
        out.insert(id.clone(), level);
    }
    out
}
