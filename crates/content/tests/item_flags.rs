//! Flags that change what the game does, as opposed to flags that are just names.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::items;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load() -> Vec<items::ItemType> {
    let core = root().join("references/Cataclysm-DDA/data/core");
    let json = root().join("references/Cataclysm-DDA/data/json");
    let dirs: [&Path; 2] = [core.as_path(), json.as_path()];
    items::load_paths(&dirs).expect("items load")
}

#[test]
fn irremovable_is_the_flag_and_nothing_else() {
    let all = load();
    let irremovable: Vec<&str> = all
        .iter()
        .filter(|item| item.is_irremovable())
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        irremovable.len(),
        46,
        "items carrying IRREMOVABLE: {irremovable:?}"
    );
    assert!(
        irremovable.contains(&"efile_lore"),
        "a lore file is part of the computer it sits in"
    );
    let rock = all.iter().find(|item| item.id == "rock").expect("a rock");
    assert!(!rock.is_irremovable(), "a rock can be picked up");
}

#[test]
fn the_port_finds_the_items_a_fire_sets_off() {
    // The reference's dump says 41 of 9,912 items carry `explode_in_fire`.
    let all = load();
    let explodes: Vec<&str> = all
        .iter()
        .filter(|item| item.explode_in_fire)
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        explodes.len(),
        41,
        "items that go off in a fire: {explodes:?}"
    );
    let acetylene = all
        .iter()
        .find(|item| item.id == "acetylene")
        .expect("acetylene");
    assert!(acetylene.will_explode_in_fire());
    let rock = all.iter().find(|item| item.id == "rock").expect("a rock");
    assert!(!rock.will_explode_in_fire());
}

#[test]
fn ammunition_cooks_off_the_way_the_reference_derives_it() {
    // `cookoff` comes from the ammo effects, not from a field (`item_factory.cpp:512-522`): 387 of
    // 9,912 items cook off and 8 do it in their own way.
    let all = load();
    let cooking: Vec<&str> = all
        .iter()
        .filter(|item| item.cookoff)
        .map(|item| item.id.as_str())
        .collect();
    let special: Vec<&str> = all
        .iter()
        .filter(|item| item.special_cookoff)
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        cooking.len(),
        387,
        "cook-off items, first few: {:?}",
        cooking.iter().take(5).collect::<Vec<_>>()
    );
    assert_eq!(special.len(), 8, "special cook-offs: {special:?}");
    let battery = all
        .iter()
        .find(|item| item.id == "battery")
        .expect("a battery");
    assert!(battery.cookoff, "a battery cooks off");
    let rock = all.iter().find(|item| item.id == "rock").expect("a rock");
    assert!(!rock.cookoff && !rock.special_cookoff);
}

#[test]
fn something_made_of_oil_never_cooks_off_however_its_effects_read() {
    // The reference exempts hydrocarbon and oil items outright (`item_factory.cpp:513-521`).
    let all = load();
    for item in &all {
        if item.materials.contains_key("hydrocarbons") || item.materials.contains_key("oil") {
            assert!(!item.cookoff, "{} is oily and must not cook off", item.id);
        }
    }
}
