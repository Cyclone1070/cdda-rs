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

/// **Diesel carries its energy, and a material that is not a fuel carries none.** Reference: `materials.json` writes `"fuel_data": { "energy": "35800 kJ" }` for diesel, which
/// is `35,800,000 J` per millilitre - the number `item::fuel_energy` multiplies by the item's volume. Flesh has no fuel data, so it answers `None` rather than zero.
#[test]
fn a_fuel_material_carries_its_energy_and_a_non_fuel_carries_none() {
    let materials = materials_loaded();
    let diesel = materials
        .iter()
        .find(|material| material.id == "diesel")
        .expect("diesel is defined");
    let energy = diesel.fuel_energy_j.expect("**diesel is a fuel**");
    assert!(
        (energy - 35_800_000.0).abs() < 1.0,
        "**35,800 kJ is 35,800,000 J: {energy}**"
    );
    let flesh = materials
        .iter()
        .find(|material| material.id == "flesh")
        .expect("flesh is defined");
    assert_eq!(
        flesh.fuel_energy_j, None,
        "flesh does not burn as fuel in an engine"
    );
}

/// **Diesel's charge carries 28,640 J, from the store rather than from a test constant.** This is [`materials::fuel_energy_per_charge`] fed by the two registries: the
/// material's energy per millilitre (35,800,000) and the item's own `200ml` over a `count` of `250`. `item::fuel_energy` is exactly that product, and `drain_energy`
/// converts joules into charges with it.
#[test]
fn a_fuel_item_and_its_material_give_the_energy_of_one_charge() {
    let items = items_loaded();
    let materials = materials_loaded();
    let diesel = items
        .iter()
        .find(|item| item.id == "diesel")
        .expect("the diesel item is defined");
    assert_eq!(diesel.volume, 200, "200 ml in the stack");
    assert_eq!(diesel.default_charges, 250, "over 250 charges");
    let material = materials
        .iter()
        .find(|material| material.id == "diesel")
        .expect("the diesel material is defined");
    let per_ml = material.fuel_energy_j.expect("**diesel burns**");
    let per_charge = cdda_content::materials::fuel_energy_per_charge(
        per_ml,
        diesel.volume,
        i64::from(diesel.default_charges),
    )
    .expect("a charge has a volume");
    assert!(
        (per_charge - 28_640.0).abs() < 1e-6,
        "**35,800,000 J/ml over 0.8 ml is 28,640 J: {per_charge}**"
    );
}
