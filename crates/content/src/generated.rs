//! Prototypes the reference makes for itself while loading.
//!
//! A good share of the ids in the reference are never written in a file. Some
//! are built for logic (`mon_null`, the dormant twins), and some are built from
//! data carried inline by another object. Enchantments are the clearest case: a
//! mutation, bionic, effect or vehicle part may carry an `enchantments` array,
//! and every object in it becomes a prototype of its own, named after its parent
//! and its position.

use cdda_json::{Object, Value};
use std::collections::BTreeSet;
use std::path::Path;

/// Why the generated ids could not be worked out.
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

/// Every enchantment id the reference ends up with: the ones written in a file,
/// and the ones built from an inline array.
///
/// The naming is the reference's: `INLINE_ENCH_` and the parent's id, then the
/// position in the array. Bionics are the exception - they name their inline
/// enchantments after their **display name**, so a bionic called "Alarm System"
/// produces `INLINE_ENCH_Alarm System_0` (`bionics.cpp:379-382`).
///
/// # Errors
///
/// Reports a file that cannot be read.
pub fn enchantment_ids(dirs: &[&Path]) -> Result<BTreeSet<String>, LoadError> {
    let mut ids = BTreeSet::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut ids);
        }
    }
    Ok(ids)
}

/// The vehicle parts the reference builds for every gun that can be mounted.
///
/// A gun becomes a turret part named `turret_<item>` unless it is a gun
/// modification or carries one of the flags that make it unusable in a mount
/// (`veh_type.cpp:473-490,568-575`).
#[must_use]
pub fn turret_part_ids(items: &[crate::items::ItemType]) -> BTreeSet<String> {
    const UNMOUNTABLE: [&str; 6] = [
        "BIONIC_WEAPON",
        "NO_TURRET",
        "NO_UNWIELD",
        "PSEUDO",
        "RELOAD_AND_SHOOT",
        "STR_DRAW",
    ];
    items
        .iter()
        .filter(|item| item.subtypes.contains("GUN") && !item.subtypes.contains("GUNMOD"))
        .filter(|item| !UNMOUNTABLE.iter().any(|flag| item.flags.contains(*flag)))
        .map(|item| format!("turret_{}", item.id))
        .collect()
}

/// Every martial-art buff id the reference ends up with.
///
/// A style carries its buffs inline, one array per trigger - `onhit_buffs`,
/// `onblock_buffs`, `oncrit_buffs` and so on - and each entry names itself. The
/// member names are not fixed, so any member ending in `_buffs` is read.
///
/// # Errors
///
/// Reports a file that cannot be read.
pub fn buff_ids(dirs: &[&Path]) -> Result<BTreeSet<String>, LoadError> {
    let mut ids = BTreeSet::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect_buffs(&value, &mut ids);
        }
    }
    Ok(ids)
}

fn collect_buffs(value: &Value, ids: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_buffs(item, ids);
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "martial_art") {
                return;
            }
            for (member, value) in object.members() {
                if !member.ends_with("_buffs") {
                    continue;
                }
                let Value::Array(buffs) = value else {
                    continue;
                };
                for buff in buffs {
                    if let Value::Object(buff) = buff
                        && let Some(Value::Str(id)) = buff.get("id")
                    {
                        ids.insert(id.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect(value: &Value, ids: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, ids);
            }
        }
        Value::Object(object) => {
            let kind = match object.get("type") {
                Some(Value::Str(kind)) => kind.to_lowercase(),
                _ => String::new(),
            };
            if kind == "enchantment"
                && let Some(Value::Str(id)) = object.get("id")
            {
                ids.insert(id.clone());
            }
            let Some(Value::Array(inline)) = object.get("enchantments") else {
                return;
            };
            // A bionic names its inline enchantments after the name it shows,
            // not after its id.
            let prefix = if kind == "bionic" {
                name_of(object)
            } else {
                identifier(object)
            };
            let Some(prefix) = prefix else {
                return;
            };
            for (index, entry) in inline.iter().enumerate() {
                if matches!(entry, Value::Object(_)) {
                    ids.insert(format!("INLINE_ENCH_{prefix}_{index}"));
                }
            }
        }
        _ => {}
    }
}

/// The id the object is filed under, or the abstract it defines.
fn identifier(object: &Object) -> Option<String> {
    match object.get("id") {
        Some(Value::Str(id)) => Some(id.clone()),
        _ => match object.get("abstract") {
            Some(Value::Str(id)) => Some(id.clone()),
            _ => None,
        },
    }
}

/// The text the object shows, from a string or from `str` or `str_sp`.
fn name_of(object: &Object) -> Option<String> {
    match object.get("name") {
        Some(Value::Str(name)) => Some(name.clone()),
        Some(Value::Object(inner)) => match inner.get("str").or_else(|| inner.get("str_sp")) {
            Some(Value::Str(name)) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}
