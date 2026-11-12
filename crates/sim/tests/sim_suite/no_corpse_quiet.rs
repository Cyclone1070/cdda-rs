//! A body that dies quietly and leaves nothing behind - and still leaves its loot.
//!
//! Reference: `monster::no_corpse_quiet` (`monster.h:570`) is tested first in `mdeath::normal`, which answers an empty item location (`mondeath.cpp:44-46`). It is **not** the same
//! promise as `no_extra_death_drops`: that one keeps the corpse and drops the loot, this one drops the corpse and keeps the loot.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

const CORPSE: cdda_sim::ItemTypeId = cdda_sim::ItemTypeId(99);

fn a_world() -> World {
    let mut world = World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_haunt",
            hp: 40,
        }],
    );
    cdda_sim::monster_registry::set_death_drops(&mut world, cdda_sim::MonsterKindId(1), "loot_group");
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "corpse",
                volume_ml: 4_000,
                weight_g: 4_000,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
            cdda_sim::item_registry::Kind {
                id: "rag",
                volume_ml: 250,
                weight_g: 100,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
        ],
    );
    let mut definitions = std::collections::BTreeMap::new();
    definitions.insert(
        "loot_group".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "loot_group".to_owned(),
            distribution: false,
            entries: vec![cdda_content::item_group_draw::Entry::Item {
                id: "rag".to_owned(),
                probability: 100,
                count: (1, 1),
                event: None,
            }],
        },
    );
    cdda_sim::item_groups::set_groups(&mut world, definitions);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    world
}

/// The kinds of thing lying on a square once a body there has died.
fn leftovers(world: &mut World, at: (i32, i32)) -> Vec<cdda_sim::ItemTypeId> {
    cdda_sim::creature::reap_dead_creatures_as_corpses(world, CORPSE, |_| 62_500);
    cdda_sim::items_at(world, at)
        .iter()
        .filter_map(|item| world.get::<cdda_sim::GroundItem>(*item).map(|held| held.kind))
        .collect()
}

/// **No corpse, no message, and the loot still on the floor.**
#[test]
fn a_quiet_body_leaves_no_corpse_and_still_leaves_its_loot() {
    let mut world = a_world();
    let body = cdda_sim::spawn_monster(&mut world, (3, 3), cdda_sim::MonsterKindId(1), 40);
    world.entity_mut(body).insert(cdda_sim::creature::NoCorpseQuiet);
    cdda_sim::creature::set_health_left(&mut world, body, 0);
    cdda_sim::messages::clear(&mut world);
    let left = leftovers(&mut world, (3, 3));
    // The reaper is told which kind of thing a corpse is, and this fixture hands it one no registry holds: what is left on the square is what the rule decides to drop.
    let rag = cdda_sim::item_registry::id_of(&mut world, "rag").expect("rag is an item");
    assert!(
        !left.contains(&CORPSE),
        "**a body told to leave no corpse leaves none**: {left:?}"
    );
    assert!(
        left.contains(&rag),
        "**and still drops what its death group says**, which is what separates this flag from `NoExtraDeathDrops`: {left:?}"
    );
    assert!(
        !cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("dies")),
        "and it dies without a word: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **An ordinary body of the same kind does leave one**, so the gate above is about the flag and not about the fixture.
#[test]
fn a_body_without_the_flag_leaves_a_corpse() {
    let mut world = a_world();
    let body = cdda_sim::spawn_monster(&mut world, (3, 3), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::creature::set_health_left(&mut world, body, 0);
    let left = leftovers(&mut world, (3, 3));
    assert!(left.contains(&CORPSE), "the same kind leaves a corpse: {left:?}");
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("dies")),
        "and says so: {:?}",
        cdda_sim::messages::lines(&world)
    );
}
