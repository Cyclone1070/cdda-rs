//! **Generating the map for an overmap tile.**
//!
//! Reference: `mapgen_factory`'s `generate` (`mapgen.cpp:1070-1090`): several mapgens may be filed under one overmap terrain, each with a weight, and the game picks one
//! **by weight** - `weights_.pick()` - then draws it. The port could draw a mapgen and write it, but nothing joined the two: `place_drawn` had no caller, so an overmap tile
//! never became a map. This is that join, and the weight is the rule that makes it a choice rather than an arbitrary first hit.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use bevy_ecs::prelude::*;
use cdda_content::mapgen::MapgenObject;
use std::collections::BTreeMap;

/// A mapgen that draws one symbol, `#`, as the terrain it is given.
fn a_mapgen(id: &str, terrain: &str, weight: i32) -> MapgenObject {
    MapgenObject {
        om_terrain: id.to_owned(),
        rows: vec!["##".to_owned(), "##".to_owned()],
        terrain: [('#', terrain.to_owned())].into_iter().collect(),
        furniture: BTreeMap::new(),
        weight,
        fields: Vec::new(),
        fill_ter: None,
    }
}

/// A world with a little level and the codes the drawn terrains use.
fn a_world(mapgens: Vec<MapgenObject>) -> World {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..48 {
        for y in 0..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_house_two".to_owned(), 12u32),
        ]
        .into_iter()
        .collect(),
    );
    cdda_sim::mapgen_update::set_mapgens(
        &mut world,
        mapgens.into_iter().map(|one| (one.om_terrain.clone(), one)),
    );
    world
}

/// **Several mapgens under one terrain are chosen by weight.** Two houses, one weighing three times the other: over four hundred tiles the heavy one is picked about
/// three times in four. A rule that always takes the first would answer four hundred to nothing, and one that takes the last would answer nothing to four hundred.
#[test]
fn a_tile_with_several_mapgens_is_drawn_by_weight() {
    let mut world = a_world(vec![
        a_mapgen("house", "t_house_one", 1),
        a_mapgen("house", "t_house_two", 3),
    ]);
    let mut light = 0;
    let mut heavy = 0;
    // **One generator, four hundred draws.** A fresh `Rolls` per iteration would be a mistake worth naming: the engine's *first* draw is `16807 * seed` over a range of two
    // billion, so for small seeds every first draw lands in the same quarter of the range and a per-seed loop would count a property of the seeding, not of the weights.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    for _ in 0..400 {
        let written = cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
        assert_eq!(written, Some(4), "**the chunk was written**: {written:?}");
        match cdda_sim::square::terrain_at(&world, (0, 0, 0)) {
            Some(11) => light += 1,
            Some(12) => heavy += 1,
            other => panic!("a square the chunk wrote holds {other:?}"),
        }
    }
    assert!(
        (250..=350).contains(&heavy),
        "**three in four pick the heavy mapgen**: {heavy} heavy, {light} light"
    );
}

/// **A terrain with one mapgen always draws it**, and a terrain with none draws nothing at all.
#[test]
fn a_tile_with_one_mapgen_draws_it_and_a_tile_with_none_draws_nothing() {
    let mut world = a_world(vec![a_mapgen("house", "t_house_one", 1)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0)),
        Some(4),
        "**the only mapgen is the one drawn**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(11),
        "and it wrote its whole chunk"
    );
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "nowhere", (0, 0, 0)),
        None,
        "**a terrain with no mapgen draws nothing**, which is the answer a fallback needs"
    );
    // **A mapgen filed at weight zero is not a candidate.** Reference: `if( weight >= 1 ) weights_.add_or_replace( ptr, weight )` (`mapgen.cpp:1070-1074`) - the reference
    // never files one, so a terrain whose only mapgen weighs nothing has nothing to draw rather than a certain draw of the impossible one.
    let mut empty = a_world(vec![a_mapgen("house", "t_house_one", 0)]);
    cdda_sim::singletons::put_rolls(&mut empty, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut empty, "house", (0, 0, 0)),
        None,
        "**a zero weight is not a choice**"
    );
}

/// **A mapgen's `fill_ter` fills the chunk, and the rows are drawn on top of it.** Reference: `mapgen_function_json::generate` (`mapgen.cpp:5100-5110`) fills the whole chunk with
/// `fill_ter` before the rows are read, and `:5168-5172` is where the member is loaded - which is why a six-square map in a twenty-four-square chunk leaves the rest at that
/// terrain rather than blank, and why a mapgen that names no `fill_ter` must explain every character it writes.
#[test]
fn a_mapgens_fill_terrain_covers_the_whole_chunk() {
    let mut world = a_world(Vec::new());
    let mut filling = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["#".to_owned()],
        terrain: [('#', "t_house_two".to_owned())].into_iter().collect(),
        furniture: BTreeMap::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: Some("t_house_one".to_owned()),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), filling.clone())]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    let written = cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        written,
        Some(24 * 24),
        "**the fill and the row together are the whole chunk**: {written:?}"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(12),
        "the row is drawn on top of the fill"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (23, 23, 0)),
        Some(11),
        "and the far corner is the fill"
    );

    // **A mapgen with no `fill_ter` writes only what its rows say**, which is what an update mapgen relies on: it must not pave the rest of the tile.
    let mut bare = a_world(Vec::new());
    filling.fill_ter = None;
    cdda_sim::mapgen_update::set_mapgens(&mut bare, [("house".to_owned(), filling)]);
    cdda_sim::singletons::put_rolls(&mut bare, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut bare, "house", (0, 0, 0)),
        Some(1),
        "**one square, because one square was drawn**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&bare, (10, 10, 0)),
        Some(0),
        "and the rest of the tile is untouched"
    );
}
