//! Monster groups: the table a spawn point draws from.
//!
//! Reference: `mongroup.cpp`. A group is a weighted list of monsters - sometimes packs of them - with optional conditions on
//! time of day, season and terrain. The port loads the identity and the weights, which is what a spawn needs; the conditions
//! follow with the spawner that reads them.
//!
//! Groups are keyed by **id**, and the same id can appear in several files: a mod appends to a core group rather than replacing
//! it, so entries accumulate in load order instead of the later object winning.

use std::collections::BTreeMap;

use cdda_json::Value;

use crate::monsters::LoadError;

/// What an entry points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// A monster's id.
    Monster(String),
    /// Another group's id: groups nest, which is how a spawn point can name a broad family and let the finer weights live
    /// in one place.
    Group(String),
}

/// One line of a group: something to spawn, and how often it is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterGroupEntry {
    /// The monster or nested group this entry names.
    pub kind: EntryKind,
    /// How often this entry is chosen relative to the others.
    pub weight: i32,
}

/// A group of monsters a spawn point may pick from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterGroup {
    /// Its id.
    pub id: String,
    /// Its entries, in the order the data gives them.
    pub entries: Vec<MonsterGroupEntry>,
}

/// Loads every monster group under `dirs`.
///
/// # Errors
///
/// Reports a directory that cannot be read.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<MonsterGroup>, LoadError> {
    let mut by_id: BTreeMap<String, Vec<MonsterGroupEntry>> = BTreeMap::new();
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
                let Value::Object(fields) = object else {
                    continue;
                };
                match fields.get("type") {
                    Some(Value::Str(kind)) if kind == "monstergroup" => {}
                    _ => continue,
                }
                let id = match fields.get("id").or_else(|| fields.get("name")) {
                    Some(Value::Str(id)) => id.clone(),
                    _ => continue,
                };
                let entries = by_id.entry(id).or_default();
                let Some(Value::Array(list)) = fields.get("monsters") else {
                    continue;
                };
                for item in list {
                    let Value::Object(entry) = item else {
                        continue;
                    };
                    // An entry names either a monster or another group; the reference reads both from the same list, and a
                    // loader that only understood `monster` silently dropped a third of the data.
                    let kind = match (entry.get("monster"), entry.get("group")) {
                        (Some(Value::Str(monster)), _) => EntryKind::Monster(monster.clone()),
                        (_, Some(Value::Str(group))) => EntryKind::Group(group.clone()),
                        _ => continue,
                    };
                    // A weight is the whole point of the entry; the data always writes one.
                    let weight = match entry.get("weight") {
                        Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(0),
                        _ => 0,
                    };
                    entries.push(MonsterGroupEntry { kind, weight });
                }
            }
        }
    }
    Ok(by_id
        .into_iter()
        .map(|(id, entries)| MonsterGroup { id, entries })
        .collect())
}
