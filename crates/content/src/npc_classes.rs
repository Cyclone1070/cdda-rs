//! NPC classes: what an NPC is before they are given a personality.
//!
//! Reference: `npc_class.cpp`. A class holds the skill levels, traits, bionics, spells, worn gear and carried items an NPC of
//! that class starts with, and every NPC is built from exactly one. The port loads the identity of each class - its id and the
//! name the player sees - because those are what mapgen and dialogue refer to; the skill and gear tables follow as the pieces
//! that consume them are ported.

use std::collections::BTreeMap;

use cdda_json::Value;

use crate::monsters::LoadError;

/// One NPC class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcClass {
    /// Its id, as the data writes it.
    pub id: String,
    /// The name the player sees, empty when the data names none.
    pub name: String,
    /// The description shown when the class is chosen, empty when the data gives none.
    pub job_description: String,
    /// Whether the class is offered in the character creator's common list.
    pub common: bool,
    /// The item groups a shopkeeper of this class restocks from - the reference's `shop_item_groups`.
    pub shop_item_groups: Vec<ShopkeeperItemGroup>,
    /// How long a shopkeeper of this class waits between restocks, in turns; **six days** when the data names none
    /// (`npc_class.cpp:268`, `npc_class.h:103`).
    pub restock_interval: i64,
}

/// One item group a shopkeeper restocks from.
///
/// Reference: `shopkeeper_item_group` (`npc_class.cpp:218-233`): a group id with a trust floor, a `strict` flag that makes the trust floor a **restock** condition rather than
/// only a selling one, and a `rigid` flag that makes the group produce its items **once** instead of being drawn until the shop's value runs out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopkeeperItemGroup {
    /// The item group's id.
    pub group: String,
    /// The faction trust at which the group is available; zero means always.
    pub trust: i32,
    /// Whether the trust floor also gates restocking, not just selling.
    pub strict: bool,
    /// Whether the group is drawn once, whatever the shop can afford.
    pub rigid: bool,
}

/// Loads every NPC class under `dirs`.
///
/// # Errors
///
/// Reports a file that cannot be read or a class without an id.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<NpcClass>, LoadError> {
    let mut by_id: BTreeMap<String, NpcClass> = BTreeMap::new();
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
                let Some(class) = read_class(object) else {
                    continue;
                };
                // Later copies replace earlier ones, as the reference's generic factory does for a repeated id.
                by_id.insert(class.id.clone(), class);
            }
        }
    }
    Ok(by_id.into_values().collect())
}

fn read_class(object: &Value) -> Option<NpcClass> {
    let Value::Object(fields) = object else {
        return None;
    };
    match fields.get("type") {
        Some(Value::Str(kind)) if kind == "npc_class" => {}
        _ => return None,
    }
    let Some(Value::Str(id)) = fields.get("id") else {
        return None;
    };
    Some(NpcClass {
        id: id.clone(),
        name: read_name(fields.get("name")),
        job_description: match fields.get("job_description") {
            Some(Value::Str(text)) => text.clone(),
            _ => String::new(),
        },
        common: matches!(fields.get("common"), Some(Value::Bool(true))),
        shop_item_groups: read_shop_item_groups(fields.get("shopkeeper_item_group")),
        restock_interval: read_turns(fields.get("restock_interval"), 6 * 24 * 60 * 60),
    })
}

/// The shopkeeper groups a class restocks from.
///
/// Reference: `npc_class::load` (`npc_class.cpp:252-261`) takes either an array of group objects or **a single group id written as a bare string**, which it reads as one group.
fn read_shop_item_groups(value: Option<&Value>) -> Vec<ShopkeeperItemGroup> {
    let Some(value) = value else {
        return Vec::new();
    };
    match value {
        Value::Str(group) => vec![ShopkeeperItemGroup {
            group: group.clone(),
            trust: 0,
            strict: false,
            rigid: false,
        }],
        Value::Array(groups) => groups
            .iter()
            .filter_map(|group| {
                let Value::Object(fields) = group else {
                    return None;
                };
                let Some(Value::Str(id)) = fields.get("group") else {
                    return None;
                };
                Some(ShopkeeperItemGroup {
                    group: id.clone(),
                    trust: match fields.get("trust") {
                        Some(Value::Int(trust)) => i32::try_from(*trust).unwrap_or(0),
                        _ => 0,
                    },
                    strict: matches!(fields.get("strict"), Some(Value::Bool(true))),
                    rigid: matches!(fields.get("rigid"), Some(Value::Bool(true))),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A duration the data writes as a string like `"6 d"`, in turns of one second.
///
/// Reference: `optional( jo, was_loaded, "restock_interval", restock_interval, 6_days )` (`npc_class.cpp:268`) - the value is read with `read_from_json_string`, whose units are
/// the game's own (`d` days, `h` hours, `m` minutes, `s` seconds) and whose base unit here is the **turn**.
fn read_turns(value: Option<&Value>, default: i64) -> i64 {
    let Some(Value::Str(text)) = value else {
        return default;
    };
    let mut parts = text.split_whitespace();
    let (Some(number), Some(unit)) = (parts.next(), parts.next()) else {
        return default;
    };
    let Ok(number) = number.parse::<i64>() else {
        return default;
    };
    let seconds = match unit {
        "d" | "day" | "days" => 24 * 60 * 60,
        "h" | "hour" | "hours" => 60 * 60,
        "m" | "minute" | "minutes" => 60,
        "s" | "second" | "seconds" => 1,
        _ => return default,
    };
    number * seconds
}

/// The name of a class, which the data writes either as a bare string or as a translated string object.
fn read_name(value: Option<&Value>) -> String {
    match value {
        Some(Value::Str(text)) => text.clone(),
        Some(Value::Object(fields)) => match fields.get("str") {
            Some(Value::Str(text)) => text.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
