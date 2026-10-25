//! A fault: what an item acquires when it takes mechanical damage, and what that does to it.
//!
//! Reference: `fault::load` (`src/fault.cpp:226-260`) and `class fault` (`src/fault.h:76-121`). A fault is mostly **presentation** — a name, a description, a prefix and
//! a suffix for the item's displayed name — plus four numbers that are not: a `degradation_mod` that repairs can never undo, a `price_modifier`, an encumbrance
//! adjustment, and two lists of damage adjustments (melee and armour) keyed by damage type, each an **additive** integer and a **multiplicative** float.

use crate::constructions::LoadError;
use cdda_json::{Object, Value};
use std::collections::BTreeSet;

/// One adjustment a fault makes to a damage type: add this much, then multiply.
///
/// Reference: the tuples in `fault::melee_damage_mod`/`armor_mod` (`src/fault.h:85-89`). The order matters and is the reference's: `( value + add ) * multiply`
/// (`src/item.cpp:944-947`), evaluated per entry in **data order**, so two entries for the same damage type compose rather than replace.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageMod {
    /// How much to add to the value.
    pub add: i32,
    /// What to multiply the result by.
    pub multiply: f64,
    /// Which damage type this applies to.
    pub damage_id: String,
}

/// One fault.
#[derive(Debug, Clone, PartialEq)]
pub struct Fault {
    /// Its id.
    pub id: String,
    /// The name a player sees.
    pub name: String,
    /// Its description.
    pub description: String,
    /// Its `fault_type`, which is the key `faults::all_of_type` and `random_of_type` are asked with — `mechanical_damage` for the ones a hit adds.
    pub fault_type: String,
    /// The prefix the item's displayed name takes (`"cracked "`), empty when the fault has none.
    pub item_prefix: String,
    /// The suffix the displayed name takes, empty when the fault has none.
    pub item_suffix: String,
    /// What it multiplies the item's price by.
    pub price_modifier: f64,
    /// Damage that can never be repaired, in damage units.
    pub degradation_mod: i32,
    /// Whether the item's own degradation multiplies this fault's effects.
    pub affected_by_degradation: bool,
    /// Flat encumbrance added.
    pub encumbrance_add: f64,
    /// Encumbrance multiplier.
    pub encumbrance_mult: f64,
    /// The faults it prevents.
    pub block_faults: BTreeSet<String>,
    /// Melee damage adjustments, in data order.
    pub melee_damage_mod: Vec<DamageMod>,
    /// Armour adjustments, in data order.
    pub armor_mod: Vec<DamageMod>,
}

impl Fault {
    /// Reads one fault out of its JSON object.
    ///
    /// # Errors
    ///
    /// If the file cannot be read; a fault's members are all optional in the reference, so nothing here fails on a missing field.
    pub fn read(object: &Object) -> Result<Self, LoadError> {
        // **A fault's name is a translation object, its description a plain string.** `{ "str": "Blown fuse" }` is how the data spells a translatable name
        // (`fault_electronic_blown_fuse`), and a plain string is how it spells everything else, so the reader takes both for every text field.
        let text = |key: &str| match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            Some(Value::Object(value)) => match value.get("str") {
                Some(Value::Str(value)) => value.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        let number = |key: &str, fallback: f64| match object.get(key) {
            Some(Value::Float(value)) => *value,
            Some(Value::Int(value)) => f64_of(*value),
            _ => fallback,
        };
        Ok(Self {
            id: text("id"),
            name: text("name"),
            description: text("description"),
            fault_type: text("fault_type"),
            item_prefix: text("item_prefix"),
            item_suffix: text("item_suffix"),
            price_modifier: number("price_modifier", 1.0),
            #[allow(clippy::cast_possible_truncation)]
            degradation_mod: number("degradation_mod", 0.0) as i32,
            affected_by_degradation: matches!(
                object.get("affected_by_degradation"),
                Some(Value::Bool(true))
            ),
            encumbrance_add: number("encumbrance_add", 0.0),
            encumbrance_mult: number("encumbrance_mult", 1.0),
            block_faults: crate::pockets::id_list(object, "block_faults")
                .into_iter()
                .collect(),
            melee_damage_mod: mods(object, "melee_damage_mod"),
            armor_mod: mods(object, "armor_mod"),
        })
    }
}

/// Reads one modifier list.
fn mods(object: &Object, key: &str) -> Vec<DamageMod> {
    let Some(Value::Array(entries)) = object.get(key) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| match entry {
            Value::Object(entry) => {
                let add = match entry.get("add") {
                    Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
                    #[allow(clippy::cast_possible_truncation)]
                    Some(Value::Float(value)) => *value as i32,
                    _ => 0,
                };
                let multiply = match entry.get("multiply") {
                    Some(Value::Float(value)) => *value,
                    Some(Value::Int(value)) => f64_of(*value),
                    _ => 1.0,
                };
                Some(DamageMod {
                    add,
                    multiply,
                    damage_id: match entry.get("damage_id") {
                        Some(Value::Str(id)) => id.clone(),
                        _ => String::new(),
                    },
                })
            }
            _ => None,
        })
        .collect()
}

/// A whole number as a real. A price modifier is a fraction of a cent, so nothing here is near the point where `f64` loses a digit.
fn f64_of(value: i64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let converted = value as f64;
    converted
}

/// Resolves one fault against the objects it may copy from.
///
/// Reference: `fault` is a `generic_factory` type, so `"copy-from"` copies the parent whole and then the child's own members are applied on top — which means a scalar
/// the child omits **keeps the parent's value** (`fault_staff_short_broken` inherits `degradation_mod: 50` and `affected_by_degradation: true` from
/// `fault_handle_broken`) and that `melee_damage_mod`/`armor_mod`, which are loaded with `emplace_back`, are **appended** to the parent's rather than replacing them
/// (the child's own extra entry comes after the parent's three).
fn resolve(
    id: &str,
    objects: &std::collections::BTreeMap<String, cdda_json::Object>,
    seen: &mut Vec<String>,
) -> Result<Fault, LoadError> {
    let Some(object) = objects.get(id) else {
        return Err(LoadError {
            message: format!("fault {id} is copied from but never defined"),
        });
    };
    let mut fault = Fault::read(object)?;
    if let Some(Value::Str(parent)) = object.get("copy-from")
        && !seen.iter().any(|seen| seen == parent)
    {
        seen.push(parent.clone());
        let inherited = resolve(parent, objects, seen)?;
        fault = inherit(inherited, object, fault);
    }
    Ok(fault)
}

/// Applies one fault's own members over what it inherited.
fn inherit(parent: Fault, object: &cdda_json::Object, child: Fault) -> Fault {
    let stated = |key: &str| object.get(key).is_some();
    let mut out = child;
    out.name = if stated("name") { out.name } else { parent.name };
    out.description = if stated("description") {
        out.description
    } else {
        parent.description
    };
    out.fault_type = if stated("fault_type") {
        out.fault_type
    } else {
        parent.fault_type
    };
    out.item_prefix = if stated("item_prefix") {
        out.item_prefix
    } else {
        parent.item_prefix
    };
    out.item_suffix = if stated("item_suffix") {
        out.item_suffix
    } else {
        parent.item_suffix
    };
    out.price_modifier = if stated("price_modifier") {
        out.price_modifier
    } else {
        parent.price_modifier
    };
    out.degradation_mod = if stated("degradation_mod") {
        out.degradation_mod
    } else {
        parent.degradation_mod
    };
    out.affected_by_degradation = if stated("affected_by_degradation") {
        out.affected_by_degradation
    } else {
        parent.affected_by_degradation
    };
    out.encumbrance_add = if stated("encumbrance_add") {
        out.encumbrance_add
    } else {
        parent.encumbrance_add
    };
    out.encumbrance_mult = if stated("encumbrance_mult") {
        out.encumbrance_mult
    } else {
        parent.encumbrance_mult
    };
    out.block_faults = if stated("block_faults") {
        out.block_faults
    } else {
        parent.block_faults
    };
    // **The lists are appended, not replaced.** The reference loads them with `emplace_back` onto whatever `copy-from` copied in (`src/fault.cpp:243-259`).
    let mut melee = parent.melee_damage_mod;
    melee.extend(out.melee_damage_mod);
    out.melee_damage_mod = melee;
    let mut armor = parent.armor_mod;
    armor.extend(out.armor_mod);
    out.armor_mod = armor;
    out
}

/// Loads every fault in the given directories, in the reference's file order.
///
/// # Errors
///
/// If a directory cannot be walked or a file cannot be read.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<Fault>, LoadError> {
    let mut objects: std::collections::BTreeMap<String, cdda_json::Object> =
        std::collections::BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(entries) = &*value else {
                continue;
            };
            for entry in entries {
                let Value::Object(object) = entry else {
                    continue;
                };
                if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "fault") {
                    continue;
                }
                let Some(Value::Str(id)) = object.get("id") else {
                    continue;
                };
                if !objects.contains_key(id) {
                    order.push(id.clone());
                }
                objects.insert(id.clone(), object.clone());
            }
        }
    }
    let mut out = Vec::new();
    for id in &order {
        out.push(resolve(id, &objects, &mut Vec::new())?);
    }
    Ok(out)
}

/// How a fault changes one number: add, then multiply, the way the reference applies its tuples.
#[must_use]
pub fn adjust(value: f64, mods: &[DamageMod], damage_type: &str) -> f64 {
    let mut adjusted = value;
    for entry in mods.iter().filter(|entry| entry.damage_id == damage_type) {
        adjusted = (adjusted + f64::from(entry.add)) * entry.multiply;
    }
    adjusted
}

/// A named bag of faults with weights, which an item type can point at instead of listing each one.
///
/// Reference: `class fault_group` and `fault_group::load` (`src/fault.cpp:393-401`): a `group` array of `{ "fault": ..., "weight": ... }`, the weight defaulting to 100.
#[derive(Debug, Clone, PartialEq)]
pub struct FaultGroup {
    /// Its id.
    pub id: String,
    /// The faults it holds, with their weights, in data order.
    pub entries: Vec<(String, i32)>,
}

/// Loads every fault group in the given directories.
///
/// # Errors
///
/// If a directory cannot be walked or a file cannot be read.
pub fn load_groups(dirs: &[&std::path::Path]) -> Result<Vec<FaultGroup>, LoadError> {
    let mut out = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(entries) = &*value else {
                continue;
            };
            for entry in entries {
                let Value::Object(object) = entry else {
                    continue;
                };
                if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "fault_group") {
                    continue;
                }
                let Some(Value::Str(id)) = object.get("id") else {
                    continue;
                };
                let mut group = FaultGroup {
                    id: id.clone(),
                    entries: Vec::new(),
                };
                if let Some(Value::Array(members)) = object.get("group") {
                    for member in members {
                        let Value::Object(member) = member else {
                            continue;
                        };
                        let Some(Value::Str(fault)) = member.get("fault") else {
                            continue;
                        };
                        let weight = match member.get("weight") {
                            Some(Value::Int(weight)) => i32::try_from(*weight).unwrap_or(100),
                            _ => 100,
                        };
                        group.entries.push((fault.clone(), weight));
                    }
                }
                out.push(group);
            }
        }
    }
    Ok(out)
}
