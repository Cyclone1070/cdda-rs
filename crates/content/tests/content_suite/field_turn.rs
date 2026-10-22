//! What a field does on its turn.
//!
//! Reference: `map::process_fields_in_submap` (`map_field.cpp:422-499`). The order of the three steps is the whole rule: a field
//! **created this turn is not processed at all** - it only decays - which gives a player standing in fire a turn to step out; a
//! field that survives runs every processor its type registered, spawning among them; and the decay comes **last**, so a field
//! that spawns this turn is still at full strength while it does.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::{FieldTurn, run_field_turn};
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

fn spawner() -> FieldTurn {
    // A field that spawns one zombie every turn it is processed, with no decay pressure: intensity two, a long half-life.
    FieldTurn {
        spawn_chance: 1,
        spawn_count: 1,
        spawn_group: "GROUP_ZOMBIE".to_owned(),
        spawn_radius: 1,
        upgrade_chance: 0,
        upgrade_duration: 0,
    }
}

#[test]
fn a_field_created_this_turn_spawns_nothing() {
    let groups = groups();
    let level = {
        let mut level = Level::new();
        level.ensure(0, 0, GROUND);
        level
    };
    let mut field = FieldEntry::new(2, 1_000_000, false, false);
    let mut rolls = Rolls::new(4);
    let spawned = run_field_turn(
        &mut field,
        &spawner(),
        (5, 5),
        &level,
        |terrain| terrain == GROUND,
        &groups,
        &mut rolls,
        0,
    );
    assert!(
        spawned.is_empty(),
        "a newborn field spawns nothing: {spawned:?}"
    );
    // It still decays - that is the whole of what happens to it - so its age moves off zero. My first version asserted the age
    // stayed at zero, which would have meant the field was skipped entirely rather than only spared its processors.
    assert_eq!(field.age(), 1, "a newborn field decays and nothing else");
}

#[test]
fn a_field_that_has_been_there_a_turn_spawns_and_then_decays() {
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let mut field = FieldEntry::new(2, 1_000_000, false, false);
    field.set_age(1);
    let mut rolls = Rolls::new(4);
    let spawned = run_field_turn(
        &mut field,
        &spawner(),
        (5, 5),
        &level,
        |terrain| terrain == GROUND,
        &groups,
        &mut rolls,
        1,
    );
    assert_eq!(spawned.len(), 1, "{spawned:?}");
    // The decay happened, so the field's age moved on; the spawn did not stop it.
    assert!(field.age() >= 1, "age {}", field.age());
    assert!(field.is_alive());
}

#[test]
fn a_field_with_no_spawn_data_runs_no_spawner_at_all() {
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let mut field = FieldEntry::new(2, 1_000_000, false, false);
    field.set_age(1);
    let mut quiet = spawner();
    quiet.spawn_chance = 0;
    quiet.spawn_count = 0;
    let mut rolls = Rolls::new(4);
    let spawned = run_field_turn(
        &mut field,
        &quiet,
        (5, 5),
        &level,
        |terrain| terrain == GROUND,
        &groups,
        &mut rolls,
        1,
    );
    assert!(spawned.is_empty(), "{spawned:?}");
}

#[test]
fn a_dead_field_does_nothing_and_keeps_nothing() {
    // An intensity of zero is a field that has decayed away: the reference removes it before any processor runs.
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    let mut field = FieldEntry::new(2, 1, false, false);
    field.set_intensity(0);
    field.set_age(5);
    let mut rolls = Rolls::new(4);
    let spawned = run_field_turn(
        &mut field,
        &spawner(),
        (5, 5),
        &level,
        |terrain| terrain == GROUND,
        &groups,
        &mut rolls,
        5,
    );
    assert!(spawned.is_empty(), "{spawned:?}");
    assert!(!field.is_alive());
}
