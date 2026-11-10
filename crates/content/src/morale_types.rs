//! The morale types the data defines, and nothing else about them.
//!
//! Reference: `morale_type` (`morale.cpp:40-90`). A morale point names one of these, and the reference refuses to apply a point whose type nobody defined - so the port
//! needs the list before it can add what a spell's `effect_str` says. The rest of a morale type (its text, its item or event trigger) is interface and trigger data the
//! port has no rule for yet; the id is what decides whether a spell's morale lands.

use cdda_json::{Object, Value};

/// Reads every morale type id the given files define.
///
/// # Errors
/// When a file cannot be read or is not JSON.
pub fn load(paths: &[&str]) -> Result<std::collections::BTreeSet<String>, crate::materials::LoadError> {
    let mut kinds = std::collections::BTreeSet::new();
    for path in paths {
        let text = std::fs::read_to_string(path).map_err(|error| crate::materials::LoadError {
            message: format!("{path}: {error}"),
        })?;
        let value = cdda_json::parse(text.as_bytes()).map_err(|error| crate::materials::LoadError {
            message: format!("{path}: {error:?}"),
        })?;
        let Value::Array(entries) = value else {
            continue;
        };
        for entry in entries {
            if let Value::Object(object) = entry {
                if let Some(id) = id_of(&object) {
                    kinds.insert(id);
                }
            }
        }
    }
    Ok(kinds)
}

/// One type's id, when the object states one.
fn id_of(object: &Object) -> Option<String> {
    match object.get("id") {
        Some(Value::Str(id)) => Some(id.clone()),
        _ => None,
    }
}
