//! Item groups: the named sets a map or a monster spawns items from.
//!
//! Reference: `item_group` (`src/item_group.cpp`), reached through `--dump-item-groups`. The
//! loader starts with the ids, because a group is only ever referred to by name: a spawn entry
//! that names a group which does not exist spawns nothing, and no count of items will reveal it.

use cdda_json::{Object, Value};

/// One item a group can produce, with how few and how many of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    /// The item's id.
    pub item: String,
    /// The reference's lower figure for a spawn.
    pub min: i32,
    /// The reference's upper figure for a spawn.
    pub max: i32,
}

/// One item group and everything it can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The group's id.
    pub id: String,
    /// Items it can produce, with nested groups already expanded.
    pub items: Vec<Spawn>,
}

/// Every item group the reference defines, with its contents.
///
/// The two numbers per item are the reference's own aggregate (`every_item_min_max`,
/// `src/item_group.cpp:967-990`), which walks the whole tree and **sums the lower figures and
/// the upper figures separately**. For a distribution group - pick one of these - that arithmetic
/// is not an interval, and the reference's own output proves it: 376 entries come out with the
/// lower figure above the upper one. They are kept exactly as given. Normalising them would
/// quietly correct the reference's data and change what a crate contains.
///
/// # Errors
///
/// Returns the reason the dump could not be read: a missing member, a member of the wrong kind,
/// or a count that is not a non-negative integer.
pub fn from_dump(value: &Value) -> Result<(Vec<Group>, usize), String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let Some(Value::Array(entries)) = root.get("groups") else {
        return Err("the dump has no groups array".to_string());
    };
    let Some(Value::Int(count)) = root.get("count") else {
        return Err("the dump has no count".to_string());
    };
    let count = usize::try_from(*count).map_err(|_| "the count is negative".to_string())?;
    let mut groups = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(object) = entry else {
            return Err("a group entry is not an object".to_string());
        };
        let Some(Value::Str(id)) = object.get("id") else {
            return Err("a group has no id".to_string());
        };
        groups.push(Group {
            id: id.clone(),
            items: spawns_of(object)?,
        });
    }
    Ok((groups, count))
}

fn spawns_of(object: &Object) -> Result<Vec<Spawn>, String> {
    let Some(Value::Array(entries)) = object.get("items") else {
        return Err("a group has no items array".to_string());
    };
    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(spawn) = entry else {
            return Err("an item entry is not an object".to_string());
        };
        let (Some(Value::Str(item)), Some(Value::Int(min)), Some(Value::Int(max))) =
            (spawn.get("item"), spawn.get("min"), spawn.get("max"))
        else {
            return Err("an item entry is missing a field".to_string());
        };
        items.push(Spawn {
            item: item.clone(),
            min: i32::try_from(*min).map_err(|_| "min is out of range".to_string())?,
            max: i32::try_from(*max).map_err(|_| "max is out of range".to_string())?,
        });
    }
    Ok(items)
}
