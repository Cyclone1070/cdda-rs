//! Overmap terrain: one entry per tile kind, and the variants each kind makes.
//!
//! Reference: `oter_type_t::load` (`src/overmap.cpp:880-975`),
//! `oter_type_t::finalize` (`src/overmap.cpp:987-1005`) and the rotation rule
//! `src/omdata.h:372`.
//!
//! Two registries come out of one set of JSON entries, which is why they are
//! counted separately:
//!
//! * a **type** per distinct id - what a map generator names;
//! * a **terrain** per variant - what a map cell holds. A rotatable type makes
//!   four of them, one per facing; a plain one makes just itself.
//!
//! The rule was measured against the reference's own dump before this loader was
//! written: [docs/15-overmap-terrain-ids.md](../../docs/15-overmap-terrain-ids.md).

use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The directions a rotatable type is registered under, in the reference's order
/// (`om_direction::all`, `src/omdata.h`).
pub const DIRECTIONS: [&str; 4] = ["north", "east", "south", "west"];

/// The sixteen shapes a line-drawing terrain registers, in the reference's order.
///
/// Reference: `om_lines::all` (`src/overmap.cpp:266-287`). Each entry is the shape
/// a tile takes for one combination of neighbours that connect to it - isolated,
/// a dead end in each of the four directions, straight runs, corners, tees and the
/// four-way - and the suffix is appended to the type's id. The order is the
/// reference's because the index is also the shape's identity when a tile is asked
/// for its connected variant.
pub const LINE_SUFFIXES: [&str; 16] = [
    "_isolated",
    "_end_south",
    "_end_west",
    "_ne",
    "_end_north",
    "_ns",
    "_es",
    "_nes",
    "_end_east",
    "_wn",
    "_ew",
    "_new",
    "_sw",
    "_nsw",
    "_esw",
    "_nesw",
];

/// The flags that decide whether a type rotates
/// (`src/omdata.h:372`, `src/omdata.h:377`).
const NO_ROTATE: &str = "NO_ROTATE";
const LINEAR: &str = "LINEAR";

/// One overmap terrain type, with the fields this loader needs so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainType {
    /// Its id.
    pub id: String,
    /// Its resolved flags.
    pub flags: BTreeSet<String>,
    /// A tile that is the same everywhere, so it never rotates
    /// (`src/overmap.cpp:937-939`).
    pub uniform: bool,
}

impl TerrainType {
    /// Whether this type registers four rotated variants.
    ///
    /// A uniform tile is checked first because the reference returns from the
    /// loader entirely for one (`src/overmap.cpp:937-939`), before it ever looks
    /// at the flags.
    #[must_use]
    pub fn rotatable(&self) -> bool {
        !self.uniform && !self.flags.contains(NO_ROTATE) && !self.flags.contains(LINEAR)
    }

    /// Whether this type draws itself from its neighbours.
    #[must_use]
    pub fn linear(&self) -> bool {
        self.flags.contains(LINEAR)
    }
}

/// What a load produced: the types, every terrain id they register, and the
/// overmap specials.
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    /// The types, in the order their first definition was read.
    pub types: Vec<TerrainType>,
    /// Every terrain id, sorted.
    pub terrains: BTreeSet<String>,
    /// Every overmap special id, sorted.
    ///
    /// A `city_building` is an alias for an overmap special - the reference's
    /// `city_buildings::load` is literally `overmap_specials::load`
    /// (`src/overmap.cpp:540-544`) - so both types land in one namespace here.
    pub specials: BTreeSet<String>,
}

/// One raw definition, before inheritance is resolved.
#[derive(Debug, Default)]
struct Raw {
    own_flags: BTreeSet<String>,
    deleted_flags: BTreeSet<String>,
    copied_from: Option<String>,
    uniform: bool,
}

/// Loads every `overmap_terrain` entry under the given roots.
///
/// # Errors
///
/// Returns an error when a file cannot be read; a file that does not parse is
/// skipped, as the reference skips it.
pub fn load_paths(roots: &[&Path]) -> Result<Loaded, String> {
    let mut raw: BTreeMap<String, Raw> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut specials: BTreeSet<String> = BTreeSet::new();

    for root in roots {
        let files = crate::json_files_in_load_order(root).map_err(|error| error.to_string())?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let entries = match value.as_ref() {
                Value::Array(entries) => entries.clone(),
                other => vec![other.clone()],
            };
            for entry in entries {
                let Value::Object(object) = &entry else {
                    continue;
                };
                match object.get("type") {
                    Some(Value::Str(kind)) if kind == "overmap_terrain" => {
                        collect(object, &mut raw, &mut order);
                    }
                    Some(Value::Str(kind))
                        if kind == "overmap_special" || kind == "city_building" =>
                    {
                        collect_special(object, &mut specials);
                    }
                    _ => {}
                }
            }
        }
    }

    let types = resolve(&raw, &order);
    let terrains = register_terrains(&types);
    Ok(Loaded {
        types,
        terrains,
        specials,
    })
}

/// Records the ids of one overmap special or city building.
///
/// The reference reads both through the same loader (`city_buildings::load` is an
/// alias for `overmap_specials::load`, `src/overmap.cpp:540-544`), so an id
/// defined as either is the same id here.
fn collect_special(object: &cdda_json::Object, specials: &mut BTreeSet<String>) {
    match object.get("id") {
        Some(Value::Str(id)) => {
            specials.insert(id.clone());
        }
        Some(Value::Array(ids)) => {
            for id in ids {
                if let Value::Str(text) = id {
                    specials.insert(text.clone());
                }
            }
        }
        _ => {}
    }
}

fn collect(object: &cdda_json::Object, raw: &mut BTreeMap<String, Raw>, order: &mut Vec<String>) {
    let ids = match object.get("id") {
        Some(Value::Str(id)) => vec![id.clone()],
        Some(Value::Array(ids)) => ids
            .iter()
            .filter_map(|id| match id {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        // An `abstract` is a parent others copy from, never registered itself
        // (`src/overmap.cpp:966`).
        _ => Vec::new(),
    };
    let abstract_id = match object.get("abstract") {
        Some(Value::Str(id)) => Some(id.clone()),
        _ => None,
    };

    let mut own = BTreeSet::new();
    if let Some(Value::Array(flags)) = object.get("flags") {
        for flag in flags {
            if let Value::Str(text) = flag {
                own.insert(text.clone());
            }
        }
    }
    let mut deleted = BTreeSet::new();
    if let Some(Value::Object(delete)) = object.get("delete")
        && let Some(Value::Array(flags)) = delete.get("flags")
    {
        for flag in flags {
            if let Value::Str(text) = flag {
                deleted.insert(text.clone());
            }
        }
    }
    if let Some(Value::Object(extend)) = object.get("extend")
        && let Some(Value::Array(flags)) = extend.get("flags")
    {
        for flag in flags {
            if let Value::Str(text) = flag {
                own.insert(text.clone());
            }
        }
    }
    let copied_from = match object.get("copy-from") {
        Some(Value::Str(parent)) => Some(parent.clone()),
        _ => None,
    };
    let uniform = matches!(object.get("uniform_terrain"), Some(Value::Str(_)));

    let targets: Vec<(String, bool)> = ids
        .into_iter()
        .map(|id| (id, false))
        .chain(abstract_id.map(|id| (id, true)))
        .collect();

    for (id, is_abstract) in targets {
        if !is_abstract && !order.contains(&id) {
            order.push(id.clone());
        }
        let entry = raw.entry(id).or_default();
        entry.own_flags.extend(own.iter().cloned());
        entry.deleted_flags.extend(deleted.iter().cloned());
        entry.uniform = entry.uniform || uniform;
        if copied_from.is_some() {
            entry.copied_from.clone_from(&copied_from);
        }
        if is_abstract {
            // Abstracts are indexed the same way; `order` decides who registers.
            entry.uniform = entry.uniform || uniform;
        }
    }
}

/// Resolves flags through the `copy-from` chain, applying `delete` last.
///
/// The order matters and cost a round to learn: `kelp_forest_end` inherits
/// `NO_ROTATE` from its parent and then deletes it, so a resolver that applies the
/// delete before the inheritance puts the flag back and gets the opposite answer
/// ([docs/15-overmap-terrain-ids.md](../../docs/15-overmap-terrain-ids.md)).
fn resolve(raw: &BTreeMap<String, Raw>, order: &[String]) -> Vec<TerrainType> {
    let mut resolved: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let resolve_one = |id: &str, resolved: &mut BTreeMap<String, BTreeSet<String>>| {
        let mut stack = vec![id.to_owned()];
        while let Some(current) = stack.last().cloned() {
            let Some(entry) = raw.get(&current) else {
                stack.pop();
                continue;
            };
            let parent_ready = entry
                .copied_from
                .as_deref()
                .is_none_or(|parent| resolved.contains_key(parent));
            if !parent_ready
                && stack.len() < 64
                && let Some(parent) = entry.copied_from.as_deref()
            {
                stack.push(parent.to_owned());
                continue;
            }
            let mut flags = BTreeSet::new();
            if let Some(parent) = entry.copied_from.as_deref()
                && let Some(inherited) = resolved.get(parent)
            {
                flags.clone_from(inherited);
            }
            flags.extend(entry.own_flags.iter().cloned());
            for flag in &entry.deleted_flags {
                flags.remove(flag);
            }
            resolved.insert(current.clone(), flags);
            stack.pop();
        }
    };
    for id in order {
        resolve_one(id, &mut resolved);
    }

    order
        .iter()
        .filter_map(|id| {
            let entry = raw.get(id)?;
            Some(TerrainType {
                id: id.clone(),
                flags: resolved.get(id).cloned().unwrap_or_default(),
                uniform: entry.uniform,
            })
        })
        .collect()
}

/// Registers every terrain id the given types own.
///
/// The empty id is in the reference's registry because its null id is a real key
/// there, so it is a real key here too.
fn register_terrains(types: &[TerrainType]) -> BTreeSet<String> {
    let mut terrains = BTreeSet::new();
    terrains.insert(String::new());
    for kind in types {
        if kind.rotatable() {
            for direction in DIRECTIONS {
                terrains.insert(format!("{}_{direction}", kind.id));
            }
        } else if kind.linear() {
            // A line-drawing type registers only its connected shapes, never the
            // bare id: the reference's `finalize` takes this branch instead of the
            // rotation one (`src/overmap.cpp:992-1000`).
            for suffix in LINE_SUFFIXES {
                terrains.insert(format!("{}{suffix}", kind.id));
            }
        } else {
            terrains.insert(kind.id.clone());
        }
    }
    terrains
}
