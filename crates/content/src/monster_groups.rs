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
    /// What one of this entry costs against a spawner's budget (`cost_multiplier`, `mongroup.cpp:249-254`): the density spawner decrements a shared quantity by
    /// `max( 1, cost_multiplier * pack_size )` for every one it places, which is what stops a dense area from filling with the most expensive monster.
    pub cost_multiplier: i32,
    /// How many of this entry arrive together, as a range (`pack_size`, or the legacy `pack_minimum`/`pack_maximum` pair).
    pub pack: (i32, i32),
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
            collect_into(value.as_ref(), &mut by_id);
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
/// Returns the monster's id as a value, or `None` when the group does not exist or holds nothing to pick.
pub fn pick_monster(group_id: &str, groups: &[MonsterGroup], rolls: &mut Rolls) -> Option<String> {
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
                // The name the entry carries **is** the answer. Searching the table for it again used to fail whenever the pick
                // came from an entry the search could not see - a group whose entries were read from another file, say - and
                // reported "picked nothing" for a group that had picked perfectly well.
                return Some(id);
            }
            EntryKind::Group(id) => {
                current = groups.iter().find(|group| group.id == id)?.id.as_str();
            }
        }
    }
    None
}

/// Picks a monster out of a group **and keeps the entry it came from**, which is what a spawner needs: the pack it arrives in and what it costs against the budget.
///
/// Reference: `MonsterGroupManager::GetResultFromGroup` (`mongroup.cpp:196-254`), whose result carries `pack_size` and whose `quantity` is decremented by
/// `max( 1, cost_multiplier * pack_size )`. [`pick_monster`] answers the name alone because a caller that only needs a name is the common case; this one answers what a
/// spawner needs and resolves a nested group to the entry that finally named a monster.
pub fn pick_entry(
    group_id: &str,
    groups: &[MonsterGroup],
    rolls: &mut Rolls,
) -> Option<(String, i32, (i32, i32))> {
    let mut current = group_id;
    for _ in 0..8 {
        let group = groups.iter().find(|group| group.id == current)?;
        let entries: Vec<(EntryKind, i32)> = group
            .entries
            .iter()
            .map(|entry| (entry.kind.clone(), entry.weight))
            .collect();
        let list = WeightedList::from_entries(entries);
        let kind = list.pick(rolls)?.clone();
        // The entry the pick landed on, by its kind: the first one that names what was picked.
        let chosen = group
            .entries
            .iter()
            .find(|entry| entry.kind == kind)
            .cloned()?;
        match kind {
            EntryKind::Monster(id) => {
                return Some((id, chosen.cost_multiplier, chosen.pack));
            }
            EntryKind::Group(id) => {
                current = groups.iter().find(|group| group.id == id)?.id.as_str();
            }
        }
    }
    None
}

/// Reads groups out of prototypes already in hand, which is how a test or a mod hands them over without a file.
#[must_use]
pub fn load_objects(prototypes: &[Value]) -> Vec<MonsterGroup> {
    let mut by_id: BTreeMap<String, Vec<MonsterGroupEntry>> = BTreeMap::new();
    for prototype in prototypes {
        collect_into(prototype, &mut by_id);
    }
    by_id
        .into_iter()
        .map(|(id, entries)| MonsterGroup { id, entries })
        .collect()
}

/// Adds one file's or one prototype's groups to the table, accumulating by id - a mod appends to a core group rather than replacing it.
fn collect_into(value: &Value, by_id: &mut BTreeMap<String, Vec<MonsterGroupEntry>>) {
    let objects: Vec<&Value> = match value {
        Value::Array(objects) => objects.iter().collect(),
        other => vec![other],
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
            // An entry names either a monster or another group; the reference reads both from the same list (`mongroup.cpp`), and a
            // loader that only understood `monster` silently dropped a third of the data.
            let kind = match (entry.get("monster"), entry.get("group")) {
                (Some(Value::Str(monster)), _) => EntryKind::Monster(monster.clone()),
                (_, Some(Value::Str(group))) => EntryKind::Group(group.clone()),
                _ => continue,
            };
            // An entry without a weight counts as **one**, not zero: the reference reads `mon.get_int( "weight", 1 )` (`mongroup.cpp:496`).
            let weight = match entry.get("weight") {
                Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(1),
                _ => 1,
            };
            // An entry with no cost counts as **one**, and one with no pack arrives **alone** (`mongroup.cpp:496-510`).
            let cost_multiplier = match entry.get("cost_multiplier") {
                Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(1),
                _ => 1,
            };
            let pack = if let Some(Value::Array(pair)) = entry.get("pack_size") {
                let low = match pair.first() {
                    Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(1),
                    _ => 1,
                };
                let high = match pair.get(1) {
                    Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(low),
                    _ => low,
                };
                (low, high)
            } else {
                let low = match entry.get("pack_minimum") {
                    Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(1),
                    _ => 1,
                };
                let high = match entry.get("pack_maximum") {
                    Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(low),
                    _ => low,
                };
                (low, high)
            };
            entries.push(MonsterGroupEntry {
                kind,
                weight,
                cost_multiplier,
                pack,
            });
        }
    }
}

/// Every monster a group names, **flattened**, with nested groups expanded and no weight consideration.
///
/// Reference: `MonsterGroupManager::GetMonstersFromGroup( group, true )` (`mongroup.cpp`), which walks the group's entries and pushes each monster's name once - a nested group
/// contributes all of its own monsters - so a name that two entries both list appears **twice**. That duplication is the rule, not an accident: `jmapgen_corpse` follows the
/// flatten with `random_entry_ref`, a uniform pick over the vector, so the data's weights are not consulted at all on that path.
#[must_use]
pub fn flatten(group_id: &str, groups: &[MonsterGroup]) -> Vec<String> {
    let mut flat = Vec::new();
    let mut stack = vec![group_id.to_owned()];
    // A chain in the data is a few deep; the bound is what stops a group that names itself.
    for _ in 0..8 {
        let Some(current) = stack.pop() else {
            return flat;
        };
        let Some(group) = groups.iter().find(|group| group.id == current) else {
            return flat;
        };
        for entry in &group.entries {
            match &entry.kind {
                EntryKind::Monster(id) => flat.push(id.clone()),
                EntryKind::Group(id) => stack.push(id.clone()),
            }
        }
    }
    flat
}

/// Picks one monster from the flattened list, **uniformly**.
///
/// Reference: `jmapgen_corpse::apply` (`mapgen.cpp:2965-2974`): `GetMonstersFromGroup` then `random_entry_ref( monster_group )`, which is a uniform choice over the vector. A
/// weighted pick here would answer differently from the reference on any group whose entries carry weights, which is most of them.
#[must_use]
pub fn pick_flat(group_id: &str, groups: &[MonsterGroup], rolls: &mut Rolls) -> Option<String> {
    let flat = flatten(group_id, groups);
    if flat.is_empty() {
        return None;
    }
    let roll = rolls
        .rng(0, i32::try_from(flat.len()).unwrap_or(1) - 1)
        .max(0);
    flat.get(usize::try_from(roll).unwrap_or(0)).cloned()
}
