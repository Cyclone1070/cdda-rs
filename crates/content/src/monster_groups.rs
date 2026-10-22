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

use cdda_core_types::rng::Rolls;
use cdda_core_types::weighted::WeightedList;

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
                    // An entry without a weight counts as **one**, not zero: the reference reads `mon.get_int( "weight", 1 )`
                    // (`mongroup.cpp:496`), and a loader that defaulted to zero dropped those entries out of the raffle
                    // entirely - which is how a group made only of other groups ended up producing nothing at all.
                    let weight = match entry.get("weight") {
                        Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(1),
                        _ => 1,
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

/// Picks a monster out of a group, resolving nested groups.
///
/// Reference: `MonsterGroup::pick` (`mongroup.cpp`). An entry is chosen by weight; if it names another **group** the pick is
/// repeated inside it, up to a bound so malformed data that refers back to itself cannot hang a spawn. Conditions on time,
/// season and terrain are not consulted - the port's spawner will pass only the entries that pass them, which is the shape the
/// reference uses too.
///
/// Returns the monster's id, or `None` when the group does not exist or holds nothing to pick.
pub fn pick_monster<'a>(
    group_id: &str,
    groups: &'a [MonsterGroup],
    rolls: &mut Rolls,
) -> Option<&'a str> {
    let mut current = group_id;
    // A nested chain in the data is two or three deep; eight is a generous ceiling that still terminates on a cycle.
    for _ in 0..8 {
        let group = groups.iter().find(|group| group.id == current)?;
        let entries: Vec<(EntryKind, i32)> = group
            .entries
            .iter()
            .map(|entry| (entry.kind.clone(), entry.weight))
            .collect();
        let list = WeightedList::from_entries(entries);
        let kind = list.pick(rolls)?.clone();
        match kind {
            EntryKind::Monster(id) => {
                // The group table borrows from itself, so the id is looked up rather than returned by value.
                return groups.iter().find_map(|group| {
                    group.entries.iter().find_map(|entry| match &entry.kind {
                        EntryKind::Monster(name) if *name == id => Some(name.as_str()),
                        _ => None,
                    })
                });
            }
            EntryKind::Group(id) => {
                current = groups.iter().find(|group| group.id == id)?.id.as_str();
            }
        }
    }
    None
}
