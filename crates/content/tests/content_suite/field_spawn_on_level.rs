//! Spawning against a real level, where the map decides what is open.
//!
//! The spawner has taken a predicate so far; what a field processor actually has is a **map**. This joins the two: the level
//! answers what is on a square, the terrain table says whether that takes a creature, and the spawner asks both before it puts a
//! monster down.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::spawn_monsters_on_level;
use cdda_content::monster_groups::load_paths;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;

const GROUND: u32 = 1;
const WALL: u32 = 2;

fn groups() -> Vec<cdda_content::monster_groups::MonsterGroup> {
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&dirs).expect("the groups load")
}

fn walled_level() -> Level {
    // Eight submap squares of open ground with a wall drawn down column ten, so a spawn to the east of it can only happen if the
    // level was consulted.
    let mut level = Level::new();
    level.ensure(0, 0, GROUND);
    for y in 0..12 {
        level.set_terrain(10, y, WALL, GROUND);
    }
    level
}

#[test]
fn a_monster_never_lands_inside_the_wall() {
    let groups = groups();
    let level = walled_level();
    let mut ever_on_wall = 0;
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..60 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        let spawned = spawn_monsters_on_level(
            &level,
            |terrain| terrain == GROUND,
            1,
            4,
            "GROUP_ZOMBIE",
            (9, 6),
            3,
            &groups,
            &mut rolls,
        );
        for (_, square) in &spawned {
            let terrain = level
                .terrain_at(square.0, square.1)
                .expect("a square of the level");
            if terrain == WALL {
                ever_on_wall += 1;
            }
        }
    }
    assert_eq!(ever_on_wall, 0, "{ever_on_wall} monsters stood in the wall");
}

#[test]
fn a_monster_lands_on_the_open_side_of_the_wall_when_that_is_all_there_is() {
    // Column ten is wall for twelve rows; the only open squares within a radius of one of (9,6) are to the west, so every monster
    // must appear there.
    let groups = groups();
    let level = walled_level();
    let mut rolls = Rolls::new(5);
    let spawned = spawn_monsters_on_level(
        &level,
        |terrain| terrain == GROUND,
        1,
        3,
        "GROUP_ZOMBIE",
        (9, 6),
        1,
        &groups,
        &mut rolls,
    );
    assert_eq!(spawned.len(), 3);
    assert!(
        spawned.iter().all(|(_, square)| square.0 <= 9),
        "{spawned:?}"
    );
}

#[test]
fn a_field_completely_walled_in_spawns_nothing() {
    let groups = groups();
    let mut level = Level::new();
    level.ensure(0, 0, WALL);
    let mut rolls = Rolls::new(9);
    let spawned = spawn_monsters_on_level(
        &level,
        |terrain| terrain == GROUND,
        1,
        3,
        "GROUP_ZOMBIE",
        (5, 5),
        2,
        &groups,
        &mut rolls,
    );
    assert!(spawned.is_empty(), "{spawned:?}");
}
