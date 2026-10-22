//! The reference content, loaded once for the whole suite.
//!
//! Every case that checks the port against the reference has to load it first, and loading the item registry takes
//! about seven seconds on this machine. With a binary per file that cost was paid per file; with one binary per
//! crate it can be paid once, which is what this module is for.
//!
//! Nothing here is a shortcut around the loaders: each accessor calls exactly the loader a case would have called,
//! with the same directories, and remembers the answer. What comes back is a copy of the remembered registry rather
//! than a borrow of it, because a copy of nine thousand items costs a fraction of a second while loading them costs
//! seven - and the callers stay ordinary code that owns what it was given.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use cdda_content::{eocs, items, materials, monsters, overmap, traps};

/// The reference checkout's data directory.
#[must_use]
pub fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data")
}

/// The two directories every loader reads, in the order the reference reads them.
#[must_use]
pub fn dirs() -> [PathBuf; 2] {
    [data().join("core"), data().join("json")]
}

fn paths() -> Vec<&'static Path> {
    // Leaked once on purpose: the loaders take borrowed paths, and these live as long as the process.
    static PATHS: OnceLock<Vec<&'static Path>> = OnceLock::new();
    PATHS
        .get_or_init(|| {
            dirs()
                .iter()
                .map(|path| &*Box::leak(path.clone().into_boxed_path()))
                .collect()
        })
        .clone()
}

/// Every item the reference defines.
#[must_use]
pub fn items_loaded() -> Vec<items::ItemType> {
    static ONCE: OnceLock<Vec<items::ItemType>> = OnceLock::new();
    ONCE.get_or_init(|| items::load_paths(&paths()).expect("items load"))
        .clone()
}

/// Every monster the reference defines.
#[must_use]
pub fn monsters_loaded() -> Vec<monsters::MonsterType> {
    static ONCE: OnceLock<Vec<monsters::MonsterType>> = OnceLock::new();
    ONCE.get_or_init(|| monsters::load_paths(&paths()).expect("monsters load"))
        .clone()
}

/// Every material the reference defines.
#[must_use]
pub fn materials_loaded() -> Vec<materials::Material> {
    static ONCE: OnceLock<Vec<materials::Material>> = OnceLock::new();
    ONCE.get_or_init(|| materials::load_paths(&paths()).expect("materials load"))
        .clone()
}

/// Every overmap terrain the reference defines.
#[must_use]
pub fn overmap_loaded() -> overmap::Loaded {
    static ONCE: OnceLock<overmap::Loaded> = OnceLock::new();
    ONCE.get_or_init(|| overmap::load_paths(&paths()).expect("the overmap terrain files load"))
        .clone()
}

/// Every effect-on-condition the reference defines.
#[must_use]
pub fn eocs_loaded() -> eocs::Loaded {
    static ONCE: OnceLock<eocs::Loaded> = OnceLock::new();
    ONCE.get_or_init(|| eocs::load_paths(&paths()).expect("eocs load"))
        .clone()
}

/// Every inline enchantment in the data tree: the ones no file names.
#[must_use]
pub fn inline_enchantments_loaded() -> Vec<String> {
    static ONCE: OnceLock<Vec<String>> = OnceLock::new();
    ONCE.get_or_init(|| {
        let mut all = std::collections::BTreeSet::new();
        for root in dirs() {
            let Ok(files) = cdda_content::json_files_in_load_order(&root) else {
                continue;
            };
            for path in files {
                let Some(value) = cdda_content::read_json(&path) else {
                    continue;
                };
                all.extend(cdda_content::enchantments::inline_ids(&value));
            }
        }
        all.into_iter().collect()
    })
    .clone()
}

/// The vehicle dump, which is the only list of parts the reference ever writes down.
#[must_use]
pub fn vehicles_dump_loaded() -> cdda_content::vehicles::Vehicles {
    static ONCE: OnceLock<cdda_content::vehicles::Vehicles> = OnceLock::new();
    ONCE.get_or_init(|| {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicles.json");
        let text = std::fs::read_to_string(&path).expect("the vehicle dump");
        let value = cdda_json::parse(text.as_bytes()).expect("it parses");
        cdda_content::vehicles::from_dump(&value).expect("vehicles load")
    })
    .clone()
}

/// Every NPC class the reference defines.
#[must_use]
pub fn npc_classes_loaded() -> Vec<cdda_content::npc_classes::NpcClass> {
    static ONCE: OnceLock<Vec<cdda_content::npc_classes::NpcClass>> = OnceLock::new();
    ONCE.get_or_init(|| cdda_content::npc_classes::load_paths(&paths()).expect("npc classes"))
        .clone()
}

/// Every harvest list the reference defines, gathered the way its loader walks the tree.
#[must_use]
pub fn harvests_loaded()
-> std::collections::BTreeMap<String, cdda_content::harvest::HarvestDefinition> {
    static ONCE: OnceLock<
        std::collections::BTreeMap<String, cdda_content::harvest::HarvestDefinition>,
    > = OnceLock::new();
    ONCE.get_or_init(|| {
        let mut all = std::collections::BTreeMap::new();
        for root in paths() {
            let Ok(files) = cdda_content::json_files_in_load_order(root) else {
                continue;
            };
            for path in files {
                let Some(value) = cdda_content::read_json(&path) else {
                    continue;
                };
                for (id, harvest) in cdda_content::harvest::load_harvests(&value) {
                    all.insert(id, harvest);
                }
            }
        }
        all
    })
    .clone()
}

/// Every trap the reference defines.
#[must_use]
pub fn traps_loaded() -> Vec<traps::Trap> {
    static ONCE: OnceLock<Vec<traps::Trap>> = OnceLock::new();
    ONCE.get_or_init(|| traps::load_paths(&paths()).expect("traps load"))
        .clone()
}
