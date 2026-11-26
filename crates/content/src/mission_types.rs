//! The missions the data defines.
//!
//! Reference: `mission_type` (`src/mission.h:39-140`) and its loader `mission_type::load` (`src/mission.cpp`). The reference's type carries the mission's **programs** - `start`,
//! `end` and `fail`, each a list of effects that runs when the mission is taken, finished or lost - plus the goal, the value and the follow-up. The port reads the data's own
//! `mission_definition` objects, the way mapgen is read, because a mission type is content rather than a behaviour: no rule runs at load time.
//!
//! **What this loader reads, and what it deliberately does not.** It reads the fields a rule needs **now** - the id, the name, the description, the goal, the difficulty, the
//! value, the follow-up and the origins. It **does not** read `start`, `end` or `fail`, because those are programs of effects and the port runs effects through the EOC
//! interpreter; reading them into a second shape would be a second source of truth for what an effect is. A mission that has them keeps them in the JSON, and the effect
//! runner is what will read them when the mission actually starts or finishes.

use std::collections::BTreeMap;

use cdda_json::{Object, Value};

/// One mission the data defines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MissionType {
    /// Its id, which is what `assign_mission`, `Missions` and `Offers` all speak.
    pub id: String,
    /// The name a player sees, untranslated.
    pub name: String,
    /// The description a player reads, untranslated.
    pub description: String,
    /// The goal's id - `MGOAL_KILL_MONSTER`, `MGOAL_FIND_ITEM`, ... (`mission.h:23-33`).
    pub goal: String,
    /// How hard the reference thinks it is: `mission_type::difficulty`.
    pub difficulty: i32,
    /// What it is worth in the reference's money.
    pub value: i32,
    /// The mission this one leads to when it is wrapped up, if any.
    pub follow_up: Option<String>,
    /// Where a mission may come from - `ORIGIN_GAME_START`, `ORIGIN_OPENER_NPC`, ... (`mission.h:140-150`).
    pub origins: Vec<String>,
}

/// Every mission the data defines, by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Missions {
    /// The missions, sorted by id.
    pub kinds: Vec<MissionType>,
    /// Where each id sits in `kinds`.
    pub by_id: BTreeMap<String, usize>,
}

impl Missions {
    /// The mission with that id, if the data defines one.
    #[must_use]
    pub fn named(&self, id: &str) -> Option<&MissionType> {
        self.by_id.get(id).and_then(|at| self.kinds.get(*at))
    }
}

/// Reads every `mission_definition` under the given data roots.
///
/// # Errors
///
/// Returns an error when a root cannot be walked, the same way the mapgen loader does.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Missions, crate::generated::LoadError> {
    let mut missions = Missions::default();
    for dir in dirs {
        let files =
            crate::json_files_in_load_order(dir).map_err(|error| crate::generated::LoadError {
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
                let Value::Object(object) = object else {
                    continue;
                };
                if let Some(kind) = read_object(object) {
                    // The first definition of an id wins, which is how the reference's own factory behaves: a later one is a duplicate.
                    if !missions.by_id.contains_key(&kind.id) {
                        missions.by_id.insert(kind.id.clone(), missions.kinds.len());
                        missions.kinds.push(kind);
                    }
                }
            }
        }
    }
    // Sorted once at the end, and the index rebuilt with it, so a mission's position is always its place in `kinds`.
    missions.kinds.sort_by(|left, right| left.id.cmp(&right.id));
    missions.by_id.clear();
    for (at, kind) in missions.kinds.iter().enumerate() {
        missions.by_id.insert(kind.id.clone(), at);
    }
    Ok(missions)
}

/// Reads one JSON object as a mission, or nothing when it is not one.
#[must_use]
pub fn read_object(object: &Object) -> Option<MissionType> {
    let Some(Value::Str(kind)) = object.get("type") else {
        return None;
    };
    if kind != "mission_definition" {
        return None;
    }
    let Some(Value::Str(id)) = object.get("id") else {
        return None;
    };
    let name = object.get("name").map(text_of).unwrap_or_default();
    let description = object.get("description").map(text_of).unwrap_or_default();
    let goal = match object.get("goal") {
        Some(Value::Str(goal)) => goal.clone(),
        _ => String::new(),
    };
    let number = |key: &str| match object.get(key) {
        Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
        _ => 0,
    };
    let follow_up = match object.get("followup") {
        Some(Value::Str(next)) => Some(next.clone()),
        _ => None,
    };
    let origins = match object.get("origins") {
        Some(Value::Array(entries)) => entries
            .iter()
            .filter_map(|entry| match entry {
                Value::Str(origin) => Some(origin.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(MissionType {
        id: id.clone(),
        name,
        description,
        goal,
        difficulty: number("difficulty"),
        value: number("value"),
        follow_up,
        origins,
    })
}

/// A mission's text field, which the data may give as a plain string or as a `{ "str": ... }` object.
fn text_of(value: &Value) -> String {
    match value {
        Value::Str(text) => text.clone(),
        Value::Object(object) => match object.get("str") {
            Some(Value::Str(text)) => text.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
