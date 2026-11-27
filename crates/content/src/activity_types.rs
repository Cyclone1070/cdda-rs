//! What an activity is, as the data defines it.
//!
//! Reference: `activity_type` (`activity_type.cpp:88-137`, `activity_type.h`). An activity is a **name plus a few facts**: the verb the player is told, what its moves are counted
//! in, whether it can be resumed, whether it is a **multiple** activity that keeps looking for more work, and whether it roots the creature in place.
//!
//! **`based_on` is the field that matters most to a simulation**: `speed` means the activity eats the creature's own moves, `time` means a hundred moves a turn whatever the
//! creature is, and `neither` means the **actor** spends what it spends - the third answer the port's `Basis` did not have until this loader existed.

use std::collections::BTreeMap;

use cdda_json::Value;

use crate::monsters::LoadError;

/// One activity definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityType {
    /// Its id, as the data writes it.
    pub id: String,
    /// The verb the player is told they are doing, e.g. `"butchering"`.
    pub verb: String,
    /// What its moves are counted in: `"speed"`, `"time"` or `"neither"`.
    ///
    /// **The default is `"speed"`**, which is the reference's own (`based_on_type based_on_ = based_on_type::SPEED`, `activity_type.h:44`); every object in the data names one,
    /// so the default is only the answer for a definition that omits it.
    pub based_on: String,
    /// Whether the activity can be picked up again after an interruption. **The reference defaults this to `true`** (`activity_type.h:45`), and 59 of the 138 activities in
    /// the data rely on that default.
    pub can_resume: bool,
    /// Whether it is a **multiple** activity: one that keeps looking for more work instead of finishing after one job.
    pub multi_activity: bool,
    /// Whether it roots the creature in place, so it cannot walk away while doing it.
    pub rooted: bool,
}

/// Every activity the data defines, by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityTypes {
    /// In load order, one per id.
    pub kinds: Vec<ActivityType>,
    /// The same ones, by id.
    pub by_id: BTreeMap<String, ActivityType>,
}

/// Loads every activity under `dirs`.
///
/// # Errors
///
/// Reports a file that cannot be read or an activity without an id.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<ActivityTypes, LoadError> {
    let mut by_id: BTreeMap<String, ActivityType> = BTreeMap::new();
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
                let Some(activity) = read_activity(object) else {
                    continue;
                };
                by_id.insert(activity.id.clone(), activity);
            }
        }
    }
    let kinds = by_id.values().cloned().collect();
    Ok(ActivityTypes { kinds, by_id })
}

fn read_activity(object: &Value) -> Option<ActivityType> {
    let Value::Object(fields) = object else {
        return None;
    };
    match fields.get("type") {
        Some(Value::Str(kind)) if kind == "activity_type" => {}
        _ => return None,
    }
    let Some(Value::Str(id)) = fields.get("id") else {
        return None;
    };
    Some(ActivityType {
        id: id.clone(),
        verb: match fields.get("verb") {
            Some(Value::Str(verb)) => verb.clone(),
            _ => String::new(),
        },
        based_on: match fields.get("based_on") {
            Some(Value::Str(based_on)) => based_on.clone(),
            _ => "speed".to_owned(),
        },
        can_resume: !matches!(fields.get("can_resume"), Some(Value::Bool(false))),
        multi_activity: matches!(fields.get("multi_activity"), Some(Value::Bool(true))),
        rooted: matches!(fields.get("rooted"), Some(Value::Bool(true))),
    })
}
