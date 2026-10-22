//! The sweep that walks a submap's fields.
//!
//! Reference: `map::process_fields_in_submap` (`map_field.cpp:445-499`) loops `locx` **outside** and `locy` inside, so a submap is
//! visited column by column and every die spent inside the loop lands in that order. It is the same trap that cost this port six
//! rounds in map generation: a sweep in the other order produces a world that looks right and is not the same world.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use cdda_content::fields::{FieldTurn, run_submap_field_turns};
use cdda_content::monster_groups::load_paths;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;

const GROUND: u32 = 1;

fn groups() -> Vec<cdda_content::monster_groups::MonsterGroup> {
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&dirs).expect("the groups load")
}

fn spawning() -> FieldTurn {
    FieldTurn {
        spawn_chance: 1,
        spawn_count: 1,
        spawn_group: "GROUP_ZOMBIE".to_owned(),
        spawn_radius: 0,
        upgrade_chance: 0,
        upgrade_duration: 0,
    }
}

fn aged_field() -> FieldEntry {
    let mut field = FieldEntry::new(2, 1_000_000, false, false);
    field.set_age(1);
    field
}

#[test]
fn the_sweep_visits_a_submap_column_by_column() {
    // Four squares holding a field, one in each corner of a two-by-two block: the order they are processed in is the order the
    // dice are spent, and it has to be the reference's - x first, then y.
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let mut fields: BTreeMap<(i32, i32), FieldEntry> = BTreeMap::new();
    for square in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        fields.insert(square, aged_field());
    }
    let mut visited = Vec::new();
    let mut rolls = Rolls::new(21);
    let spawned = run_submap_field_turns(
        &mut fields,
        &spawning(),
        &level,
        |terrain| terrain == GROUND,
        &groups,
        &mut rolls,
        1,
        |square| visited.push(square),
    );
    assert_eq!(
        visited,
        vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        "column by column"
    );
    assert_eq!(spawned.len(), 4, "{spawned:?}");
    assert_eq!(
        spawned
            .iter()
            .map(|(_, square)| *square)
            .collect::<Vec<_>>(),
        visited
    );
}

#[test]
fn a_square_without_a_field_is_not_visited_and_spends_nothing() {
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let mut fields: BTreeMap<(i32, i32), FieldEntry> = BTreeMap::new();
    fields.insert((3, 4), aged_field());
    let mut visited = Vec::new();
    let mut rolls = Rolls::new(2);
    let spawned = run_submap_field_turns(
        &mut fields,
        &spawning(),
        &level,
        |terrain| terrain == GROUND,
        &groups,
        &mut rolls,
        1,
        |square| visited.push(square),
    );
    assert_eq!(visited, vec![(3, 4)]);
    assert_eq!(spawned.len(), 1);
}

#[test]
fn the_same_seed_sweeps_to_the_same_result() {
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let build = || {
        let mut fields: BTreeMap<(i32, i32), FieldEntry> = BTreeMap::new();
        for square in [(0, 0), (1, 2), (5, 5)] {
            fields.insert(square, aged_field());
        }
        fields
    };
    let mut first_fields = build();
    let mut second_fields = build();
    let mut first = Rolls::new(77);
    let mut second = Rolls::new(77);
    let a = run_submap_field_turns(
        &mut first_fields,
        &spawning(),
        &level,
        |t| t == GROUND,
        &groups,
        &mut first,
        1,
        |_| {},
    );
    let b = run_submap_field_turns(
        &mut second_fields,
        &spawning(),
        &level,
        |t| t == GROUND,
        &groups,
        &mut second,
        1,
        |_| {},
    );
    assert_eq!(a, b);
}
