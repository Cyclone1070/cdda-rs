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
            if out.insert(id.clone(), Quality { id: id.clone(), name }).is_some() {
                return Err(LoadError {
                    message: format!("quality {id} is defined twice: {}", path.display()),
                });
            }
        }
        _ => {}
    }
    Ok(())
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
    let mut out = BTreeMap::new();
    let Some(Value::Array(pairs)) = object.get("qualities") else {
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
