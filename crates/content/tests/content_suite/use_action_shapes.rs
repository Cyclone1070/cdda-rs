//! The shapes a `use_action` entry can take, which are four and not two.
//!
//! Reference: `use_function_reader_helper` (`src/item_factory.cpp:1877-1903`). An entry is a **string** (a legacy id, `"ALCOHOL"`), an **object** (an actor with a
//! `"type"`), or an **array** (`[ "CROWBAR", 0 ]`, which names the action and overrides its `ammo_scale`, `1894-1899`); and the member itself may be a single entry
//! or an array of entries.
//!
//! The array shape is the one that is easy to lose: a reader that only accepts strings and objects drops those entries **without an error**, which leaves a
//! repairkit with a shorter action list than the reference's.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use std::collections::BTreeMap;

fn loaded() -> BTreeMap<String, ItemType> {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect()
}

#[test]
fn an_array_shaped_entry_is_an_action_and_not_a_gap() {
    // Both repairkits state five entries each: `"GUN_REPAIR"`, `["CROWBAR", 0]`, `["HASSAW", 0]`… and two objects. The two array entries carry an `ammo_scale`
    // the port does not model yet, but they are still **actions with a name**, so they belong in the list.
    let items = loaded();
    for id in ["large_repairkit", "small_repairkit"] {
        let item = items.get(id).unwrap_or_else(|| panic!("{id} loads"));
        // **Sorted by name, not in data order, and `repair_item` renamed.** `use_methods` is a `std::map`, so it comes back as `CROWBAR, GUN_REPAIR, HACKSAW, …`
        // (upper case before lower), `link_up` sorts after `REMOVE_ALL_MODS`, and the `repair_item` actor answers to `repair_metal` — its `item_action_type`
        // (`src/item_factory.cpp:1858-1865`). The repairkits also **inherit** `REMOVE_ALL_MODS` and `saw_barrel` from a parent, which is why there are seven.
        let names: Vec<&String> = item.use_actions.keys().collect();
        assert_eq!(
            names,
            vec![
                "CROWBAR",
                "GUN_REPAIR",
                "HACKSAW",
                "REMOVE_ALL_MODS",
                "link_up",
                "repair_metal",
                "saw_barrel"
            ],
            "{id} has all seven of its actions, in the map's own order"
        );
    }
}
