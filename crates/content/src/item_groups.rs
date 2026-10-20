//! Item groups: the named sets a map or a monster spawns items from.
//!
//! Reference: `item_group` (`src/item_group.cpp`), reached through `--dump-item-groups`. The
//! loader starts with the ids, because a group is only ever referred to by name: a spawn entry
//! that names a group which does not exist spawns nothing, and no count of items will reveal it.

use cdda_json::Value;

/// Every item group id the reference defines, in the dump's order.
///
/// # Errors
///
/// Returns the reason the dump could not be read: a missing member, a member of the wrong kind,
/// or a count that is not a non-negative integer.
pub fn from_dump(value: &Value) -> Result<(Vec<String>, usize), String> {
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
        let Value::Str(id) = entry else {
            return Err("a group id is not a string".to_string());
        };
        groups.push(id.clone());
    }
    Ok((groups, count))
}
