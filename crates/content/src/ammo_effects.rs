//! The ammo effect registry: which effects exist at all.
//!
//! An ammunition's `"effects"` are typed ids, so it is tempting to refuse one the registry does not hold — but **the reference does not**: `ammo_effect_str_id` is a
//! plain string id and validation only happens when somebody asks `is_valid()`, which nothing does at load. The content proves the point by putting a **flag** name
//! in an ammunition's effects (`rock` lists `ALLOWS_BODY_BLOCK`, which is a flag, not an effect), and the reference loads it without a murmur. So the registry here
//! is a **record of what exists**, for the loader to check against and for a gate to pin, not a filter.

use crate::items::LoadError;
use cdda_json::Value;
use std::collections::BTreeSet;

/// Loads every ammo effect id the content defines, in the reference's file order.
///
/// # Errors
///
/// Fails when a file cannot be read or an effect is defined twice.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<BTreeSet<String>, LoadError> {
    let mut out: BTreeSet<String> = BTreeSet::new();
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

fn collect(
    value: &Value,
    out: &mut BTreeSet<String>,
    path: &std::path::Path,
) -> Result<(), LoadError> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, path)?;
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "ammo_effect") {
                return Ok(());
            }
            let Some(Value::Str(id)) = object.get("id") else {
                return Ok(());
            };
            // The effect's own data (its burst radius, its fields) is not ported yet; only the id matters for refusing an unknown one.
            if !out.insert(id.clone()) {
                return Err(LoadError {
                    message: format!("ammo effect {id} is defined twice: {}", path.display()),
                });
            }
        }
        _ => {}
    }
    Ok(())
}
