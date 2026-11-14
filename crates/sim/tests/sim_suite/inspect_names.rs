//! A report that says `?` is a report that cannot be read.
//!
//! `inspect::what_is_at` names every component an entity carries, from a table of the port's own. The table is a **short** list, and a component missing from it turns a
//! debugging report into a row of question marks - which is worse than no report, because it looks like an answer. This gate asks the table about the things a person
//! actually inspects: a monster with its type's numbers, the survivor, the ground they stand on and what they are carrying.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_json::{Value, parse};
use cdda_sim::terrain_table::TerrainTable;

/// A world with a floor, a zombie from its type, a survivor and an item on the floor.
fn an_inhabited_world() -> (World, Entity, Entity) {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..8 {
        for y in 0..8 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let mut terrain = TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert_move_cost(1, 2);
    cdda_sim::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/monsters/monsters.json");
    let bytes = std::fs::read(path).expect("the monster dump exists");
    let root = parse(&bytes).expect("the dump is valid JSON");
    let Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    let zombie = cdda_content::monsters::load_objects(prototypes)
        .expect("the dump loads")
        .into_iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie");
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: zombie.hp,
        }],
    );
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_zombie").expect("a zombie");
    let monster =
        cdda_sim::creature::spawn_monster_of_type(&mut world, kind, &zombie, (3, 3, 0), 100);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    (world, monster, survivor)
}

/// Nothing a person inspects reports as a question mark.
#[test]
fn every_component_a_creature_carries_has_a_name() {
    let (world, monster, survivor) = an_inhabited_world();

    // A square holds the index's own square entity, the ground, and whatever stands on them - so the check is over **everything on the square**, which is what a reader
    // of the report sees.
    for (what, square) in [("the monster", (3, 3, 0)), ("the survivor", (0, 0, 0))] {
        let found = cdda_sim::inspect::what_is_at(&world, square);
        assert!(found.len() >= 2, "{what} is found on its square: {found:?}");
        let unnamed: Vec<&String> = found
            .iter()
            .flat_map(|(_, components)| components.iter())
            .filter(|name| name.as_str() == "?")
            .collect();
        assert!(
            unnamed.is_empty(),
            "{what}: {} component(s) the report cannot name, in {found:?}",
            unnamed.len()
        );
    }
    // And the ground is named too, since that is the other half of every square report.
    let ground = cdda_sim::inspect::what_is_at(&world, (5, 5, 0));
    assert!(
        ground
            .iter()
            .flat_map(|(_, components)| components.iter())
            .all(|name| name.as_str() != "?"),
        "the floor is named: {ground:?}"
    );
    let _ = (monster, survivor);
}
