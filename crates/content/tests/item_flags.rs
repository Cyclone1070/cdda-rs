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
fn ammunition_cookoff_is_not_ported_yet_and_says_so() {
    // `cookoff` is derived from ammo effects (`src/item_factory.cpp:512-522`), which the port does
    // not read: 387 items cook off in the reference and none of them do here yet.
    let all = load();
    assert!(
        all.iter()
            .all(|item| !item.cookoff && !item.special_cookoff),
        "the fields exist but nothing sets them"
    );
}
