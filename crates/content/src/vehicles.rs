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

/// The flags that take a gun out of the running for a turret part.
///
/// Reference: `mountable_gun_filter` (`src/veh_type.cpp:473-491`). A bionic weapon is part of a person, `NO_TURRET` says so outright, something that cannot be
/// unwielded cannot be mounted, a pseudo item is not a real thing, a reload-and-shoot weapon has no magazine to feed a turret, and a strength-drawn bow needs
/// its archer.
const NOT_MOUNTABLE: [&str; 6] = [
    "BIONIC_WEAPON",
    "NO_TURRET",
    "NO_UNWIELD",
    "PSEUDO",
    "RELOAD_AND_SHOOT",
    "STR_DRAW",
];

/// The turret parts the reference invents for the guns that could be mounted.
///
/// Reference: `vehicles::parts::finalize` (`src/veh_type.cpp:565-640`). This is why a port that only reads the `vehicle_part` entries has 462 parts where the
/// reference has 701: the missing 239 are never written down, they are made from the guns.
#[must_use]
pub fn turret_parts(items: &[crate::items::ItemType]) -> Vec<String> {
    let mut parts: Vec<String> = items
        .iter()
        .filter(|item| item.is_gun && !item.is_gunmod)
        .filter(|item| !NOT_MOUNTABLE.iter().any(|flag| item.flags.contains(*flag)))
        .map(|item| format!("turret_{}", item.id))
        .collect();
    parts.sort();
    parts
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
