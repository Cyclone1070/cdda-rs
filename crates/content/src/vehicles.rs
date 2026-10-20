//! Vehicle parts and vehicle prototypes.
//!
//! Reference: `vehicles::parts::get_all` and `vehicles::get_all_prototypes`
//! (`src/veh_type.h:52,45`), reached through `--dump-vehicles`. Ids only for now: a part also
//! carries its durability, its requirements and what it can be installed on, and those come next.

use cdda_json::{Object, Value};

/// Every vehicle part and vehicle prototype the reference defines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vehicles {
    /// Part ids, sorted.
    pub parts: Vec<String>,
    /// Prototype ids, sorted.
    pub prototypes: Vec<String>,
}

fn ids_of(object: &Object, key: &str) -> Result<Vec<String>, String> {
    let Some(Value::Array(entries)) = object.get(key) else {
        return Err(format!("the dump has no {key} array"));
    };
    let mut ids = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Str(id) = entry else {
            return Err(format!("a {key} id is not a string"));
        };
        ids.push(id.clone());
    }
    Ok(ids)
}

fn count_of(object: &Object, key: &str, listed: usize) -> Result<(), String> {
    let Some(Value::Int(count)) = object.get(key) else {
        return Err(format!("the dump has no {key}"));
    };
    if usize::try_from(*count).ok() != Some(listed) {
        return Err(format!("{key} disagrees with the ids listed"));
    }
    Ok(())
}

/// Reads the vehicle dump.
///
/// The counts are checked against the arrays, as with the starting kit: a dump that contradicts
/// itself is an error, not something to paper over.
///
/// # Errors
///
/// Returns the reason the dump could not be read, or that its counts disagree with its contents.
pub fn from_dump(value: &Value) -> Result<Vehicles, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let parts = ids_of(root, "parts")?;
    let prototypes = ids_of(root, "prototypes")?;
    count_of(root, "part_count", parts.len())?;
    count_of(root, "prototype_count", prototypes.len())?;
    Ok(Vehicles { parts, prototypes })
}
