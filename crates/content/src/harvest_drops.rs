//! What each harvest **drop type** is judged on: the skills its dissection measures and practises, and the messages a
//! failed attempt shows.
//!
//! Reference: `harvest_drop_type.json` and `butchery_dissect_skill_level` (`butchery.cpp:550-564`). The rule that needs
//! this is the one everybody mistakes: a dissection's skill is **the drop type's own skills** plus the blade, and not the
//! survivor's survival level - so flesh, bone, skin, blood and offal, which name no skills at all, are judged on the blade
//! alone, and a mutagen, which names two, is not.
//!
//! Kept in content because the list is data; the sim only asks what it says.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// The messages a drop type shows when an attempt at it fails, one per kind of job.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FailureMessages {
    /// What a failed field dressing says.
    pub field_dress_fail: Option<String>,
    /// What a failed butchery says.
    pub butcher_fail: Option<String>,
    /// What a failed dissection says.
    pub dissect_fail: Option<String>,
}

/// One harvest drop type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DropType {
    /// The reference's own id, which a harvest entry names as its `type`.
    pub id: String,
    /// The skills a dissection of this drop type is judged on, **in the data's order**.
    pub harvest_skills: Vec<String>,
    /// What a failed attempt at it says.
    pub messages: FailureMessages,
    /// Whether its `drop` names an **item group** rather than one item type.
    ///
    /// Reference: `harvest_drop_type::is_group` (`harvest.h:36-42`), read from the data's `"group": true`. **A group entry drops nothing**: the reference
    /// resolves a drop only `if( entry.type->is_itype() )` (`butchery.cpp:713-716`), and the walk that places items is guarded by `drop != nullptr`
    /// (`:829-914`), so a `mutagen_group` entry passes through the whole loop without placing anything.
    pub group: bool,
}

/// Reads every drop type the given files define, by id.
///
/// # Errors
/// When a file cannot be read or is not the JSON the reference writes.
pub fn load(paths: &[&str]) -> Result<BTreeMap<String, DropType>, String> {
    let mut types = BTreeMap::new();
    for path in paths {
        let parsed = crate::read_json(std::path::Path::new(path))
            .ok_or_else(|| format!("{path}: could not be read"))?;
        let Value::Array(entries) = parsed.as_ref() else {
            return Err(format!("{path}: not an array"));
        };
        for entry in entries.clone() {
            let Value::Object(object) = entry else {
                continue;
            };
            let Some(kind) = object.get("type") else {
                continue;
            };
            if kind != &Value::Str("harvest_drop_type".to_owned()) {
                continue;
            }
            let Some(drop_type) = read_one(&object) else {
                continue;
            };
            types.insert(drop_type.id.clone(), drop_type);
        }
    }
    Ok(types)
}

/// One entry of the file.
fn read_one(object: &Object) -> Option<DropType> {
    let Value::Str(id) = object.get("id")? else {
        return None;
    };
    let harvest_skills = match object.get("harvest_skills") {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|value| match value {
                Value::Str(name) => Some(name.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(DropType {
        id: id.clone(),
        harvest_skills,
        messages: FailureMessages {
            field_dress_fail: text(object, "msg_fielddress_fail"),
            butcher_fail: text(object, "msg_butcher_fail"),
            dissect_fail: text(object, "msg_dissect_fail"),
        },
        group: matches!(object.get("group"), Some(Value::Bool(true))),
    })
}

/// A string field, if the file has one.
fn text(object: &Object, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::Str(value)) => Some(value.clone()),
        _ => None,
    }
}
