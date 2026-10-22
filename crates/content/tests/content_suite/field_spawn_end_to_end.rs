//! A monster-spawning field, end to end: gate, group, square.
//!
//! The three pieces exist separately - the gate that decides whether anything spawns, the group that decides what, and the
//! placement that decides where. What a caller needs is the composition, and the thing worth pinning is that the three spend
//! their dice **in that order**, because a spawner that picked the monster before rolling the gate would still "work" while
//! putting every later draw of the world on different dice.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::spawn_monsters_from_field;
use cdda_content::monster_groups::load_paths;
use cdda_core_types::rng::Rolls;

fn groups() -> Vec<cdda_content::monster_groups::MonsterGroup> {
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&dirs).expect("the groups load")
}

#[test]
fn a_triggered_field_places_every_monster_inside_the_box() {
    let groups = groups();
    let mut rolls = Rolls::new(4242);
    let spawned = spawn_monsters_from_field(
        1,
        3,
        "GROUP_ZOMBIE",
        (10, 10),
        2,
        &groups,
        |_, _| true,
        &mut rolls,
    );
    assert_eq!(spawned.len(), 3, "{spawned:?}");
    for (monster, square) in &spawned {
        assert!(!monster.is_empty());
        assert!((8..=12).contains(&square.0), "{square:?}");
        assert!((8..=12).contains(&square.1), "{square:?}");
    }
}

#[test]
fn a_wall_beside_the_field_never_takes_a_monster() {
    // Every square but one is refused, which forces the enumeration path in the placement. Nothing may land on the wall, and the
    // one open square is where everything lands.
    let groups = groups();
    let mut rolls = Rolls::new(7);
    let open = (10, 10);
    let spawned = spawn_monsters_from_field(
        1,
        4,
        "GROUP_ZOMBIE",
        (10, 10),
        1,
        &groups,
        move |x, y| (x, y) == open,
        &mut rolls,
    );
    assert_eq!(spawned.len(), 4);
    assert!(
        spawned.iter().all(|(_, square)| *square == open),
        "{spawned:?}"
    );
}

#[test]
fn a_field_out_of_nowhere_spawns_nothing_and_costs_nothing() {
    // A radius with no open square at all: the gate passes, the group produces monsters, and every one of them finds nowhere to
    // stand - so the result is empty rather than a panic or a monster on a wall.
    let groups = groups();
    let mut rolls = Rolls::new(11);
    let spawned = spawn_monsters_from_field(
        1,
        2,
        "GROUP_ZOMBIE",
        (0, 0),
        1,
        &groups,
        |_, _| false,
        &mut rolls,
    );
    assert!(spawned.is_empty(), "{spawned:?}");
}

#[test]
fn the_same_seed_spawns_the_same_monsters_on_the_same_squares() {
    let groups = groups();
    let mut first = Rolls::new(2024);
    let mut second = Rolls::new(2024);
    assert_eq!(
        spawn_monsters_from_field(
            1,
            3,
            "GROUP_ZOMBIE",
            (5, 5),
            2,
            &groups,
            |_, _| true,
            &mut first
        ),
        spawn_monsters_from_field(
            1,
            3,
            "GROUP_ZOMBIE",
            (5, 5),
            2,
            &groups,
            |_, _| true,
            &mut second
        )
    );
}
