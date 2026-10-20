//! Mods: what they are called, and what must be loaded before them.
//!
//! Reference: `mod_manager` and `MOD_INFO` (`src/mod_manager.h:41-90`). The port reads the same
//! files the reference does, because a mod's identity and its dependencies are data, not runtime
//! state - which is why this loader needs no oracle: the gate is the reference's own files plus a
//! dependency order that can be checked for being an order at all.

use cdda_json::{Object, Value};

/// One mod, as its `modinfo.json` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInfo {
    /// The mod's id, which is what other mods depend on.
    pub id: String,
    /// Its display name.
    pub name: String,
    /// Mods that must load before it.
    pub dependencies: Vec<String>,
}

fn text_of(object: &Object, key: &str) -> Option<String> {
    match object.get(key)? {
        Value::Str(text) => Some(text.clone()),
        _ => None,
    }
}

/// Reads one `modinfo.json` file.
///
/// The file is normally an array of objects, and a mod may declare several - the reference allows
/// one file to describe more than one mod - so both shapes are accepted.
///
/// # Errors
///
/// Returns the reason the file could not be read: a shape it does not understand, or a mod with
/// no id.
pub fn from_data(value: &Value) -> Result<Vec<ModInfo>, String> {
    let entries: &[Value] = match value {
        Value::Array(entries) => entries,
        Value::Object(_) => std::slice::from_ref(value),
        _ => return Err("a modinfo file is neither an array nor an object".to_string()),
    };
    let mut mods = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            return Err("a modinfo entry is not an object".to_string());
        };
        match object.get("type") {
            Some(Value::Str(kind)) if kind == "MOD_INFO" => {}
            _ => continue,
        }
        let Some(id) = text_of(object, "id") else {
            return Err("a mod has no id".to_string());
        };
        let name = text_of(object, "name").unwrap_or_else(|| id.clone());
        let mut dependencies = Vec::new();
        if let Some(Value::Array(deps)) = object.get("dependencies") {
            for dep in deps {
                let Value::Str(dep) = dep else {
                    return Err("a dependency is not a string".to_string());
                };
                dependencies.push(dep.clone());
            }
        }
        mods.push(ModInfo {
            id,
            name,
            dependencies,
        });
    }
    Ok(mods)
}

/// An order in which the mods can be loaded: every mod after everything it depends on.
///
/// Mods that are ready at the same time are taken alphabetically, so the order is the same on
/// every run. That is not the reference's rule - it sorts by dependency and then by its own
/// registration order - but a port that produced a different order on every run would be
/// undebuggable, and the difference is recorded rather than hidden.
///
/// # Errors
///
/// Returns the ids of the mods caught in a dependency cycle: there is no order for those, and
/// silently dropping them would hide a broken mod set.
pub fn dependency_order(mods: &[ModInfo]) -> Result<Vec<String>, String> {
    let mut remaining: Vec<&ModInfo> = mods.iter().collect();
    let mut ordered: Vec<String> = Vec::new();
    while !remaining.is_empty() {
        let mut ready: Vec<&&ModInfo> = remaining
            .iter()
            .filter(|mod_info| {
                mod_info
                    .dependencies
                    .iter()
                    .all(|dep| ordered.contains(dep))
            })
            .collect();
        if ready.is_empty() {
            let stuck: Vec<&str> = remaining.iter().map(|m| m.id.as_str()).collect();
            return Err(format!("dependency cycle among: {}", stuck.join(", ")));
        }
        ready.sort_by(|a, b| a.id.cmp(&b.id));
        let next = ready.first().map(|m| m.id.clone()).ok_or("no mod ready")?;
        ordered.push(next.clone());
        remaining.retain(|m| m.id != next);
    }
    Ok(ordered)
}
