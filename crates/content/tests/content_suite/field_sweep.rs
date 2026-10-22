//! The sweep that walks a submap's fields.
//!
//! Reference: `map::process_fields_in_submap` (`map_field.cpp:445-499`) loops `locx` **outside** and `locy` inside, so a submap is
//! visited column by column and every die spent inside the loop lands in that order. It is the same trap that cost this port six
//! rounds in map generation: a sweep in the other order produces a world that looks right and is not the same world.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::{FieldLevel, FieldType, run_level_field_turns};
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

fn field_level(spawn_chance: i32, spawn_count: i32) -> FieldLevel {
    FieldLevel {
        name: String::new(),
        sym: 0,
        color: String::new(),
        dangerous: false,
        transparent: true,
        move_cost: 0,
        upgrade_chance: 0,
        upgrade_duration: 0,
        spawn_chance,
        spawn_count,
        spawn_radius: 0,
        spawn_group: if spawn_chance > 0 {
            "GROUP_ZOMBIE".to_owned()
        } else {
            String::new()
        },
    }
}

/// The field type the sweep runs: its first level spawns, the second does not - so an intensity-two field is visibly different.
fn spawning() -> FieldType {
    FieldType {
        id: "fd_probe".to_owned(),
        priority: 0,
        underwater_age_speedup: 0,
        outdoor_age_speedup: 0,
        percent_spread: 0,
        levels: vec![field_level(1, 1), field_level(0, 0)],
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
    for square in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        level.add_field(square.0, square.1, aged_field());
    }
    let mut visited = Vec::new();
    let mut rolls = Rolls::new(21);
    let spawned = run_level_field_turns(
        &mut level,
        &spawning(),
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
    level.add_field(3, 4, aged_field());
    let mut visited = Vec::new();
    let mut rolls = Rolls::new(2);
    let spawned = run_level_field_turns(
        &mut level,
        &spawning(),
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
        let mut built = Level::new();
        built.ensure(0, 0, GROUND);
        for square in [(0, 0), (1, 2), (5, 5)] {
            built.add_field(square.0, square.1, aged_field());
        }
        built
    };
    let mut first_level = build();
    let mut second_level = build();
    let mut first = Rolls::new(77);
    let mut second = Rolls::new(77);
    let a = run_level_field_turns(
        &mut first_level,
        &spawning(),
        |t| t == GROUND,
        &groups,
        &mut first,
        1,
        |_| {},
    );
    let b = run_level_field_turns(
        &mut second_level,
        &spawning(),
        |t| t == GROUND,
        &groups,
        &mut second,
        1,
        |_| {},
    );
    assert_eq!(a, b);
}

#[test]
fn a_field_that_died_during_its_turn_leaves_the_level() {
    // The removal falls out of the same pass: the sweep takes a field off the level, gives it its turn, and puts it back only if
    // it survived. A field that decays away must not be restored.
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let mut dying = FieldEntry::new(1, 1, false, false);
    dying.set_age(1_000_000);
    level.add_field(2, 2, dying);
    let mut rolls = Rolls::new(4);
    let mut quiet = spawning();
    quiet.levels = vec![field_level(0, 0)];
    run_level_field_turns(
        &mut level,
        &quiet,
        |t| t == GROUND,
        &groups,
        &mut rolls,
        1_000_000,
        |_| {},
    );
    assert!(
        level.field_at(2, 2).is_none(),
        "a decayed field is gone from the level"
    );
    assert_eq!(level.field_squares(), Vec::new());
}

#[test]
fn the_sweep_uses_the_fields_live_intensity() {
    // The fixture's first level spawns and its second does not, so the same field on the same square spawns or not depending only
    // on how strong it currently is. A sweep that used one level's numbers for everything would fail one of these two.
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let mut weak = aged_field();
    weak.set_intensity(1);
    level.add_field(4, 4, weak);
    let mut rolls = Rolls::new(9);
    let spawned = run_level_field_turns(
        &mut level,
        &spawning(),
        |t| t == GROUND,
        &groups,
        &mut rolls,
        1,
        |_| {},
    );
    assert_eq!(
        spawned.len(),
        1,
        "an intensity-one field spawns: {spawned:?}"
    );

    let mut strong_level = Level::new();
    strong_level.ensure(0, 0, GROUND);
    let mut strong = aged_field();
    strong.set_intensity(2);
    strong_level.add_field(4, 4, strong);
    let mut rolls = Rolls::new(9);
    let spawned = run_level_field_turns(
        &mut strong_level,
        &spawning(),
        |t| t == GROUND,
        &groups,
        &mut rolls,
        1,
        |_| {},
    );
    assert!(
        spawned.is_empty(),
        "an intensity-two field does not: {spawned:?}"
    );
}
