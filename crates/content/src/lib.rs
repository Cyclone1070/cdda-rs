//! Content registry.
//!
//! The reference keeps one registry per JSON `type` — ninety of them in the core
//! content — each holding the concrete prototypes of that type after inheritance
//! has been resolved. This crate starts from the part that can be verified before
//! any merge rule exists: which ids each registry ends up holding.
//!
//! The order of loading is the reference's: files in filesystem order, then the
//! objects inside each file in document order (`filesystem.cpp:332,372-374`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod examine;
pub mod snippets;
pub mod flags;
pub mod ammo_effects;
pub mod catalog;
pub mod blood;
pub mod books;
pub mod comestible;
pub mod condition;
pub mod constructions;
pub mod digest;
pub mod enchantments;
pub mod faults;
pub mod dialogue;
pub mod events;
pub mod effects;
pub mod emit;
pub mod eocs;
pub mod factory;
pub mod fields;
pub mod guns;
pub mod frames;
pub mod generated;
pub mod harvest;
pub mod harvest_drops;
pub mod item_group_draw;
pub mod item_groups;
pub mod items;
pub mod jmath;
pub mod math_expr;
pub mod mapdata;
pub mod mapgen;
pub mod pockets;
pub mod use_actions;
pub mod martial_arts;
pub mod materials;
pub mod morale_types;
pub mod weather;
pub mod weapon_categories;
pub mod mods;
pub mod monster_groups;
pub mod ammo;
pub mod anatomies;
pub mod armour;
pub mod body_parts;
pub mod monsters;
pub mod weakpoints;
pub mod npc_classes;
pub mod overmap;
pub mod proficiencies;
pub mod recipes;
pub mod spells;
pub mod qualities;
pub mod zones;
pub mod requirements;
mod rules;
mod scan;
pub mod starting_kit;
pub mod terrain;
pub mod traps;
pub mod vitamins;
pub mod mutations;
pub mod vehicles;

pub use rules::{id_member, null_prototype, registry_for};
pub use scan::{Registry, ScanError, scan};

/// Every `*.json` file under `dir`, in the reference's load order.
///
/// # Errors
///
/// Returns an error only if the starting directory itself cannot be read; a
/// subdirectory that cannot be opened is skipped, as the reference skips it.
///
/// The reference walks a directory level by level, sorting the entries of each
/// level (`filesystem.cpp:332,372-374`), so directories and files interleave by
/// name rather than files-before-directories.
pub fn json_files_in_load_order(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut ordered = Vec::new();
    let mut level = vec![dir.to_path_buf()];
    while !level.is_empty() {
        let mut next = Vec::new();
        let mut files = Vec::new();
        let mut entries = Vec::new();
        for current in &level {
            let Ok(read) = std::fs::read_dir(current) else {
                continue;
            };
            for entry in read.flatten() {
                entries.push(entry.path());
            }
        }
        entries.sort();
        for path in entries {
            if path.is_dir() {
                next.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                files.push(path);
            }
        }
        ordered.extend(files);
        level = next;
    }
    Ok(ordered)
}

/// The concrete ids a registry holds, keyed by the registry's type name.
pub type IdsByType = BTreeMap<String, Vec<String>>;

/// Reads and parses one JSON file, remembering the result for this process.
///
/// Every loader walks the same 6,220 files, so a gate that loads items and then
/// monsters parses the 107 MB tree twice for nothing. The memo makes the second
/// pass free. It is keyed by path because the content a run sees cannot change
/// underneath it; a caller that needs to see a modified file starts a new process,
/// which is what every gate does anyway.
///
/// The memory is the point: the whole tree costs a few hundred megabytes once,
/// where re-parsing costs seconds every time. `CDDA_NO_PARSE_MEMO=1` turns it off
/// for anyone measuring the parser itself.
pub fn read_json(path: &Path) -> Option<std::sync::Arc<cdda_json::Value>> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::{Arc, OnceLock};

    static DISABLED: OnceLock<bool> = OnceLock::new();
    let disabled = *DISABLED.get_or_init(|| std::env::var_os("CDDA_NO_PARSE_MEMO").is_some());

    thread_local! {
        static MEMO: RefCell<HashMap<PathBuf, Option<Arc<cdda_json::Value>>>> =
            RefCell::new(HashMap::new());
    }

    let bytes = std::fs::read(path).ok()?;
    if disabled {
        return cdda_json::parse(&bytes).ok().map(Arc::new);
    }
    MEMO.with(|memo| {
        if let Some(hit) = memo.borrow().get(path) {
            return hit.clone();
        }
        let parsed = cdda_json::parse(&bytes).ok().map(Arc::new);
        memo.borrow_mut().insert(path.to_path_buf(), parsed.clone());
        parsed
    })
}
pub mod overmap_build;
pub mod overmap_special;
pub mod skills;
pub mod crit_chance;
pub mod swing_cost;
