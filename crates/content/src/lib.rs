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

pub mod activity_types;
pub mod ammo;
pub mod ammo_effects;
pub mod anatomies;
pub mod armour;
pub mod blood;
pub mod body_parts;
pub mod books;
pub mod catalog;
pub mod comestible;
pub mod condition;
pub mod constructions;
pub mod dialogue;
pub mod digest;
pub mod effects;
pub mod emit;
pub mod enchantments;
pub mod eocs;
pub mod events;
pub mod examine;
pub mod factory;
pub mod faults;
pub mod fields;
pub mod flags;
pub mod frames;
pub mod generated;
pub mod guns;
pub mod harvest;
pub mod harvest_drops;
pub mod item_damage;
pub mod item_group_draw;
pub mod item_groups;
pub mod items;
pub mod jmath;
pub mod mapdata;
pub mod mapgen;
pub mod martial_arts;
pub mod materials;
pub mod math_expr;
pub mod mission_types;
pub mod mods;
pub mod monster_groups;
pub mod monsters;
pub mod morale_types;
pub mod mutations;
pub mod npc_classes;
pub mod npc_templates;
pub mod overmap;
pub mod pockets;
pub mod proficiencies;
pub mod qualities;
pub mod recipes;
pub mod requirements;
mod rules;
mod scan;
pub mod snippets;
pub mod spells;
pub mod starting_kit;
pub mod terrain;
pub mod traps;
pub mod use_actions;
pub mod vehicle_groups;
pub mod vehicles;
pub mod vitamins;
pub mod weakpoints;
pub mod weapon_categories;
pub mod weather;
pub mod zone_types;
pub mod zones;

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
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    // **The memo is the process's, not the thread's.** It was a `thread_local!` while this doc said "this process", so the gate's test threads each parsed the
    // 6,220-file tree for themselves, and the suite spent its time repeating work the memo was written to avoid. One lock, held only for the map operation and never
    // across a parse, so two threads reading two different files still parse in parallel.
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, Option<Arc<cdda_json::Value>>>>> = OnceLock::new();
    static DISABLED: OnceLock<bool> = OnceLock::new();
    let disabled = *DISABLED.get_or_init(|| std::env::var_os("CDDA_NO_PARSE_MEMO").is_some());

    let bytes = std::fs::read(path).ok()?;
    if disabled {
        return cdda_json::parse(&bytes).ok().map(Arc::new);
    }
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = memo.lock().ok()?.get(path).cloned() {
        return hit;
    }
    let parsed = cdda_json::parse(&bytes).ok().map(Arc::new);
    PARSES.with(|count| count.set(count.get() + 1));
    if let Ok(mut held) = memo.lock() {
        held.insert(path.to_path_buf(), parsed.clone());
    }
    parsed
}

// Per thread, because a test asserting "my second call parsed nothing" should not be counting the tests running beside it.
std::thread_local! {
    static PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many files **this thread** has parsed through [`read_json`].
///
/// The memo's whole worth is the difference between this number and the number of calls, and a test is the only caller.
#[must_use]
pub fn parse_count() -> usize {
    PARSES.with(std::cell::Cell::get)
}
pub mod crit_chance;
pub mod overmap_build;
pub mod overmap_connection;
pub mod overmap_special;
pub mod skills;
pub mod swing_cost;
