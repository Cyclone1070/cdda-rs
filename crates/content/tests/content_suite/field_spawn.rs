//! What a monster-spawning field actually produces.
//!
//! Reference: `field_processor_monster_spawn` (`map_field.cpp:762-786`). A level with a positive chance and count rolls
//! **once**; if that roll passes, it asks the group for a batch repeatedly until the count is used up, and each monster goes to
//! a random passable square within the radius. The gate below pins the half that was missing: a triggered spawn produces
//! **monsters from the group**, not the group's name.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::spawn_from_field;
use cdda_content::monster_groups::load_paths;
use cdda_core_types::rng::Rolls;

fn groups() -> Vec<cdda_content::monster_groups::MonsterGroup> {
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&dirs).expect("the groups load")
}

#[test]
fn a_triggered_spawn_produces_as_many_monsters_as_the_count_says() {
    // A chance of one in one always passes, which is what makes the count observable.
    let groups = groups();
    let mut rolls = Rolls::new(11);
    let produced = spawn_from_field(1, 3, "GROUP_ZOMBIE", &groups, &mut rolls);
    assert_eq!(produced.len(), 3, "{produced:?}");
    assert!(
        produced.iter().all(|monster| groups.iter().any(|group| group.entries.iter().any(
            |entry| matches!(&entry.kind, cdda_content::monster_groups::EntryKind::Monster(id) if id == monster)
        ))),
        "every result is a monster the data defines: {produced:?}"
    );
}

#[test]
fn a_chance_of_a_hundred_triggers_about_once_in_a_hundred() {
    let groups = groups();
    let mut triggered = 0;
    for attempt in 0..2000_u32 {
        // The seeds are **spread**, not counted up: `uniform_int` maps a small engine value to a small answer, and `minstd_rand0`'s first draw from a small seed is
        // small too, so `0, 1, 2, ...` would test the low end of one distribution rather than a one-in-a-hundred chance. (Measured: counting up gives 1278 hits of
        // 2000; this spread gives the handful the chance promises.)
        let mut rolls = Rolls::new(attempt.wrapping_mul(2_654_435_761).wrapping_add(1));
        if !spawn_from_field(100, 1, "GROUP_ZOMBIE", &groups, &mut rolls).is_empty() {
            triggered += 1;
        }
    }
    assert!(
        (10..=35).contains(&triggered),
        "{triggered} of 2000 at a one-in-a-hundred chance"
    );
}

#[test]
fn a_level_that_spawns_nothing_spends_no_dice() {
    // The reference tests the count and the chance **before** rolling, so a level with either at zero is free. A port that
    // rolled first would shift every later draw in the world.
    let groups = groups();
    let mut untouched = Rolls::new(5);
    let expected = untouched.clone();
    assert!(spawn_from_field(0, 3, "GROUP_ZOMBIE", &groups, &mut untouched).is_empty());
    assert!(spawn_from_field(10, 0, "GROUP_ZOMBIE", &groups, &mut untouched).is_empty());
    assert!(spawn_from_field(10, 3, "", &groups, &mut untouched).is_empty());
    assert_eq!(untouched, expected, "no dice were spent");
}

#[test]
fn a_group_the_data_does_not_define_spawns_nothing() {
    let groups = groups();
    let mut rolls = Rolls::new(3);
    assert!(spawn_from_field(1, 2, "GROUP_NOT_IN_THE_DATA", &groups, &mut rolls).is_empty());
}
