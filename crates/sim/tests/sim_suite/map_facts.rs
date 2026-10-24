//! A fire reading a real map instead of a test double.
//!
//! Everything the fire chain needs to know about the world - does this ground burn, does it burn hard, is the floor above open, does this room suppress
//! smoke - is in the terrain the reference loaded. `MapFacts` is that terrain arranged so the fire chain can ask about a square, and these tests check the
//! answers against the reference's own flag rules.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::mapdata::{MapData, MapDataKind};
use cdda_sim::{FireMap, MapFacts, TerrainFacts};
use std::collections::BTreeSet;

/// The facts a fire needs, read out of a loaded prototype the way the game's own adapter does.
fn facts_of(prototype: &MapData) -> TerrainFacts {
    TerrainFacts {
        flammable: prototype.flags.contains("FLAMMABLE"),
        flammable_ash: prototype.flags.contains("FLAMMABLE_ASH"),
        flammable_hard: prototype.flags.contains("FLAMMABLE_HARD"),
        fire_container: prototype.flags.contains("FIRE_CONTAINER"),
        no_floor: prototype.flags.contains("NO_FLOOR"),
        suppress_smoke: prototype.flags.contains("SUPPRESS_SMOKE"),
    }
}

fn terrain(id: &str, flags: &[&str]) -> MapData {
    MapData {
        kind: MapDataKind::Terrain,
        id: id.to_string(),
        name: id.to_string(),
        symbol: [35; 4],
        color: [cdda_core_types::colors::NcColor::default(); 4],
        move_cost: 2,
        light: 0,
        coverage: 0,
        flags: flags
            .iter()
            .map(|flag| (*flag).to_string())
            .collect::<BTreeSet<String>>(),
        // A fixture is not loaded from JSON, so it starts where an ordinary prototype starts: opaque.
        transparent: false,
        bash: None,
        roof: None,
    }
}

#[test]
fn ordinary_ground_does_not_burn() {
    let mut facts = MapFacts::new();
    facts.set_terrain((0, 0, 0), facts_of(&terrain("t_floor", &[])));
    assert!(!facts.can_burn((0, 0, 0)));
}

#[test]
fn hard_ground_burns_and_says_so_to_a_fire_that_wants_to_spread() {
    // `FLAMMABLE_HARD` is the flag that makes `t_floor` feed a fire on a die rather than every turn, and the fire's neighbour question has to see it.
    let mut facts = MapFacts::new();
    facts.set_terrain(
        (0, 0, 0),
        facts_of(&terrain("t_floor", &["FLAMMABLE_HARD"])),
    );
    assert!(facts.can_burn((0, 0, 0)));
    assert!(facts.neighbour((0, 0, 0)).flammable_hard);
    assert!(!facts.neighbour((0, 0, 0)).flammable);
}

#[test]
fn a_fire_container_burns_but_does_not_feed_the_fire_in_it() {
    // `can_burn` in the reference is `( ter.is_flammable() || frn.is_flammable() ) && !has_flag( FIRE_CONTAINER )`: a brazier is flammable and still does
    // not feed what is burning inside it.
    let mut facts = MapFacts::new();
    facts.set_terrain(
        (0, 0, 0),
        facts_of(&terrain("f_brazier", &["FLAMMABLE", "FIRE_CONTAINER"])),
    );
    assert!(
        !facts.can_burn((0, 0, 0)),
        "a container holds the fire, it does not feed it"
    );
}

#[test]
fn an_open_square_overhead_lets_the_smoke_through() {
    // `smoke_ceiling_open` is `has_flag_ter( TFLAG_NO_FLOOR, up )`.
    let mut facts = MapFacts::new();
    facts.set_terrain(
        (0, 0, 0),
        facts_of(&terrain("t_floor", &["FLAMMABLE_HARD"])),
    );
    assert!(
        !facts.smoke_ceiling_open((0, 0, 0)),
        "a floor overhead stops it"
    );
    facts.set_terrain((0, 0, 1), facts_of(&terrain("t_open_air", &["NO_FLOOR"])));
    assert!(
        facts.smoke_ceiling_open((0, 0, 0)),
        "open air above does not"
    );
}

#[test]
fn a_sealed_room_suppresses_smoke() {
    let mut facts = MapFacts::new();
    facts.set_terrain(
        (0, 0, 0),
        facts_of(&terrain("t_floor", &["SUPPRESS_SMOKE"])),
    );
    assert!(facts.suppress_smoke((0, 0, 0)));
}

#[test]
fn nearby_fires_are_counted_on_the_fires_own_floor() {
    // `adjacent_fires` counts the eight squares around, and a fire one floor up is not one of them.
    let mut facts = MapFacts::new();
    facts.note_fire((1, 0, 0), 1, 0);
    facts.note_fire((0, 1, 0), 1, 0);
    facts.note_fire((0, 0, 1), 1, 0);
    assert_eq!(
        facts.adjacent_fires((0, 0, 0)),
        2,
        "two beside it, one upstairs"
    );
}

#[test]
fn the_ground_class_comes_from_the_flags_underfoot() {
    use cdda_core_types::fire::Ground;
    let mut facts = MapFacts::new();
    facts.set_terrain((0, 0, 0), facts_of(&terrain("t_dirt", &["FLAMMABLE"])));
    facts.set_terrain(
        (1, 0, 0),
        facts_of(&terrain("t_grass", &["FLAMMABLE", "FLAMMABLE_ASH"])),
    );
    facts.set_terrain(
        (2, 0, 0),
        facts_of(&terrain("t_floor", &["FLAMMABLE_HARD"])),
    );
    facts.set_terrain((3, 0, 0), facts_of(&terrain("t_concrete", &[])));
    facts.set_terrain(
        (4, 0, 0),
        facts_of(&terrain("f_brazier", &["FLAMMABLE", "FIRE_CONTAINER"])),
    );
    assert_eq!(facts.ground((0, 0, 0)), Ground::Flammable);
    assert_eq!(
        facts.ground((1, 0, 0)),
        Ground::Flammable,
        "ground that is both is asked about FLAMMABLE first, which is the branch without a die"
    );
    assert_eq!(
        facts.ground((2, 0, 0)),
        Ground::FlammableHard,
        "hard ground feeds on a die"
    );
    assert_eq!(facts.ground((3, 0, 0)), Ground::Inert);
    assert_eq!(
        facts.ground((4, 0, 0)),
        Ground::Inert,
        "a container feeds nothing"
    );
}

#[test]
fn a_square_takes_its_flags_from_its_terrain_and_its_furniture_together() {
    // Reference: `ter_furn_has_flag( ter, frn, flag )` reads both, and `can_burn` is `( ter.is_flammable() || frn.is_flammable() ) && !FIRE_CONTAINER`. So a
    // tiled floor with a bench on it burns like a bench, and a tiled floor with a locker on it does not burn at all - the terrain's `FLAMMABLE_HARD` is not
    // enough once the furniture is taken into the same answer.
    let bare = cdda_sim::TerrainFacts::default();
    let floor = cdda_sim::TerrainFacts {
        flammable_hard: true,
        ..bare
    };
    let bench = cdda_sim::TerrainFacts {
        flammable_ash: true,
        ..bare
    };
    let locker = bare;
    let facts = |under: cdda_sim::TerrainFacts| {
        let mut map = cdda_sim::MapFacts::new();
        map.set_terrain((0, 0, 0), floor);
        map.set_furniture((0, 0, 0), under);
        map
    };
    assert_eq!(
        facts(bench).ground((0, 0, 0)),
        cdda_core_types::fire::Ground::FlammableHard,
        "the harder flag is asked first, so a bench on a tiled floor is still hard ground"
    );
    assert!(facts(bench).can_burn((0, 0, 0)), "the bench makes it burn");
    let mut on_a_bench = cdda_sim::MapFacts::new();
    on_a_bench.set_terrain((0, 0, 0), bare);
    on_a_bench.set_furniture((0, 0, 0), bench);
    assert_eq!(
        on_a_bench.ground((0, 0, 0)),
        cdda_core_types::fire::Ground::FlammableAsh,
        "and with nothing flammable under it, the bench is what burns"
    );
    let mut on_a_locker = cdda_sim::MapFacts::new();
    on_a_locker.set_terrain((0, 0, 0), bare);
    on_a_locker.set_furniture((0, 0, 0), locker);
    assert!(
        !on_a_locker.can_burn((0, 0, 0)),
        "a locker on a fireproof floor is a square that does not burn, so the fire draws no fuel die"
    );
    // A hole overhead is read from the square **above**, not from this one, so the test asks the square where it belongs.
    assert!(
        !facts(bare).smoke_ceiling_open((0, 0, 0)),
        "nothing overhead is a ceiling"
    );
}
