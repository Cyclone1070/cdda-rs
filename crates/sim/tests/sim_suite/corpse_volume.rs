//! What a corpse takes up, for every creature the reference has, plain and butchered four ways.
//!
//! `corpus/creatures/corpse-volume.json` is `--dump-corpse-volume`: 1,154 creatures, and for each the volume of its corpse as `item::volume` reports it — plain, and
//! with `QUARTERED`, `FIELD_DRESS`, `FIELD_DRESS_FAILED`, `GIBBED` and `SKINNED`, plus all four at once.
//!
//! The rule is small and the order is everything: a quarter, then three quarters, then 0.85 twice, **truncating at every step** because the arithmetic is on whole
//! millilitres — and a creature bigger than a tile still leaves a corpse capped at one tile's volume. Seven creatures in the corpus are over that cap, which is the
//! only reason a plain corpse ever differs from the creature's own volume.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_json::Value;
use cdda_sim::{
    CorpseOf, ItemFlags, ItemTypeId, MonsterKindId, TypeVolume, corpse_volume, drop_item,
    item_volume,
};
use std::collections::BTreeSet;

/// One recorded creature: its own volume and the seven corpse volumes.
struct Recorded {
    id: String,
    monster_volume: i64,
    plain: i64,
    quartered: i64,
    dressed: i64,
    dressed_badly: i64,
    gibbed: i64,
    skinned: i64,
    all: i64,
}

fn dump() -> Vec<Recorded> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/creatures/corpse-volume.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(creatures)) = root.get("creatures") else {
        panic!("the dump holds creatures")
    };
    creatures
        .iter()
        .map(|creature| {
            let Value::Object(creature) = creature else {
                panic!("a creature")
            };
            let whole = |key: &str| match creature.get(key) {
                Some(Value::Int(value)) => *value,
                other => panic!("{key} is a whole number: {other:?}"),
            };
            let Some(Value::Str(id)) = creature.get("id") else {
                panic!("an id")
            };
            Recorded {
                id: id.clone(),
                monster_volume: whole("monster_volume"),
                plain: whole("plain"),
                quartered: whole("quartered"),
                dressed: whole("dressed"),
                dressed_badly: whole("dressed_badly"),
                gibbed: whole("gibbed"),
                skinned: whole("skinned"),
                all: whole("all"),
            }
        })
        .collect()
}

fn flags(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_corpse_takes_up_what_the_reference_says_it_does() {
    let creatures = dump();
    assert_eq!(creatures.len(), 1_154, "every creature was measured");

    let mut differing: Vec<String> = Vec::new();
    let mut capped = 0;
    for creature in &creatures {
        if creature.plain != creature.monster_volume {
            capped += 1;
        }
        let cases: [(&str, i64, BTreeSet<String>); 7] = [
            ("plain", creature.plain, flags(&[])),
            ("quartered", creature.quartered, flags(&["QUARTERED"])),
            ("dressed", creature.dressed, flags(&["FIELD_DRESS"])),
            (
                "dressed_badly",
                creature.dressed_badly,
                flags(&["FIELD_DRESS_FAILED"]),
            ),
            ("gibbed", creature.gibbed, flags(&["GIBBED"])),
            ("skinned", creature.skinned, flags(&["SKINNED"])),
            (
                "all",
                creature.all,
                flags(&["QUARTERED", "FIELD_DRESS", "GIBBED", "SKINNED"]),
            ),
        ];
        for (label, expected, flags) in cases {
            let got = corpse_volume(creature.monster_volume, &flags);
            if got != expected {
                differing.push(format!(
                    "{} {label}: the port says {got}, the reference says {expected} (creature volume {})",
                    creature.id, creature.monster_volume
                ));
            }
        }
    }
    assert!(
        capped > 5,
        "the cap has to be exercised, or it is untested: {capped} corpses differ from the creature's volume"
    );
    assert!(
        differing.is_empty(),
        "{} of {} measurements disagree; the first twenty are {:#?}",
        differing.len(),
        creatures.len() * 7,
        differing.get(..20).unwrap_or(&differing)
    );
}

#[test]
fn a_corpse_in_the_world_is_measured_from_its_creature_and_not_from_its_item_type() {
    // The corpse item type is a placeholder - every corpse is the same item type - so its own volume says nothing about the body it holds. The rule reaches for the
    // **creature's** volume, and the butchering flags are what this corpse carries rather than what its type states.
    let mut world = World::new();
    let corpse = drop_item(&mut world, (3, 3), ItemTypeId(7), 1);
    world.entity_mut(corpse).insert(CorpseOf {
        kind: MonsterKindId(1),
        died_on: 0,
        revives: false,
        monster_volume_ml: 100_000,
    });
    // The type says one millilitre, and the corpse says otherwise.
    let facts = |_kind: ItemTypeId| TypeVolume {
        millilitres: 1,
        integral_millilitres: 1,
        count_by_charges: false,
        stack_size: 1,
    };
    assert_eq!(
        item_volume(&mut world, corpse, facts),
        100_000,
        "a plain corpse is the creature's volume"
    );
    world
        .entity_mut(corpse)
        .insert(ItemFlags(["QUARTERED".to_owned()].into_iter().collect()));
    assert_eq!(
        item_volume(&mut world, corpse, facts),
        25_000,
        "a quartered corpse is a quarter of it"
    );
    world.entity_mut(corpse).insert(ItemFlags(
        ["QUARTERED", "FIELD_DRESS", "GIBBED", "SKINNED"]
            .iter()
            .map(|flag| (*flag).to_owned())
            .collect(),
    ));
    // 100,000 -> 25,000 -> 18,750 -> 15,937.5 -> 13,546.875, truncated at each step.
    assert_eq!(
        item_volume(&mut world, corpse, facts),
        13_546,
        "and all four flags truncate at every step"
    );
}
