//! The registry built from the game's own item data.
//!
//! Reference: `item_factory`'s table, filled by `load_item` from the JSON. The port's version is `item_data::install`,
//! and the thing worth gating is that the numbers survive the crossing: a registry that quietly kept names and dropped
//! weights would still let a survivor pick things up, and would be wrong about everything that weighs anything.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_content::items;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The game's item types, read once for every test in this file.
fn the_games_items() -> &'static Vec<items::ItemType> {
    static ONCE: OnceLock<Vec<items::ItemType>> = OnceLock::new();
    ONCE.get_or_init(|| {
        let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
        let dirs = [reference.join("core"), reference.join("json")];
        let args: Vec<PathBuf> = dirs.to_vec();
        let borrowed: Vec<&Path> = args.iter().map(PathBuf::as_path).collect();
        items::load_paths(&borrowed).expect("the item data loads")
    })
}

#[test]
fn every_item_the_game_has_can_be_looked_up_by_name_and_number() {
    let kinds = the_games_items();
    let mut world = World::new();
    cdda_sim::item_data::install(&mut world, kinds, &std::collections::BTreeMap::new());

    assert!(
        kinds.len() > 9_000,
        "the game has {} item types, which is too few to be the real data",
        kinds.len()
    );
    assert_eq!(
        cdda_sim::item_registry::count(&mut world),
        kinds.len(),
        "every item type is in the registry, and no more"
    );
    // The first and last, by number, are the first and last in the data's order - which is how the reference numbers
    // them too.
    let first = kinds.first().expect("the data is not empty");
    let last = kinds.last().expect("the data has a last");
    assert_eq!(
        cdda_sim::item_registry::name_of(&mut world, cdda_sim::ItemTypeId(1)).as_deref(),
        Some(first.id.as_str())
    );
    let last_number = u32::try_from(kinds.len()).expect("fewer items than a u32 can count");
    assert_eq!(
        cdda_sim::item_registry::id_of(&mut world, &last.id),
        Some(cdda_sim::ItemTypeId(last_number)),
        "the last item is numbered last"
    );
}

#[test]
fn an_items_weight_and_volume_cross_over_unchanged() {
    let kinds = the_games_items();
    let mut world = World::new();
    cdda_sim::item_data::install(&mut world, kinds, &std::collections::BTreeMap::new());

    // Every single-charge item keeps its weight and volume. Items that come in charges are exempt: their stated volume
    // is for one charge and the registry stores one, which is the same number.
    let mut checked = 0;
    for (index, kind) in kinds.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let number = cdda_sim::ItemTypeId(1 + index as u32);
        assert_eq!(
            cdda_sim::item_registry::volume_of(&mut world, number),
            i32::try_from(kind.volume).unwrap_or(i32::MAX),
            "volume of {} did not survive",
            kind.id
        );
        assert_eq!(
            cdda_sim::item_registry::weight_of(&mut world, number),
            kind.weight,
            "weight of {} did not survive",
            kind.id
        );
        checked += 1;
    }
    assert!(checked > 9_000, "checked {checked} items");
}

#[test]
fn the_things_that_hold_things_still_hold_them() {
    let kinds = the_games_items();
    let mut world = World::new();
    cdda_sim::item_data::install(&mut world, kinds, &std::collections::BTreeMap::new());

    let mut with_pockets = 0;
    let mut biggest: Option<(String, i32)> = None;
    for (index, kind) in kinds.iter().enumerate() {
        if kind.pockets.is_empty() {
            continue;
        }
        with_pockets += 1;
        #[allow(clippy::cast_possible_truncation)]
        let number = cdda_sim::ItemTypeId(1 + index as u32);
        let held = cdda_sim::item_registry::holds_ml(&mut world, number);
        let expected: i32 = kind
            .pockets
            .iter()
            .map(|pocket| i32::try_from(pocket.max_volume_ml).unwrap_or(i32::MAX))
            .sum();
        assert_eq!(held, expected, "{} holds what its pockets say", kind.id);
        if biggest.as_ref().is_none_or(|(_, most)| held > *most) {
            biggest = Some((kind.id.clone(), held));
        }
    }
    assert!(
        with_pockets > 500,
        "only {with_pockets} items hold anything, which cannot be the real data"
    );
    let (id, held) = biggest.expect("something holds something");
    assert!(
        held > 10_000,
        "the roomiest thing is {id} at {held} ml, which is smaller than any real container"
    );
}
