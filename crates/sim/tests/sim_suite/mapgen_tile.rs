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
use cdda_content::mapgen::Piece;
use std::collections::BTreeMap;

/// A mapgen that draws one symbol, `#`, as the terrain it is given.
fn a_mapgen(id: &str, terrain: &str, weight: i32) -> MapgenObject {
    MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: id.to_owned(),
        rows: vec!["##".to_owned(), "##".to_owned()],
        terrain: [('#', Piece::Id(terrain.to_owned()))].into_iter().collect(),
        furniture: BTreeMap::new(),
        parameters: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
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
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["#".to_owned()],
        terrain: [('#', Piece::Id("t_house_two".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        fill_ter: Some("t_house_one".to_owned()),
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
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

/// **A mapgen's furniture symbol map places furniture on the squares its rows draw it on.** Reference: `mapgen_function_json::generate` (`mapgen.cpp:5480-5490`): a symbol may
/// carry terrain, furniture or both, and furniture is placed at the same square the row put it on - `furn_set( p, furniture )` - which is how a house is drawn as a grid of
/// characters rather than a list of objects.
#[test]
fn a_mapgens_furniture_lands_where_its_rows_say() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_furniture_codes(
        &mut world,
        [("f_desk".to_owned(), 21u32)].into_iter().collect(),
    );
    let furnished = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec![".F.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('F', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: [('F', Piece::Id("f_desk".to_owned()))]
            .into_iter()
            .collect(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), furnished)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0)),
        Some(3),
        "three squares drawn"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (1, 0, 0)),
        Some(21),
        "**the desk stands where the F is**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 0, 0)),
        Some(11),
        "**and on the terrain the row drew under it**, because a symbol may carry both"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        None,
        "and the plain squares have none"
    );
}

/// **A mapgen's trap symbol map sets traps on the squares its rows draw them on.** Reference: `mapgen_function_json::generate` (`mapgen.cpp:5470-5480`): the layer order is
/// terrain, then furniture, then traps, and a trap character is a square with a trap on it - which is how a minefield is drawn rather than placed piece by piece.
#[test]
fn a_mapgens_traps_are_set_where_its_rows_say() {
    let mut world = a_world(Vec::new());
    let trapped = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "field".to_owned(),
        rows: vec!["T.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('T', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: [('T', "tr_bear".to_owned())].into_iter().collect(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("field".to_owned(), trapped)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "field", (0, 0, 0)),
        Some(2),
        "two squares drawn"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_bear".to_owned()),
        "**the trap is on the square the row drew**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 0, 0)),
        None,
        "and the plain square has none"
    );
}

/// **A mapgen's item symbols place items on the squares its rows say.** Reference: `jmapgen_item_group` (`mapgen.cpp:5320-5340`): a symbol's item is placed with a `chance`
/// out of a hundred and an `amount` the dice answer between, and a chance of one hundred is always there - which is how a room is furnished with loot rather than described.
#[test]
fn a_mapgens_items_land_on_the_square_its_rows_say() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let stocked = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["I.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('I', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: [(
            'I',
            vec![cdda_content::mapgen::ItemPlacement {
                item: Some("rock".to_owned()),
                group: None,
                chance: 100,
                amount: (3, 3),
            }],
        )]
        .into_iter()
        .collect(),
        monsters: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), stocked)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0)),
        Some(2),
        "two squares drawn"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        3,
        "**three rocks, because the amount says three**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        0,
        "and the plain square has nothing on it"
    );
}

/// **A mapgen symbol may name an item group instead of an item**, and the group is expanded where the symbol sits. Reference: `jmapgen_item_group::apply`
/// (`mapgen.cpp:5360-5400`): a group is drawn from once per placement, so the square gets a random member of the group rather than a named thing - and a spawn rate
/// scales how often the group produces anything at all.
#[test]
fn a_mapgens_item_group_is_expanded_where_its_rows_say() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    cdda_sim::item_groups::set_groups(
        &mut world,
        [(
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
        )]
        .into_iter()
        .collect(),
    );
    let grouped = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["G.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('G', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: [(
            'G',
            vec![cdda_content::mapgen::ItemPlacement {
                item: None,
                group: Some("loot_group".to_owned()),
                chance: 100,
                amount: (2, 2),
            }],
        )]
        .into_iter()
        .collect(),
        monsters: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), grouped)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0)),
        Some(2),
        "two squares drawn"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        2,
        "**the group is drawn once per amount, and both rags are here**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        0,
        "and the plain square has nothing"
    );
}

/// **A mapgen's `place_monster` entries spawn monsters.** Reference: `jmapgen_monster::apply` (`mapgen.cpp:5560-5610`): an entry names a monster (or a group), a square or a
/// range of them, a `chance` out of a hundred, and a `pack_size` the dice answer between - and the pack is what makes a map a place something lives rather than a place
/// something stood.
///
/// The numbers are what the test pins: a pack of two is two monsters, both at the square the entry names, both with the hit points their kind says.
#[test]
fn a_mapgens_place_monster_spawns_its_pack() {
    let mut world = a_world(Vec::new());
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 40,
        }],
    );
    let haunted = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: vec![cdda_content::mapgen::MonsterPlacement {
            monster: Some("mon_zombie".to_owned()),
            group: None,
            chance: 100,
            pack_size: (2, 2),
            x: (1, 1),
            y: (0, 0),
            friendly: false,
        }],
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), haunted)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0)),
        Some(2),
        "two squares drawn"
    );
    let zombies: Vec<Entity> = world
        .query::<(Entity, &cdda_sim::creature::Creature)>()
        .iter(&world)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(zombies.len(), 2, "**a pack of two is two monsters**");
    for zombie in zombies {
        assert_eq!(
            cdda_sim::creature::health_of(&world, zombie),
            Some(40),
            "**with the hit points its kind says**"
        );
        assert!(
            cdda_sim::index(&world).is_some_and(|index| index.at_level(1, 0, 0).contains(&zombie)),
            "**and standing on the square the entry names**"
        );
    }
}

/// **A mapgen may name a predecessor, and that map is drawn first.** Reference: `mapgen_function_json::generate` (`mapgen.cpp:6033-6062`): `predecessor_mapgen` runs the
/// other terrain's mapgen into the chunk, and only then does this one's own rows draw on top of it - which is how a roof keeps the walls of the house under it, and how an
/// upper floor keeps the footprint below.
#[test]
fn a_mapgens_predecessor_is_drawn_first() {
    let mut world = a_world(Vec::new());
    let walls = MapgenObject {
        om_terrain: "house_inside".to_owned(),
        // Two rows of walls, and the roof covers only the first: the second row is the predecessor showing through.
        rows: vec!["##".to_owned(), "##".to_owned()],
        terrain: [('#', Piece::Id("t_house_two".to_owned()))]
            .into_iter()
            .collect(),
        ..a_plain_mapgen()
    };
    let roof = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        predecessor_mapgen: Some("house_inside".to_owned()),
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(
        &mut world,
        [
            ("house_inside".to_owned(), walls),
            ("house".to_owned(), roof),
        ],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0)),
        Some(2),
        "the mapgen's own squares are the count"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(11),
        "**the roof's own row is on top**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 0, 0)),
        Some(11),
        "and its second square"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 1, 0)),
        Some(12),
        "**and the predecessor's wall stands where the roof drew nothing**"
    );
}

/// **A `place_monster` entry that names a group draws one member from it.** Reference: `jmapgen_monster` reads `group` (`mapgen.cpp:3110-3120`) and its `apply` asks
/// `MonsterGroupManager::GetResultFromGroup` for the member, once per entry (`:3266-3275`) - so a pack is the **group's answer repeated**, not a fresh draw per member.
/// The port read the `group` member and then skipped the entry, so a mapgen that places a horde placed nothing.
#[test]
fn a_mapgens_place_monster_draws_from_a_group() {
    let mut world = a_world(Vec::new());
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[
            cdda_sim::monster_registry::MonsterKind {
                id: "mon_zombie",
                hp: 40,
            },
            cdda_sim::monster_registry::MonsterKind {
                id: "mon_dog",
                hp: 25,
            },
        ],
    );
    // A group whose only member is the zombie, so the draw has one answer and the test cannot pass on a lucky weight.
    cdda_sim::monster_groups::set_groups(
        &mut world,
        vec![cdda_content::monster_groups::MonsterGroup {
            id: "beasts".to_owned(),
            entries: vec![cdda_content::monster_groups::MonsterGroupEntry {
                kind: cdda_content::monster_groups::EntryKind::Monster("mon_zombie".to_owned()),
                weight: 1,
                cost_multiplier: 1,
                pack: (1, 1),
            }],
        }],
    );
    let horde = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: vec![cdda_content::mapgen::MonsterPlacement {
            monster: None,
            group: Some("beasts".to_owned()),
            chance: 100,
            pack_size: (2, 2),
            x: (1, 1),
            y: (0, 0),
            friendly: false,
        }],
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), horde)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert_eq!(
        cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0)),
        Some(2),
        "two squares drawn"
    );
    // **The hit points are what names the member**: a monster spawned from a kind carries that kind's health, so the group's answer is visible in the numbers - the
    // zombie is the only member with forty, where the other kind in the registry has twenty-five.
    let spawned: Vec<i32> = world
        .query::<(Entity, &cdda_sim::creature::Creature)>()
        .iter(&world)
        .map(|(entity, _)| cdda_sim::creature::health_of(&world, entity).unwrap_or(0))
        .collect();
    assert_eq!(
        spawned.len(),
        2,
        "**the group answered once and the pack repeated it**: {spawned:?}"
    );
    assert!(
        spawned.iter().all(|hp| *hp == 40),
        "**and the member is the group's**: {spawned:?}"
    );
}

/// **A mapgen can place monsters that are on the player's side.** Reference: `jmapgen_monster`'s `friendly` (`mapgen.cpp:3124`) is passed to `add_spawn`
/// (`:3268-3270`) and becomes `monster::friendly = -1` - **friendly for good**, the same mark the dialogue's `f_spawn_monster` writes.
///
/// The test also pins the bug this rule surfaced: `make_friendly` clamped its count at zero, so `-1` was stored as `0`, and `is_friendly` asks whether the count is zero -
/// a friendly spawn was **not friendly**. Both halves ride the same store, so one test can see both.
#[test]
fn a_mapgen_can_place_monsters_on_the_players_side() {
    let mut world = a_world(Vec::new());
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 40,
        }],
    );
    let loyal = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: vec![cdda_content::mapgen::MonsterPlacement {
            monster: Some("mon_zombie".to_owned()),
            group: None,
            chance: 100,
            pack_size: (1, 1),
            x: (0, 0),
            y: (0, 0),
            friendly: true,
        }],
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), loyal)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let spawned: Vec<Entity> = world
        .query::<(Entity, &cdda_sim::creature::Creature)>()
        .iter(&world)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(spawned.len(), 1, "one zombie was placed");
    assert!(
        cdda_sim::friendly::is_friendly(&world, spawned[0]),
        "**and it is on the player's side**"
    );
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, spawned[0]),
        cdda_sim::friendly::FOR_EVER,
        "**for good, not for a count of turns**"
    );
}

/// **A mapgen's toilet is furniture with water in it.** Reference: `jmapgen_toilet` (`mapgen.cpp:2824-2845`) reads an `amount` of charges and calls `place_toilet`, which
/// sets `f_toilet` and drops a `water` item with those charges (`mapgen.cpp:7296-7301`); an amount of zero means the default of twenty-four (`map.h:1679`).
///
/// The member is one the data uses **298 times** and the port read not at all, so every bathroom in every map was an empty square where a toilet stands.
#[test]
fn a_mapgens_toilet_is_furniture_with_water_in_it() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(&mut world, [("f_toilet".to_owned(), 44u32)]);
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "water",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let bathroom = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["T.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('T', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: [('T', (0, 0))].into_iter().collect(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), bathroom)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        Some(44),
        "**the toilet is furniture on the square**"
    );
    let water: Vec<i32> = cdda_sim::items_at(&mut world, (0, 0))
        .into_iter()
        .filter_map(|item| {
            world
                .get::<cdda_sim::GroundItem>(item)
                .map(|held| held.charges)
        })
        .collect();
    assert_eq!(
        water,
        vec![24],
        "**and the data's default of twenty-four charges is in it**"
    );
}

/// **A mapgen's liquid is a bottle on the counter.** Reference: `jmapgen_liquid_item` (`mapgen.cpp:2900-2944`) - one chance in `chance` of being there, charges drawn from the
/// `amount` range, and a **fuel** multiplied by a hundred because the reference counts fuel in hundreds of charges (`:2929-2934`).
///
/// The data uses `liquids` **106 times**: every kitchen's water, every garage's gasoline, every hospital's blood was an empty square.
#[test]
fn a_mapgens_liquid_is_poured_with_the_charges_the_data_asks_for() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "water_clean",
                volume_ml: 0,
                weight_g: 0,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
            cdda_sim::item_registry::Kind {
                id: "gasoline",
                volume_ml: 0,
                weight_g: 0,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
        ],
    );
    let kitchen = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["WG".to_owned()],
        terrain: [
            ('W', Piece::Id("t_house_one".to_owned())),
            ('G', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: [
            (
                'W',
                cdda_content::mapgen::LiquidPlacement {
                    liquid: "water_clean".to_owned(),
                    amount: (2, 2),
                    chance: 1,
                },
            ),
            (
                'G',
                cdda_content::mapgen::LiquidPlacement {
                    liquid: "gasoline".to_owned(),
                    amount: (1, 1),
                    chance: 1,
                },
            ),
        ]
        .into_iter()
        .collect(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), kitchen)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let charges_at = |world: &mut World, spot| -> Vec<i32> {
        cdda_sim::items_at(world, spot)
            .into_iter()
            .filter_map(|item| {
                world
                    .get::<cdda_sim::GroundItem>(item)
                    .map(|held| held.charges)
            })
            .collect()
    };
    assert_eq!(
        charges_at(&mut world, (0, 0)),
        vec![2],
        "**the water is two charges, as the data's amount says**"
    );
    assert_eq!(
        charges_at(&mut world, (1, 0)),
        vec![100],
        "**and the fuel is a hundred times one, because fuel is counted in hundreds**"
    );
}

/// **A mapgen scatters an item group over a rectangle.** Reference: `jmapgen_item_group::apply` (`mapgen.cpp:3009-3015`) calls `map::place_items` (`:7376-7420`), which draws
/// `roll_remainder( chance / 100 )` spawns and puts each on a **random square of the rectangle**. `repeat` places the entry that many times, each with its own count.
///
/// The data uses `place_items` **428 times**: a kitchen's drawers, a surgery's tray, a soldier's kit. Without it every room in every map is furnished and empty.
#[test]
fn a_mapgen_scatters_an_item_group_over_a_rectangle() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    cdda_sim::item_groups::set_groups(
        &mut world,
        [(
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
        )]
        .into_iter()
        .collect(),
    );
    let scattered = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned(), "..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: vec![cdda_content::mapgen::AreaItemPlacement {
            group: "loot_group".to_owned(),
            chance: 100,
            repeat: (3, 3),
            x: (0, 1),
            y: (0, 1),
        }],
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), scattered)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let mut rags = 0;
    for x in 0..2 {
        for y in 0..2 {
            rags += cdda_sim::items_at(&mut world, (x, y)).len();
        }
    }
    assert_eq!(
        rags, 3,
        "**three placements of one rag each, scattered inside the rectangle**"
    );
}

/// **A mapgen drops loot on one square of a rectangle.** Reference: `jmapgen_loot::apply` (`mapgen.cpp:3056-3065`) - `rng(0, 99) < chance` is a **percentage** of placing
/// anything at all, the square is a random point in the rectangle, and there is **no repeat**: one entry is one roll.
///
/// `place_loot` is the mapgen's **most used member, 669 times**, and it is the one shape the data uses for the thing you came to the room for: the gun in the safe, the
/// first aid kit on the shelf, the corpse's wallet.
#[test]
fn a_mapgen_drops_loot_on_one_square_of_a_rectangle() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    cdda_sim::item_groups::set_groups(
        &mut world,
        [(
            "kit".to_owned(),
            cdda_content::item_group_draw::Definition {
                id: "kit".to_owned(),
                distribution: false,
                entries: vec![cdda_content::item_group_draw::Entry::Item {
                    id: "rag".to_owned(),
                    probability: 100,
                    count: (2, 2),
                    event: None,
                }],
            },
        )]
        .into_iter()
        .collect(),
    );
    let looting = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: vec![
            // A single item, always there, on the third column.
            cdda_content::mapgen::LootPlacement {
                item: Some("rag".to_owned()),
                group: None,
                chance: 100,
                x: (2, 2),
                y: (0, 0),
            },
            // A group, never there, on the last column: the chance is a percentage of **placing anything at all**.
            cdda_content::mapgen::LootPlacement {
                item: None,
                group: Some("kit".to_owned()),
                chance: 0,
                x: (3, 3),
                y: (0, 0),
            },
        ],
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), looting)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::items_at(&mut world, (2, 0)).len(),
        1,
        "**the single item is on its square**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (3, 0)).len(),
        0,
        "**and a chance of zero places nothing at all**"
    );
}

/// **A locker holds what the mapgen puts in it.** Reference: `jmapgen_sealed_item::apply` (`mapgen.cpp:4049-4060`) - the chance is a percentage of the whole entry, the
/// contents are placed, and the furniture is set **over** them last. That order is the rule: a locker's contents are inside it, not on the floor beside it.
///
/// The data uses `sealed_item` **77 times**: every locker, cabinet, crate and vending machine that is not empty. The port read none of them, so every one was a bare
/// furniture square holding nothing.
#[test]
fn a_sealed_item_is_furniture_with_something_inside_it() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(&mut world, [("f_locker".to_owned(), 51u32)]);
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let lockers = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["L.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('L', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: [(
            'L',
            cdda_content::mapgen::SealedPlacement {
                furniture: "f_locker".to_owned(),
                chance: 100,
                item: Some(cdda_content::mapgen::SealedItem {
                    id: "rag".to_owned(),
                    amount: (2, 2),
                    chance: 100,
                }),
                group: None,
            },
        )]
        .into_iter()
        .collect(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), lockers)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        Some(51),
        "**the locker is furniture on the square**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        2,
        "**and the two rags are in it, on the same square as the furniture**"
    );
}

/// **A vending machine is furniture and a stock.** Reference: `jmapgen_vending_machine::apply` (`mapgen.cpp:2803-2812`) calls `place_vending` (`:7306-7336`), which picks one
/// of **six** furniture ids by `powered`, `reinforced` and `networked`, then stocks the square with the machine's item group.
///
/// The data uses `vendingmachines` **118 times**, and the port read none of them: every machine in every shop was a bare square, and the flag combinations that choose the
/// furniture went nowhere. The test takes two machines with different flags so the choice itself is checked, not just "some furniture was written".
#[test]
fn a_vending_machine_is_furniture_and_a_stock() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(
        &mut world,
        [
            ("f_vending_c_off".to_owned(), 61u32),
            ("f_vending_reinforced".to_owned(), 62u32),
        ],
    );
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    cdda_sim::item_groups::set_groups(
        &mut world,
        [(
            "default_vending_machine".to_owned(),
            cdda_content::item_group_draw::Definition {
                id: "default_vending_machine".to_owned(),
                distribution: false,
                entries: vec![cdda_content::item_group_draw::Entry::Item {
                    id: "rag".to_owned(),
                    probability: 100,
                    count: (2, 2),
                    event: None,
                }],
            },
        )]
        .into_iter()
        .collect(),
    );
    let machines = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["VR".to_owned()],
        terrain: [
            ('V', Piece::Id("t_house_one".to_owned())),
            ('R', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        vending_machines: [
            (
                'V',
                cdda_content::mapgen::VendingPlacement {
                    group: "default_vending_machine".to_owned(),
                    reinforced: false,
                    networked: false,
                    powered: false,
                    lootable: false,
                },
            ),
            (
                'R',
                cdda_content::mapgen::VendingPlacement {
                    group: "default_vending_machine".to_owned(),
                    reinforced: true,
                    networked: false,
                    powered: true,
                    lootable: false,
                },
            ),
        ]
        .into_iter()
        .collect(),
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), machines)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        Some(61),
        "**a machine with no power is the dark one**"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (1, 0, 0)),
        Some(62),
        "**and a powered, armoured one is its own furniture**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        2,
        "**and it is stocked**"
    );
}

/// **A sign is furniture that carries words.** Reference: `jmapgen_sign::apply` (`mapgen.cpp:2701-2721`): the furniture (default `f_sign`) goes on the square and
/// `set_signage` stores the text, which is the part a player reads - a shop's name, a lab's warning, a graffito.
///
/// The data uses `signs` **31 times**. The port read none of them and had no place to put the words, so every sign was silent furniture.
#[test]
fn a_sign_is_furniture_that_carries_words() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(&mut world, [("f_sign".to_owned(), 71u32)]);
    let shop = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["S.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('S', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: [(
            'S',
            cdda_content::mapgen::SignPlacement {
                furniture: "f_sign".to_owned(),
                text: "OPEN".to_owned(),
            },
        )]
        .into_iter()
        .collect(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), shop)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        Some(71),
        "**the sign is furniture**"
    );
    assert_eq!(
        cdda_sim::signage::text_at(&world, (0, 0, 0)),
        Some("OPEN".to_owned()),
        "**and it says what the data wrote**"
    );
    assert_eq!(
        cdda_sim::signage::census(&mut world),
        1,
        "**one sign in the store, not a copy beside it**"
    );
}

/// **A gas pump is terrain with fuel in it.** Reference: `jmapgen_gaspump::apply` (`mapgen.cpp:2872-2892`) and `place_gas_pump` (`:7278-7295`): the charges are the
/// `amount` **times a hundred**, no amount is a randomly filled pump, and the square becomes **terrain** - `t_gas_pump` or `t_diesel_pump` - rather than furniture.
///
/// The data uses `gaspumps` **29 times**, and the distinction from a toilet matters: a pump is not furniture standing on a floor, it is the ground itself, which is why the
/// test reads terrain and not furniture.
#[test]
fn a_gas_pump_is_terrain_with_fuel_in_it() {
    let mut world = a_world(Vec::new());
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_gas_pump".to_owned(), 81u32),
            ("t_diesel_pump".to_owned(), 82u32),
        ],
    );
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "gasoline",
                volume_ml: 0,
                weight_g: 0,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
            cdda_sim::item_registry::Kind {
                id: "diesel",
                volume_ml: 0,
                weight_g: 0,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
        ],
    );
    let forecourt = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["G.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('G', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: [(
            'G',
            cdda_content::mapgen::GasPumpPlacement {
                amount: (1, 1),
                fuel: Some("diesel".to_owned()),
            },
        )]
        .into_iter()
        .collect(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), forecourt)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(13));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(82),
        "**the diesel pump is the ground itself**"
    );
    let charges: Vec<i32> = cdda_sim::items_at(&mut world, (0, 0))
        .into_iter()
        .filter_map(|item| {
            world
                .get::<cdda_sim::GroundItem>(item)
                .map(|held| held.charges)
        })
        .collect();
    assert_eq!(
        charges,
        vec![100],
        "**an amount of one is a hundred charges of fuel**"
    );
}

/// **A nested mapgen is a whole chunk drawn shifted.** Reference: `jmapgen_nested::apply` (`mapgen.cpp:4488-4514`) picks an id from the weighted `chunks` list and `nest`s it;
/// `jmapgen_objects::apply` then offsets every piece by **minus the marker's position** (`:6145`), so the nested square standing at the marker lands on the outer chunk's
/// origin and the rest extends up and left of it.
///
/// The data uses `nested` **335 times** - more than any member but `place_loot` - and it is how a house gets its kitchen, its bedroom and its bathroom: the outer chunk is
/// walls and doors and the nested chunk is the room inside.
#[test]
fn a_nested_mapgen_is_drawn_shifted_by_its_marker() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_nested_floor".to_owned(), 91u32),
        ]
        .into_iter()
        .collect(),
    );
    let room = MapgenObject {
        om_terrain: "nested_room".to_owned(),
        rows: vec!["NN".to_owned(), "NN".to_owned()],
        terrain: [('N', Piece::Id("t_nested_floor".to_owned()))]
            .into_iter()
            .collect(),
        ..a_plain_mapgen()
    };
    let house = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["n...".to_owned(), "....".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('n', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: [(
            'n',
            cdda_content::mapgen::NestedPlacement {
                chunks: vec![("nested_room".to_owned(), 100)],
            },
        )]
        .into_iter()
        .collect(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(
        &mut world,
        [
            ("house".to_owned(), house),
            ("nested_room".to_owned(), room),
        ],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(17));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(91),
        "**the nested chunk's corner lands on the marker's square**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(91),
        "**and its far corner follows the rows**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 0, 0)),
        Some(11),
        "**the outer chunk's own floor is untouched beyond it**"
    );
}

/// **A nested chunk stands at its marker and hangs off the edge that it crosses.** Reference: `jmapgen_setmap::apply` **adds** the offset (`mapgen.cpp:5627-5631`), so the
/// nested square `( col, row )` lands on `marker + ( col, row )`, and the part beyond the submap's edge is clipped.
///
/// This test pins the **sign**, which the first one cannot: with the marker at the origin, plus and minus are the same. A marker inside the chunk tells them apart.
#[test]
fn a_nested_chunk_stands_at_its_marker() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_nested_floor".to_owned(), 91u32),
        ]
        .into_iter()
        .collect(),
    );
    let room = MapgenObject {
        om_terrain: "nested_room".to_owned(),
        rows: vec!["NN".to_owned(), "NN".to_owned()],
        terrain: [('N', Piece::Id("t_nested_floor".to_owned()))]
            .into_iter()
            .collect(),
        ..a_plain_mapgen()
    };
    let house = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned(), ".n..".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('n', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: [(
            'n',
            cdda_content::mapgen::NestedPlacement {
                chunks: vec![("nested_room".to_owned(), 100)],
            },
        )]
        .into_iter()
        .collect(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(
        &mut world,
        [
            ("house".to_owned(), house),
            ("nested_room".to_owned(), room),
        ],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(19));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(91),
        "**the nested chunk's square zero landed on the marker, which is at ( 1, 1 )**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 2, 0)),
        Some(91),
        "**and its far corner on the square two along from the marker**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(11),
        "**the square up and left of the marker is still the outer floor**"
    );
}

/// **A rectangle is a field of candidates, not a box that is filled.** Reference: `jmapgen_objects::apply` (`mapgen.cpp:6161-6164`) applies a piece `repeat` times, and each
/// application draws **one** square from the rectangle (`jmapgen_int::get`) - so `place_terrain` with `repeat: 4` over a 3×3 box writes **four** squares, not nine.
///
/// The data uses `place_terrain` 56 times, `place_furniture` 65 and `place_traps` 35, and the convention is the one that reads most easily backwards: a minefield is
/// `repeat: 200` over a whole chunk, which is nothing like "fill the chunk with traps".
#[test]
fn a_rectangle_is_a_field_of_candidates_and_repeat_is_how_many_land() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_rubble".to_owned(), 77u32),
        ]
        .into_iter()
        .collect(),
    );
    // The rows resolve through the mapgen's own code table and the objects through the type registries, which is the split the loader has: a row's symbol names a
    // terrain in the mapgen, and a `place_terrain` entry names one in the terrain table.
    cdda_sim::terrain_types::set_types(&mut world, [("t_rubble".to_owned(), 77u32)]);
    cdda_sim::furniture_types::set_types(&mut world, [("f_rubble".to_owned(), 78u32)]);
    let field = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["...".to_owned(), "...".to_owned(), "...".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: vec![cdda_content::mapgen::AreaSet {
            id: "t_rubble".to_owned(),
            x: (0, 2),
            y: (0, 2),
            repeat: (4, 4),
        }],
        place_furniture: vec![cdda_content::mapgen::AreaSet {
            id: "f_rubble".to_owned(),
            x: (0, 0),
            y: (0, 0),
            repeat: (1, 1),
        }],
        place_traps: vec![cdda_content::mapgen::AreaSet {
            id: "tr_bear".to_owned(),
            x: (2, 2),
            y: (2, 2),
            repeat: (1, 1),
        }],
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), field)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(23));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let mut rubbled = 0;
    for x in 0..3 {
        for y in 0..3 {
            if cdda_sim::square::terrain_at(&world, (x, y, 0)) == Some(77) {
                rubbled += 1;
            }
        }
    }
    assert_eq!(
        rubbled, 4,
        "**four squares of nine, because repeat is four**"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        Some(78),
        "**the furniture went where its rectangle said**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (2, 2, 0)),
        Some("tr_bear".to_owned()),
        "**and the trap did too**"
    );
}

/// **The `item` symbol map is the single-item form of the item layer.** Reference: `jmapgen_spawn_item::apply` (`mapgen.cpp:3500-3520`) - a chance of a hundred is **one**
/// spawn, anything less is `roll_remainder( chance / 100 )`, and each spawn drops `amount` items.
///
/// The port read the **group** form (`items`) and not this one, and this is the form the data uses most: it is how a mapgen puts one specific thing on one specific square -
/// the axe in the shed, the book on the desk, the cigarettes on the table.
///
/// The same fixture checks the **field symbol map** (`fields`, `jmapgen_field`), which is the third of the symbol maps the loader left unread: a patch of smoke on the square
/// the row drew.
#[test]
fn the_item_and_fields_symbol_maps_place_what_their_symbols_name() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_smoke", 1, 3)]);
    let room = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["IF".to_owned()],
        terrain: [
            ('I', Piece::Id("t_house_one".to_owned())),
            ('F', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: [(
            'I',
            cdda_content::mapgen::SingleItemPlacement {
                item: "rag".to_owned(),
                amount: (2, 2),
                chance: 100,
            },
        )]
        .into_iter()
        .collect(),
        field_symbols: [(
            'F',
            cdda_content::mapgen::FieldPatch {
                field: "fd_smoke".to_owned(),
                intensity: (3, 3),
                chance: 100,
            },
        )]
        .into_iter()
        .collect(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), room)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(29));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        2,
        "**one spawn of two rags, because a chance of a hundred is one spawn**"
    );
    let smoke = cdda_sim::field_registry::id_of(&mut world, "fd_smoke").expect("the field");
    assert_eq!(
        cdda_sim::field_strength_at(&world, (1, 0, 0), smoke),
        3,
        "**and the smoke is on its own square at the strength the data wrote**"
    );
}

/// **A terminal is furniture and a record.** Reference: `jmapgen_computer::apply` (`mapgen.cpp:3878-3905`): the furniture is `f_console` and `add_computer` stores the
/// record - a name, a security level, the actions it offers - which is what a hacking pass reads later.
///
/// The data uses `computers` **85 times**, and the port read none of them: every console in every lab, church and vault was a piece of furniture with nothing behind it. The
/// test checks that the record is **on the square** and that its options survived the read, because a terminal whose menu is empty is not the reference's terminal.
#[test]
fn a_terminal_is_furniture_and_a_record() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(&mut world, [("f_console".to_owned(), 85u32)]);
    let church = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["C.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('C', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: [(
            'C',
            cdda_content::mapgen::ComputerPlacement {
                name: "Church Bells 1.0".to_owned(),
                access_denied: "Access denied.".to_owned(),
                security: 2,
                options: vec![("Toll Bells".to_owned(), "toll".to_owned())],
                eocs: vec!["EOC_bells".to_owned()],
                chat_topics: Vec::new(),
            },
        )]
        .into_iter()
        .collect(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), church)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(31));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        Some(85),
        "**the console is furniture on the square**"
    );
    let terminal = cdda_sim::computer_terminal::computer_at(&world, (0, 0, 0)).expect("a terminal");
    assert_eq!(terminal.name, "Church Bells 1.0");
    assert_eq!(terminal.security, 2, "**its security came through**");
    assert_eq!(
        terminal.options,
        vec![cdda_sim::computer_terminal::TerminalOption {
            name: "Toll Bells".to_owned(),
            action: "toll".to_owned(),
        }],
        "**and so did the action it offers**"
    );
    assert_eq!(
        cdda_sim::computer_terminal::census(&mut world),
        1,
        "**one record in the store, not a copy beside it**"
    );
}

/// **A `remove_all` strips its squares down to the terrain, and it is the first phase there is.** Reference: `jmapgen_remove_all::apply` (`mapgen.cpp:4177-4205`) walks
/// **every** square of its rectangle - unlike every other rectangle in a mapgen - clearing furniture, items, traps and fields; and `mapgen.h:88-97` puts `removal` before
/// `terrain`, so it runs before the fill and the rows put anything back.
///
/// The data uses it 16 times, and it is how a mapgen **unsets** what a predecessor or an earlier overlay left: a stairwell cut through a house, a room the vault dug out.
/// The test gives the mapgen rows that draw **nothing** on the removed square, because a removal that only works where the mapgen redraws would be no removal at all.
#[test]
fn a_remove_all_strips_its_squares_before_anything_is_drawn() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(&mut world, [("f_table".to_owned(), 93u32)]);
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    // What an earlier overlay left behind: a table and a rag on each of two squares.
    let old_furniture = cdda_sim::furniture_types::code_of(&world, "f_table").expect("the table");
    let rag = cdda_sim::item_registry::id_of(&mut world, "rag").expect("the rag");
    cdda_sim::square::set_terrain(&mut world, (0, 0, 0), 11);
    cdda_sim::square::set_furniture(&mut world, (0, 0, 0), old_furniture);
    cdda_sim::drop_item(&mut world, (0, 0), rag, 1);
    cdda_sim::square::set_furniture(&mut world, (1, 0, 0), old_furniture);
    cdda_sim::drop_item(&mut world, (1, 0), rag, 1);
    let stripped = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: vec![cdda_content::mapgen::RemoveArea {
            x: (0, 0),
            y: (0, 0),
        }],
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), stripped)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(37));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        None,
        "**the table inside the rectangle is gone - furniture code zero clears the square - and the row drew none**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        0,
        "**and so is the rag**"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (1, 0, 0)),
        Some(old_furniture),
        "**one square outside the rectangle, the table still stands**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        1,
        "**and the rag is still on it**"
    );
}

/// **A monster can be a symbol, and that is how the data most often says it.** Reference: `jmapgen_monster` is loaded under **`monster`** as a symbol map (`mapgen.cpp:5130`)
/// and under `place_monster` as a rectangle list (`:5402`); the piece is the same, the square is what differs. The symbol map has **1,748 uses** in the data - more than any
/// other member - and the port read only the rectangle form, so every monster drawn as a symbol on a map was missing.
#[test]
fn a_monster_can_be_a_symbol() {
    let mut world = a_world(Vec::new());
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 80,
        }],
    );
    let house = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["Z.".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('Z', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: [(
            'Z',
            cdda_content::mapgen::SymbolMonster {
                monster: Some("mon_zombie".to_owned()),
                group: None,
                chance: 100,
                pack_size: (1, 1),
                friendly: false,
            },
        )]
        .into_iter()
        .collect(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), house)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(41));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::creature::creature_at(&world, (0, 0, 0)).len(),
        1,
        "**one zombie on the symbol's square**"
    );
    assert!(
        cdda_sim::creature::creature_at(&world, (1, 0, 0)).is_empty(),
        "**and the plain square is empty**"
    );
}

/// **`place_nested` is the same piece as the `nested` symbol map, with the dice choosing the marker.** Reference: `jmapgen_nested::nest` is called with `( x.get(), y.get(), z.get() )`
/// (`mapgen.cpp:4502`), so the piece draws one square from its own range - the symbol form fixed it to the symbol's square, this form leaves it to the rectangle.
///
/// **1,382 uses** in the data, and the way it is written says what it is for: a 4×4 sewer blockage at `x: 10`, `y: [ 0, 20 ]` - a small chunk standing **at** its marker, not a
/// whole-chunk overlay.
#[test]
fn a_place_nested_entry_draws_a_small_chunk_at_its_marker() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_nested_floor".to_owned(), 91u32),
        ]
        .into_iter()
        .collect(),
    );
    let room = MapgenObject {
        om_terrain: "nested_room".to_owned(),
        rows: vec!["NN".to_owned(), "NN".to_owned()],
        terrain: [('N', Piece::Id("t_nested_floor".to_owned()))]
            .into_iter()
            .collect(),
        ..a_plain_mapgen()
    };
    let house = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec![".".repeat(24); 24],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: vec![cdda_content::mapgen::AreaNested {
            x: (10, 10),
            y: (5, 5),
            chunks: vec![("nested_room".to_owned(), 100)],
        }],
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(
        &mut world,
        [
            ("house".to_owned(), house),
            ("nested_room".to_owned(), room),
        ],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(43));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (10, 5, 0)),
        Some(91),
        "**the chunk's square zero landed on the marker at ( 10, 5 )**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (11, 6, 0)),
        Some(91),
        "**and its far corner two along**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (9, 5, 0)),
        Some(11),
        "**one square before the marker is still the outer floor**"
    );
}

/// **The rectangle forms of two symbol maps.** `jmapgen_spawn_item` is loaded under `item` (the symbol map) and under **`place_item`** (`mapgen.cpp:5390`, 165 uses);
/// `jmapgen_sign` under `signs` and under **`place_signs`** (`:5393`, 127 uses). Same pieces, same rules, squares written out instead of symbols.
///
/// The test puts both on fixed squares, so it pins what the rectangle form **is** - the same placement the symbol form makes, at the square the entry names - and not the
/// rectangle's dice, which the area-set test already covers.
#[test]
fn a_rectangle_can_place_a_single_item_and_a_sign() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    cdda_sim::furniture_types::set_types(&mut world, [("f_sign".to_owned(), 87u32)]);
    let room = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: vec![cdda_content::mapgen::AreaSingleItem {
            item: "rag".to_owned(),
            amount: (2, 2),
            chance: 100,
            x: (1, 1),
            y: (0, 0),
        }],
        place_signs: vec![cdda_content::mapgen::AreaSign {
            furniture: "f_sign".to_owned(),
            text: "No Entry".to_owned(),
            x: (3, 3),
            y: (0, 0),
        }],
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), room)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(47));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        2,
        "**two rags on the square the entry named**"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (3, 0, 0)),
        Some(87),
        "**the sign's furniture on its own square**"
    );
    assert_eq!(
        cdda_sim::signage::text_at(&world, (3, 0, 0)),
        Some("No Entry".to_owned()),
        "**and the words it says**"
    );
}

/// **`place_liquids` is the `liquids` symbol map with the square written out.** Reference: `mapgen.cpp:5398` loads the same `jmapgen_liquid_item` under both names - 40 uses of
/// the rectangle form in the data, and the port read only the symbol form.
///
/// The rule has the reference's **one-in-n** chance, not a percentage, and multiplies a **fuel's** charges by a hundred; the test pins the fuel branch, because water and a full
/// tank landing on the same square with the same number would hide the one rule that is easy to lose when two square sources share a shape.
#[test]
fn a_rectangle_can_place_a_liquid() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "gasoline",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let room = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: vec![cdda_content::mapgen::AreaLiquid {
            liquid: "gasoline".to_owned(),
            amount: (2, 2),
            chance: 1,
            x: (1, 1),
            y: (0, 0),
        }],
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), room)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(53));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let spilled = cdda_sim::items_at(&mut world, (1, 0));
    assert_eq!(spilled.len(), 1, "**one puddle on the square**");
    let charge = spilled
        .first()
        .and_then(|entity| world.get::<cdda_sim::GroundItem>(*entity))
        .map(|item| item.charges);
    assert_eq!(
        charge,
        Some(200),
        "**two amounts of gasoline is two hundred charges, because a fuel counts in hundreds**"
    );
}

/// **A rotated mapgen is the whole chunk turned, not its rows re-read.** Reference: `mapgen_function_json::generate` draws at rotation zero into a pre-rotated map and rotates
/// it back (`mapgen.cpp:6072-6096`), which leaves everything it drew turned by `rot`; `point::rotate( 1, dim )` sends `( 0, 0 )` to `( dim.y - 1, 0 )`, a quarter turn
/// **clockwise**.
///
/// The data writes `rotation` **2,198 times**, so a port that ignores it draws every rotated mapgen in the wrong orientation - and the layers would still agree with each other,
/// which is what makes the bug look like nothing at all. The test pins a terrain square, a furniture square and an item, because "the chunk was turned" has to be true of
/// every layer at once.
#[test]
fn a_rotated_mapgen_turns_its_whole_chunk_clockwise() {
    let mut world = a_world(Vec::new());
    let turned = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["AF.".to_owned(), "...".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('A', Piece::Id("t_rubble".to_owned())),
            ('F', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: [(
            'F',
            cdda_content::mapgen::FieldPatch {
                field: "fd_smoke".to_owned(),
                intensity: (1, 1),
                chance: 100,
            },
        )]
        .into_iter()
        .collect(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (1, 1),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: Some("t_house_one".to_owned()),
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_rubble".to_owned(), 77u32),
        ]
        .into_iter()
        .collect(),
    );
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_smoke", 1, 3)]);
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), turned)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(59));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (23, 0, 0)),
        Some(77),
        "**the rubble stood at ( 0, 0 ) and a clockwise quarter turn puts it at ( 23, 0 )**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (22, 0, 0)),
        Some(11),
        "**and the square it came from is the fill again**"
    );
    let smoke = cdda_sim::field_registry::id_of(&mut world, "fd_smoke").expect("the field");
    assert_eq!(
        cdda_sim::field_strength_at(&world, (23, 1, 0), smoke),
        1,
        "**the smoke that stood at ( 1, 0 ) is at ( 23, 1 ), one square below the rubble**"
    );
}

/// **The `set` list is the oldest way the data names one square's layer, and it is still the most common.** Reference: `jmapgen_setmap::apply` (`mapgen.cpp:5622-5640`):
/// `one_in( chance )`, then `repeat` squares drawn from the entry's range, each writing terrain, furniture or a trap.
///
/// The data writes 1,374 `set` entries, **1,129 of them point entries**, and 1,063 of those name one of the three layers the port has. A mapgen that reads symbols and not
/// `set` draws its walls and never its scattered dirt, its underbrush or its traps - which is why the test checks all three layers on one mapgen.
#[test]
fn a_set_point_writes_the_layer_it_names() {
    let mut world = a_world(Vec::new());
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_dirt".to_owned(), 66u32),
        ]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>(),
    );
    cdda_sim::furniture_types::set_types(&mut world, [("f_underbrush".to_owned(), 67u32)]);
    let scattered = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        set_points: vec![
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::Terrain,
                shape: cdda_content::mapgen::SetShape::Point,
                id: "t_dirt".to_owned(),
                x: (2, 2),
                y: (0, 0),
                x2: (2, 2),
                y2: (0, 0),
                repeat: (1, 1),
                amount: (0, 0),
            },
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::Furniture,
                shape: cdda_content::mapgen::SetShape::Point,
                id: "f_underbrush".to_owned(),
                x: (3, 3),
                y: (0, 0),
                x2: (3, 3),
                y2: (0, 0),
                repeat: (1, 1),
                amount: (0, 0),
            },
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::Trap,
                shape: cdda_content::mapgen::SetShape::Point,
                id: "tr_bear".to_owned(),
                x: (1, 1),
                y: (0, 0),
                x2: (1, 1),
                y2: (0, 0),
                repeat: (1, 1),
                amount: (0, 0),
            },
        ],
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), scattered)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(61));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 0, 0)),
        Some(66),
        "**the dirt the set point named**"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (3, 0, 0)),
        Some(67),
        "**the underbrush on its own square**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 0, 0)),
        Some("tr_bear".to_owned()),
        "**and the trap on its own**"
    );
}

/// **The three shapes a `set` entry can draw, and the one rule each gets wrong if it is guessed.** Reference: `jmapgen_setmap::apply` (`mapgen.cpp:5622-5800`).
///
/// - a **line** is `line_to( p1, p2 )` (`line.cpp:224-238`), which starts walking at `p1` and returns the cells **after** it - so a line does **not** write its own first
///   square. That is the rule worth a test, because "draw a line from A to B" reads as if it includes A;
/// - a **square** is `draw_square` (`drawing_primitives.cpp:22-36`), which **fills** the rectangle between the corners, both included - not an outline;
/// - a **point** is one square.
///
/// The shapes are 245 of the data's `set` entries (163 lines, 82 squares), and they are how a mapgen draws a road across a chunk or a fence around a yard.
#[test]
fn a_set_entry_draws_a_point_a_line_or_a_filled_square() {
    let mut world = a_world(Vec::new());
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_dirt".to_owned(), 66u32),
        ]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>(),
    );
    let shapes = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec![
            "......".to_owned(),
            "......".to_owned(),
            "......".to_owned(),
        ],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        set_points: vec![
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::Terrain,
                shape: cdda_content::mapgen::SetShape::Point,
                id: "t_dirt".to_owned(),
                x: (0, 0),
                y: (2, 2),
                x2: (0, 0),
                y2: (2, 2),
                repeat: (1, 1),
                amount: (0, 0),
            },
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::Terrain,
                shape: cdda_content::mapgen::SetShape::Line,
                id: "t_dirt".to_owned(),
                x: (0, 0),
                y: (0, 0),
                x2: (3, 3),
                y2: (0, 0),
                repeat: (1, 1),
                amount: (0, 0),
            },
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::Terrain,
                shape: cdda_content::mapgen::SetShape::Square,
                id: "t_dirt".to_owned(),
                x: (4, 4),
                y: (1, 1),
                x2: (5, 5),
                y2: (2, 2),
                repeat: (1, 1),
                amount: (0, 0),
            },
        ],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), shapes)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(67));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 2, 0)),
        Some(66),
        "**the point wrote its square**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(11),
        "**the line did NOT write its own first square - line_to returns the cells after p1**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 0, 0)),
        Some(66),
        "**but it wrote the one after it**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (3, 0, 0)),
        Some(66),
        "**and its last, p2 included**"
    );
    for (x, y) in [(4, 1), (5, 1), (4, 2), (5, 2)] {
        assert_eq!(
            cdda_sim::square::terrain_at(&world, (x, y, 0)),
            Some(66),
            "**the square filled ({x}, {y})**"
        );
    }
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (4, 0, 0)),
        Some(11),
        "**and drew nothing above its top edge**"
    );
}

/// **A `set` radiation entry writes an absolute dose, and its range is the dose's - not the square's.** Reference: `jmapgen_setmap::apply`'s radiation arm
/// (`mapgen.cpp:5676-5679`) calls `m.set_radiation( target_pos, val.get() )`, and `val` was read from `amount` (`:1712-1713`).
///
/// The data writes 26 of them - the hot spots of a reactor, a lab's spill, a crater - and a port that reads only the `id` ops drops every one of them. The test puts a dose on
/// a square that already carries radiation, so "set" and "add" cannot be confused: an adjust would answer sixty, and the reference answers thirty.
#[test]
fn a_set_radiation_entry_writes_a_dose() {
    let mut world = a_world(Vec::new());
    cdda_sim::radiation::set(&mut world, (4, 4, 0), 12);
    let hotspot = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        set_points: Vec::new(),
        set_radiation: vec![cdda_content::mapgen::SetRadiation {
            amount: (30, 30),
            x: (4, 4),
            y: (4, 4),
            repeat: (1, 1),
        }],
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), hotspot)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(71));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::radiation::dose_at(&world, (4, 4, 0)),
        30,
        "**thirty, not forty-two: the entry sets the dose, it does not add to it**"
    );
}

/// **Rubble is a furniture and three hardcoded piles of debris.** Reference: `jmapgen_make_rubble` (`mapgen.cpp:3796-3837`) and `map::make_rubble` (`map.cpp:3239-3291`): the
/// furniture is set, the floor is forced only when the entry says `overwrite`, and the debris depends on which furniture it is - wreckage drops steel and scrap, rock rubble
/// drops rocks, and plain rubble drops splinters and nails.
///
/// The data writes `rubble` 12 times and `place_rubble` 23. The test pins the **plain** branch's two draws, because they are the ones a port is tempted to collapse into a
/// single item: two to eight splinters **and** twenty to fifty nails, both from the same square.
#[test]
fn rubble_is_furniture_and_debris() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(&mut world, [("f_rubble".to_owned(), 96u32)]);
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "splinter",
                volume_ml: 0,
                weight_g: 0,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
            cdda_sim::item_registry::Kind {
                id: "nail",
                volume_ml: 0,
                weight_g: 0,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
        ],
    );
    let collapsed = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["R..".to_owned()],
        terrain: [
            ('.', Piece::Id("t_house_one".to_owned())),
            ('R', Piece::Id("t_house_one".to_owned())),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        rubble: [(
            'R',
            cdda_content::mapgen::RubblePlacement {
                rubble_type: "f_rubble".to_owned(),
                floor_type: "t_dirt".to_owned(),
                items: true,
                overwrite: false,
            },
        )]
        .into_iter()
        .collect(),
        place_rubble: Vec::new(),
        set_variables: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), collapsed)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(73));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (0, 0, 0)),
        Some(96),
        "**the rubble stands as furniture**"
    );
    let dropped = cdda_sim::items_at(&mut world, (0, 0));
    let charges: i32 = dropped
        .iter()
        .filter_map(|entity| world.get::<cdda_sim::GroundItem>(*entity))
        .map(|item| item.charges)
        .sum();
    assert!(
        (22..=58).contains(&charges),
        "**two to eight splinters and twenty to fifty nails: the nails are ONE STACK of twenty to fifty charges, so the total is twenty-two to fifty-eight and was {charges}**"
    );
}

/// **A `set` variable names a square, and the name outlives the mapgen.** Reference: `jmapgen_setmap`'s variable arm (`mapgen.cpp:5735-5737`) queues
/// `name -> absolute square`, and `set_queued_points()` writes every queued point into `global_variables` when the map is finished (`:6098`, `:8762-8769`).
///
/// The data writes 230 of them, and they are the bridge between a map and everything that later refers to a place by name: a mission's target, a teleporter's destination, a
/// dialogue's `u_location_variable`. The test reads the **global** value store, because that is the one place the reference writes them to and the one place a later reader
/// looks - a per-mapgen store would be a second source of truth with a shorter life.
#[test]
fn a_set_variable_names_a_square_in_the_global_store() {
    let mut world = a_world(Vec::new());
    let lab = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: vec![cdda_content::mapgen::SetVariable {
            name: "LIXA_return_tp".to_owned(),
            x: (9, 9),
            y: (13, 13),
        }],
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), lab)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(79));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let holder = cdda_sim::dialogue_values::global_holder(&world).expect("the global store");
    assert_eq!(
        cdda_sim::dialogue_values::value_of(&world, holder, "LIXA_return_tp"),
        Some(cdda_sim::dialogue_values::Value::Position { x: 9, y: 13, z: 0 }),
        "**the name holds the square the entry named, in world coordinates**"
    );
}

/// **The `set` list can also take things away, and a removal names no id.** Reference: `jmapgen_setmap`'s removal arms (`mapgen.cpp:5627-5655`): `trap_remove` clears the trap,
/// `item_remove` clears the items, `field_remove` clears the fields, and `creature_remove` kills what stands there.
///
/// The data writes 18 of them (3 `trap_remove`, 10 `item_remove`, 4 `field_remove`, 1 `creature_remove`), and they are the op that makes a mapgen able to *undo*: a shop whose
/// stock was placed by a predecessor, a cleared corridor, a corpse taken away. The test puts an item and a trap on two squares and takes them off, because a removal that removes
/// nothing is indistinguishable from one that removes everything.
#[test]
fn a_set_removal_takes_away_what_it_names() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let kind = cdda_sim::item_registry::id_of(&mut world, "rock").expect("rock");
    cdda_sim::drop_item(&mut world, (2, 0), kind, 1);
    cdda_sim::square::set_trap(&mut world, (3, 0, 0), "tr_bear");
    let swept = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        set_points: vec![
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::ItemRemove,
                shape: cdda_content::mapgen::SetShape::Point,
                id: String::new(),
                x: (2, 2),
                y: (0, 0),
                x2: (2, 2),
                y2: (0, 0),
                repeat: (1, 1),
                amount: (0, 0),
            },
            cdda_content::mapgen::SetPoint {
                kind: cdda_content::mapgen::SetKind::TrapRemove,
                shape: cdda_content::mapgen::SetShape::Point,
                id: String::new(),
                x: (3, 3),
                y: (0, 0),
                x2: (3, 3),
                y2: (0, 0),
                repeat: (1, 1),
                amount: (0, 0),
            },
        ],
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
        parameters: BTreeMap::new(),
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), swept)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(83));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert!(
        cdda_sim::items_at(&mut world, (2, 0)).is_empty(),
        "**item_remove took the rock off the square**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (3, 0, 0)),
        None,
        "**trap_remove took the trap off the square**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        0,
        "**and neither removal touched a square it did not name**"
    );
}

/// **A mapgen that expects a predecessor draws its fallback when the place has none.** Reference: `mapgen.cpp:6047-6062` - `predecessor_mapgen` names the mapgen to draw
/// underneath, and `fallback_predecessor_mapgen` is what `expects_predecessor` answers with (`:6101-6104`) when the `mapgendata` carries no predecessor of its own.
///
/// 57 of the data's mapgens declare a fallback, and they are the ones that say "I sit on a field" or "I sit on a road": without the rule they draw on bare ground, and every
/// square their own rows do not cover stays empty. The test's outer rows map only their first two squares, so the last two can only come from the fallback.
#[test]
fn a_mapgen_with_a_fallback_predecessor_draws_it_below() {
    let mut world = a_world(Vec::new());
    // The mapgen writer reads its own code table, which `a_world` fills with two ids; the fallback's terrain has to be in **that** table, not in a second one.
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_house_one".to_owned(), 11u32),
            ("t_house_two".to_owned(), 12u32),
            ("t_grass".to_owned(), 66u32),
        ]
        .into_iter()
        .collect(),
    );
    let field = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "field".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_grass".to_owned()))]
            .into_iter()
            .collect(),
        ..a_plain_mapgen()
    };
    let house = MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: "house".to_owned(),
        rows: vec!["..~~".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        fallback_predecessor_mapgen: Some("field".to_owned()),
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(
        &mut world,
        [("field".to_owned(), field), ("house".to_owned(), house)],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(89));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(11),
        "**the mapgen's own rows still draw on top**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (3, 0, 0)),
        Some(66),
        "**and the squares the rows leave alone are the fallback's grass**"
    );
}

/// A mapgen with every list empty, for a test that only wants a few of them filled.
fn a_plain_mapgen() -> MapgenObject {
    MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        om_terrain: String::new(),
        rows: Vec::new(),
        terrain: BTreeMap::new(),
        furniture: BTreeMap::new(),
        parameters: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: BTreeMap::new(),
        vending_machines: BTreeMap::new(),
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        nested: BTreeMap::new(),
        place_terrain: Vec::new(),
        place_furniture: Vec::new(),
        place_traps: Vec::new(),
        items_single: BTreeMap::new(),
        field_symbols: BTreeMap::new(),
        computers: BTreeMap::new(),
        remove_all: Vec::new(),
        monster_symbols: BTreeMap::new(),
        rotation: (0, 0),
        place_nested: Vec::new(),
        place_item: Vec::new(),
        place_signs: Vec::new(),
        place_liquids: Vec::new(),
        set_points: Vec::new(),
        set_radiation: Vec::new(),
        set_variables: Vec::new(),
        rubble: BTreeMap::new(),
        place_rubble: Vec::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fallback_predecessor_mapgen: None,
    }
}

/// **A chunk that declares a small size refuses the placements that fall outside it.** Reference: `mapgen.cpp:5105-5110` reads `mapgensize` (the overmap tile's 24 by default)
/// and `check_inbounds` (`:5130-5140`) **defers** a placement whose coordinates are outside it rather than clipping it.
///
/// This is how a palette written for a 24-square chunk is reused at 4x4 or 1x1 - the nested blocks, the sewer pieces, the single-furniture pieces - and a port that ignores the
/// size places furniture ten squares into a four-square chunk, which is not a smaller error than placing nothing: it is furniture in the wall of the piece next door.
#[test]
fn a_small_chunk_refuses_a_placement_outside_its_size() {
    let mut world = a_world(Vec::new());
    cdda_sim::furniture_types::set_types(
        &mut world,
        [("f_counter".to_owned(), 97u32)]
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
    );
    let small = MapgenObject {
        rows: vec![
            "....".to_owned(),
            "....".to_owned(),
            "....".to_owned(),
            "....".to_owned(),
        ],
        place_furniture: vec![
            cdda_content::mapgen::AreaSet {
                id: "f_counter".to_owned(),
                x: (10, 10),
                y: (0, 0),
                repeat: (1, 1),
            },
            cdda_content::mapgen::AreaSet {
                id: "f_counter".to_owned(),
                x: (2, 2),
                y: (1, 1),
                repeat: (1, 1),
            },
        ],
        mapgensize: (4, 4),
        ..a_plain_mapgen()
    };
    let sized = MapgenObject {
        om_terrain: "house".to_owned(),
        ..small
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), sized)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(97));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (10, 0, 0)),
        None,
        "**the placement outside a four-square chunk is not drawn**"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (2, 1, 0)),
        Some(97),
        "**the one inside it is**"
    );
}

/// **A zone is a labelled rectangle, and it covers every square inside it.** Reference: `jmapgen_zone::apply` (`mapgen.cpp:4064-4073`) turns the entry's two corner ranges
/// into absolute squares and records the zone over the rectangle between them.
///
/// The data writes 66 of them, and a zone is how a map says "this room is a base's workshop", "this is a stockpile", or "do not investigate this area" before anything stands
/// there. The test reads a square **inside** the rectangle and one just outside it, because a zone that covered the whole chunk would pass a test that only asked about the
/// middle.
#[test]
fn a_zone_covers_the_rectangle_it_names() {
    let mut world = a_world(Vec::new());
    let base = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        place_zones: vec![cdda_content::mapgen::AreaZone {
            kind: "ZONE_STOCKPILE".to_owned(),
            faction: "no_faction".to_owned(),
            name: "the pile".to_owned(),
            filter: String::new(),
            x: (2, 2),
            y: (3, 3),
            x2: (5, 5),
            y2: (6, 6),
        }],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), base)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(101));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(cdda_sim::zones::census(&mut world), 1, "**one zone**");
    let inside = cdda_sim::zones::zones_at(&mut world, (4, 5, 0));
    assert_eq!(
        inside.len(),
        1,
        "**the square inside the rectangle is covered**"
    );
    assert_eq!(
        inside.first().map(|zone| zone.kind.clone()),
        Some("ZONE_STOCKPILE".to_owned())
    );
    assert_eq!(
        inside.first().map(|zone| zone.name.clone()),
        Some("the pile".to_owned()),
        "**and it kept its name**"
    );
    assert!(
        cdda_sim::zones::zones_at(&mut world, (6, 5, 0)).is_empty(),
        "**a square one column past the corner is not**"
    );
}

/// **The density spawner fills a rectangle with what a group holds, and its budget is spent by each entry's cost.** Reference:
/// `jmapgen_monster_group::apply` (`mapgen.cpp:3093-3100`) to `map::place_spawns` (`:7231-7281`):
///
/// - `one_in( chance )` first - the whole entry may not fire;
/// - `num = roll_remainder( density * SPAWN_DENSITY * rng_float( 10.0, 50.0 ) )` - which is why a tenth of a density still fills a room;
/// - then, while the budget lasts, a square in the rectangle and a result from the group, placed `pack` times, with the budget decremented by
///   `max( 1, cost * pack )`.
///
/// 388 entries use this form - the military bases, the labs, the malls - and a port without it leaves those maps empty. The test pins the three things a wrong
/// implementation gets wrong: **where** the monsters stand (inside the rectangle), **whether** the zero-density entry placed nothing, and that the group was drawn from at all.
#[test]
fn the_density_spawner_fills_its_rectangle() {
    let mut world = a_world(Vec::new());
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 40,
        }],
    );
    cdda_sim::monster_groups::set_groups(
        &mut world,
        vec![cdda_content::monster_groups::MonsterGroup {
            id: "GROUP_TEST".to_owned(),
            entries: vec![cdda_content::monster_groups::MonsterGroupEntry {
                kind: cdda_content::monster_groups::EntryKind::Monster("mon_zombie".to_owned()),
                weight: 100,
                cost_multiplier: 1,
                pack: (1, 1),
            }],
        }],
    );
    let infested = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned(), "....".to_owned(), "....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        place_monsters: vec![
            cdda_content::mapgen::AreaMonsters {
                group: "GROUP_TEST".to_owned(),
                x: (0, 1),
                y: (0, 1),
                chance: (1, 1),
                density_thousandths: 2000,
            },
            // A second entry at zero density: the rule is `roll_remainder( 0 )`, which is nothing, and a spawner that ignored density would fill this one too.
            cdda_content::mapgen::AreaMonsters {
                group: "GROUP_TEST".to_owned(),
                x: (10, 12),
                y: (10, 12),
                chance: (1, 1),
                density_thousandths: 0,
            },
        ],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), infested)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(103));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    // **Every creature in the world**, not the ones standing on one square: the first version of this test asked for the creatures at the origin, so a spawner that
    // filled the dry corner still passed - the mutation found it. A test that cannot see where the monsters are cannot say the rectangle was respected.
    let standing: Vec<(i32, i32, i32)> = cdda_sim::creature::creatures(&mut world)
        .iter()
        .filter_map(|entity| world.get::<cdda_sim::Position>(*entity))
        .map(|at| (at.x, at.y, at.z))
        .collect();
    assert!(
        !standing.is_empty(),
        "**a density of two fills the two-square rectangle: {standing:?}**"
    );
    for at in &standing {
        assert!(
            (0..=1).contains(&at.0) && (0..=1).contains(&at.1),
            "**every monster stands inside the rectangle, and {at:?} does not**"
        );
    }
    let in_the_dry_corner: Vec<&(i32, i32, i32)> = standing
        .iter()
        .filter(|at| (10..=12).contains(&at.0) && (10..=12).contains(&at.1))
        .collect();
    assert!(
        in_the_dry_corner.is_empty(),
        "**the zero-density entry placed nothing: {in_the_dry_corner:?}**"
    );
}

/// **A palette's parameter is drawn once, and every symbol that names it takes that one value.** Reference: `mapgen_palette::resolve_parameters`
/// (`mapgen.cpp:440-520`), called once per generation: each parameter takes one weighted value from its `default.distribution`, and every `{ "param": name }` in the symbol
/// tables becomes it - which is what makes a palette's variant **coherent** instead of one wall type per square.
///
/// 302 palettes declare parameters and 832 symbols name one, so a port without them draws the fallback everywhere, or nothing at all.
#[test]
fn a_parameter_is_drawn_once_for_the_whole_chunk() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_wall_a".to_owned(), 41u32),
            ("t_wall_b".to_owned(), 42u32),
            ("t_fallback".to_owned(), 43u32),
        ]
        .into_iter()
        .collect(),
    );
    let parameterised = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["###".to_owned(), "###".to_owned()],
        terrain: [(
            '#',
            Piece::Param {
                name: "wall".to_owned(),
                fallback: None,
            },
        )]
        .into_iter()
        .collect(),
        parameters: [(
            "wall".to_owned(),
            cdda_content::mapgen::MapgenParameter {
                values: vec![("t_wall_a".to_owned(), 1), ("t_wall_b".to_owned(), 1)],
            },
        )]
        .into_iter()
        .collect(),
        ..a_plain_mapgen()
    };
    // A second palette names a parameter it does **not** declare, so its fallback is what it must draw.
    let mut with_fallback = parameterised.clone();
    with_fallback.om_terrain = "fallback".to_owned();
    with_fallback.rows = vec!["u".to_owned()];
    with_fallback.parameters.clear();
    with_fallback.terrain = [(
        'u',
        Piece::Param {
            name: "not_declared".to_owned(),
            fallback: Some("t_fallback".to_owned()),
        },
    )]
    .into_iter()
    .collect();
    cdda_sim::mapgen_update::set_mapgens(
        &mut world,
        [
            ("house".to_owned(), parameterised),
            ("fallback".to_owned(), with_fallback),
        ],
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(107));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let drawn: Vec<Option<u32>> = (0..3)
        .flat_map(|x| (0..2).map(move |y| (x, y)))
        .map(|(x, y)| cdda_sim::square::terrain_at(&world, (x, y, 0)))
        .collect();
    assert_eq!(
        drawn.iter().filter(|code| **code == Some(41)).count()
            + drawn.iter().filter(|code| **code == Some(42)).count(),
        6,
        "**every square is a wall, and the wall type is one of the two the parameter declares: {drawn:?}**"
    );
    assert!(
        drawn.iter().all(|code| *code == drawn[0]),
        "**and it is the SAME wall type on every square, because the parameter is drawn once: {drawn:?}**"
    );
    // The undeclared parameter falls back, which is how a palette that names a parameter it does not declare still draws.
    cdda_sim::mapgen_tile::generate(&mut world, "fallback", (5, 5, 0));
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (5, 5, 0)),
        Some(43),
        "**an undeclared parameter draws its fallback**"
    );
}

/// **A mapgen parks a vehicle where it says, facing where it says.** Reference: `jmapgen_vehicle::apply` (`mapgen.cpp:3180-3200`) - `one_in( chance )`, then `map::add_vehicle`
/// with the square and the rotation. 500 mapgen entries use it, and the parking lots, the garages and the car dealerships of the world are exactly these entries: a port
/// without it draws empty lots.
///
/// The test also pins the **group** form, which is what `parkinglotbasic` is: the `vehicle` member names a group 111 times, and the draw then picks a prototype out of it.
#[test]
fn a_mapgen_parks_a_vehicle_where_it_says() {
    let mut world = a_world(Vec::new());
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: vec![cdda_content::vehicles::PartSpec {
                id: "frame".to_owned(),
                name: "frame".to_owned(),
                durability: 400,
                ..cdda_content::vehicles::PartSpec::default()
            }],
            prototypes: vec![
                cdda_content::vehicles::Prototype {
                    id: "car".to_owned(),
                    name: "car".to_owned(),
                    parts: vec![cdda_content::vehicles::Placement {
                        part: "frame".to_owned(),
                        x: 0,
                        y: 0,
                        ..cdda_content::vehicles::Placement::default()
                    }],
                    spawns: Vec::new(),
                },
                cdda_content::vehicles::Prototype {
                    id: "hearse".to_owned(),
                    name: "hearse".to_owned(),
                    parts: vec![cdda_content::vehicles::Placement {
                        part: "frame".to_owned(),
                        x: 0,
                        y: 0,
                        ..cdda_content::vehicles::Placement::default()
                    }],
                    spawns: Vec::new(),
                },
            ],
        },
    );
    cdda_sim::vehicle_groups::set_groups(
        &mut world,
        vec![cdda_content::vehicle_groups::VehicleGroup {
            id: "hearses_only".to_owned(),
            vehicles: vec![("hearse".to_owned(), 100)],
        }],
    );
    let lot = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        place_vehicles: vec![
            cdda_content::mapgen::AreaVehicles {
                vehicle: "car".to_owned(),
                x: (1, 1),
                y: (2, 2),
                chance: (1, 1),
                rotation: (90, 90),
            },
            cdda_content::mapgen::AreaVehicles {
                vehicle: "hearses_only".to_owned(),
                x: (3, 3),
                y: (3, 3),
                chance: (1, 1),
                rotation: (0, 0),
            },
            // A one-in-fifty entry on a square of its own: with the seed below the roll does not land, and a placement that ignored the chance would show a vehicle there.
            cdda_content::mapgen::AreaVehicles {
                vehicle: "car".to_owned(),
                x: (7, 7),
                y: (7, 7),
                chance: (50, 50),
                rotation: (0, 0),
            },
        ],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), lot)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(109));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let parked: Vec<((i32, i32, i32), f64)> = {
        let mut query = world.query::<(&cdda_sim::Position, &cdda_sim::vehicles::Vehicle)>();
        query
            .iter(&world)
            .map(|(at, vehicle)| ((at.x, at.y, at.z), vehicle.rotation))
            .collect()
    };
    assert!(
        parked.contains(&((1, 2, 0), 90.0)),
        "**the named prototype stands on its square, facing the heading the entry names: {parked:?}**"
    );
    assert!(
        parked.iter().any(|(at, _)| *at == (3, 3, 0)),
        "**the group form drew a prototype out of the group and parked it: {parked:?}**"
    );
    assert!(
        !parked.iter().any(|(at, _)| *at == (7, 7, 0)),
        "**the one-in-fifty entry did not fire, so that square is empty: {parked:?}**"
    );
}

/// **A `mapping` bundle's item list lands on the symbol, every placement of it.** Reference: `mapgen_palette::load`'s per-symbol reader (`mapgen.cpp:5360-5400`), which accepts
/// `item` and `items` in one bundle - and a bundle's `items` is a **list**, because a bed has a pillow and a blanket on it.
///
/// The port held one placement per symbol, so this member had nowhere to go and was named a gap. The round that made the table a list is the round that closes the gap, and this
/// test is the reason: the loader reads both placements, and the writer places both on the square.
#[test]
fn a_mapping_bundle_places_every_item_it_lists() {
    let mut world = a_world(Vec::new());
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "rock",
                volume_ml: 250,
                weight_g: 657,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
            cdda_sim::item_registry::Kind {
                id: "nail",
                volume_ml: 0,
                weight_g: 4,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
        ],
    );
    let text = r#"{"type": "mapgen", "method": "json", "om_terrain": "house",
        "object": {"rows": ["B"], "mapping": {"B": {"terrain": "t_floor",
            "items": [ {"item": "rock"}, {"item": "nail"} ]}}}}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let loaded = cdda_content::mapgen::load_objects(&value);
    let bed = loaded.first().expect("one mapgen");
    assert_eq!(
        bed.items.get(&'B').map(Vec::len),
        Some(2),
        "**the loader read both placements out of the bundle**"
    );
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [("t_floor".to_owned(), 11u32)].into_iter().collect(),
    );
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), bed.clone())]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(113));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let on_the_bed = cdda_sim::items_at(&mut world, (0, 0)).len();
    assert_eq!(
        on_the_bed, 2,
        "**both placements landed on the square, not just the first**"
    );
}

/// **A mapgen applies a transform over a rectangle, square by square.** Reference: `jmapgen_ter_furn_transform::apply` (`mapgen.cpp:3773-3784`) - the entry's two ranges are a
/// **rectangle** walked in both directions, each square getting `ter_furn_transform::transform` once. No repetition, no chance: a transform is a fact about the map, not a spawn.
///
/// 45 mapgen entries use it, and it is how the data says "there is a window here, and it has been broken out" without spelling the broken shape in its rows.
#[test]
fn a_mapgen_transforms_the_rectangle_it_names() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_window".to_owned(), 61u32),
            ("t_window_broken".to_owned(), 62u32),
        ]
        .into_iter()
        .collect(),
    );
    // **Two registries, on purpose**: the mapgen reads ids through its own code table, and the transform rule resolves a square's code back to a name through the terrain
    // types table. A test that installed only the first drew the windows and transformed nothing.
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_window".to_owned(), 61u32),
            ("t_window_broken".to_owned(), 62u32),
        ],
    );
    cdda_sim::terrain_transform::set_transforms(
        &mut world,
        [(
            "break_windows".to_owned(),
            [("t_window".to_owned(), "t_window_broken".to_owned())]
                .into_iter()
                .collect(),
        )],
    );
    let shop = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["www".to_owned(), "www".to_owned()],
        terrain: [('w', Piece::Id("t_window".to_owned()))]
            .into_iter()
            .collect(),
        place_ter_furn_transforms: vec![cdda_content::mapgen::AreaTransform {
            transform: "break_windows".to_owned(),
            x: (1, 2),
            y: (0, 1),
        }],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), shop)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(127));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let row: Vec<Option<u32>> = (0..3)
        .map(|x| cdda_sim::square::terrain_at(&world, (x, 0, 0)))
        .collect();
    assert_eq!(
        row,
        vec![Some(61), Some(62), Some(62)],
        "**the rectangle's columns are broken and the others are not: {row:?}**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 1, 0)),
        Some(62),
        "**and every row inside the rectangle, not only the first**"
    );
}

/// **A mapgen writes graffiti, from its own text or from a snippet category, and a `remove_all` scrubs it.** Reference: `jmapgen_graffiti::apply`
/// (`mapgen.cpp:2750-2776`), which writes either the entry's `text` or a line drawn out of its `snippet` category, expands the placeholders, and calls `map::set_graffiti`;
/// and `jmapgen_remove_all::apply` (`:4177-4205`), which ends with `delete_graffiti` on every square of its rectangle.
///
/// The test pins all three: the literal text lands, the snippet form draws a line **out of the category** rather than writing the category's name, and a `remove_all` over the
/// same square takes it away again - which is the only way the delete rule can be seen at all.
#[test]
fn a_mapgen_writes_graffiti_and_remove_all_scrubs_it() {
    let mut world = a_world(Vec::new());
    cdda_sim::snippets::set_snippets(
        &mut world,
        [(
            "billboard_ad_snippets".to_owned(),
            cdda_sim::snippets::SnippetLine {
                id: "ad_one".to_owned(),
                text: "EAT AT JOE'S".to_owned(),
                weight: 100,
            },
        )],
    );
    let painted = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        place_graffiti: vec![
            cdda_content::mapgen::AreaGraffiti {
                text: "The end is <city_name>".to_owned(),
                snippet: String::new(),
                x: (0, 0),
                y: (0, 0),
            },
            cdda_content::mapgen::AreaGraffiti {
                text: String::new(),
                snippet: "billboard_ad_snippets".to_owned(),
                x: (1, 1),
                y: (0, 0),
            },
        ],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), painted.clone())]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(131));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::graffiti_at(&world, (0, 0, 0)).as_deref(),
        Some("The end is illegible city name"),
        "**the text lands, with the city tag replaced by the reference's own fallback**"
    );
    assert_eq!(
        cdda_sim::square::graffiti_at(&world, (1, 0, 0)).as_deref(),
        Some("EAT AT JOE'S"),
        "**the snippet form draws a LINE out of the category, not the category's name**"
    );
    // **The scrub is a phase, not an afterthought**: `remove_all` is the mapgen's FIRST phase (`mapgen.h:88-97`), so it can only take away graffiti that was already on the
    // square. The test therefore writes on a chunk that already carries some - the same mapgen generated a second time, without its graffiti entry.
    let scrub_only = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        remove_all: vec![cdda_content::mapgen::RemoveArea {
            x: (0, 0),
            y: (0, 0),
        }],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), scrub_only)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(131));
    assert_eq!(
        cdda_sim::square::graffiti_at(&world, (0, 0, 0)).as_deref(),
        Some("The end is illegible city name"),
        "**the graffiti is there before the scrub runs**"
    );
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::square::graffiti_at(&world, (0, 0, 0)),
        None,
        "**and a remove_all over that square takes it away**"
    );
}

/// **A `bash` set point hits the square once, hard.** Reference: `jmapgen_setmap::apply`'s `JMAPGEN_SETMAP_BASH` arm (`mapgen.cpp:5696-5699`): `m.bash( target_pos, 9999, true )`
/// - one hit at strength 9999, silent. It is how the data draws a room that has been broken open without naming the broken shape in its rows.
///
/// The test gives the square a bashable wall and a set point aimed at it, and asserts the wall became what the wall's own bash block says it becomes - which is the bash
/// module's rule, reached through the mapgen.
#[test]
fn a_bash_set_point_hits_the_square() {
    let mut world = a_world(Vec::new());
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [("t_wall".to_owned(), 71u32), ("t_floor".to_owned(), 72u32)]
            .into_iter()
            .collect(),
    );
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_wall".to_owned(), 71u32),
            ("t_floor".to_owned(), 72u32),
            ("t_wall_rubble".to_owned(), 73u32),
        ],
    );
    cdda_sim::bash::set_data(
        &mut world,
        [cdda_content::mapdata::MapData {
            kind: cdda_content::mapdata::MapDataKind::Terrain,
            id: "t_wall".to_owned(),
            name: "t_wall".to_owned(),
            symbol: [35; 4],
            color: [cdda_core_types::colors::NcColor::default(); 4],
            move_cost: 0,
            light: 0,
            coverage: 0,
            transparent: false,
            examine: None,
            heat_radiation: 0,
            flags: std::collections::BTreeSet::new(),
            bash: Some(cdda_content::mapdata::BashInfo {
                str_min: 0,
                str_max: 1,
                str_min_blocked: 0,
                str_max_blocked: 1,
                str_min_supported: 0,
                str_max_supported: 1,
                destroy_only: false,
                bash_below: false,
                sound_vol: -1,
                sound_fail_vol: -1,
                set: "t_wall_rubble".to_owned(),
                set_from_above: None,
                drop_group: None,
            }),
            roof: None,
        }],
    );
    let broken = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["w".to_owned()],
        terrain: [('w', Piece::Id("t_wall".to_owned()))]
            .into_iter()
            .collect(),
        set_points: vec![cdda_content::mapgen::SetPoint {
            kind: cdda_content::mapgen::SetKind::Bash,
            shape: cdda_content::mapgen::SetShape::Point,
            id: String::new(),
            x: (0, 0),
            y: (0, 0),
            x2: (0, 0),
            y2: (0, 0),
            repeat: (1, 1),
            amount: (0, 0),
        }],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), broken)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(137));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let after = cdda_sim::square::terrain_at(&world, (0, 0, 0));
    assert_eq!(
        after.and_then(|code| cdda_sim::terrain_types::name_of(&world, code)),
        Some("t_wall_rubble".to_owned()),
        "**the bash broke the wall into what its own bash block names**"
    );
}

/// **A radiation set point sets the dose on every square of its shape.** Reference: `jmapgen_setmap::apply`'s `JMAPGEN_SETMAP_RADIATION` arm (`mapgen.cpp:5672-5675`):
/// `m.set_radiation( target_pos, val.get() )`, where the value is the entry's **`amount`** rolled per square. The data uses it ten times, all in the microlab reactor - a room
/// at 1000, a corridor at 100, a hot cell at 10000.
///
/// The test names a square shape because that is the form the data writes (`{ "square": "radiation", "amount": 1000, ... }`), and reads a square inside it and one outside.
#[test]
fn a_radiation_set_point_sets_the_dose() {
    let mut world = a_world(Vec::new());
    let reactor = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned(), "....".to_owned(), "....".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        set_points: vec![cdda_content::mapgen::SetPoint {
            kind: cdda_content::mapgen::SetKind::Radiation,
            shape: cdda_content::mapgen::SetShape::Square,
            id: String::new(),
            x: (1, 1),
            y: (1, 1),
            x2: (2, 2),
            y2: (2, 2),
            repeat: (1, 1),
            amount: (250, 250),
        }],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), reactor)]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(139));
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    assert_eq!(
        cdda_sim::radiation::dose_at(&world, (1, 1, 0)),
        250,
        "**the first square of the shape carries the dose**"
    );
    assert_eq!(
        cdda_sim::radiation::dose_at(&world, (2, 2, 0)),
        250,
        "**and the far one too, because a square shape fills**"
    );
    assert_eq!(
        cdda_sim::radiation::dose_at(&world, (0, 0, 0)),
        0,
        "**while a square outside the shape is clean**"
    );
}

/// **A mapgen lays out a corpse drawn from a group, dated by the entry's age.** Reference: `jmapgen_corpse::apply` (`mapgen.cpp:2965-2974`) - a uniform pick over the group's
/// flattened monsters, a corpse item of that kind dated `age` days back and never before the cataclysm, dropped on the square.
///
/// The test gives the world one monster, one group naming it, the corpse item kind and the corpse store, then asks the mapgen for a body two days old and reads back the three
/// things the corpse knows: what it was, when it died, and whether it can get up.
#[test]
fn a_mapgen_lays_out_a_corpse_it_names() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(151));
    let corpse_item = {
        let kind = cdda_sim::item_registry::Kind {
            id: "corpse",
            volume_ml: 250,
            weight_g: 1_000,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        };
        cdda_sim::item_registry::set_item_types(&mut world, &[kind]);
        cdda_sim::item_registry::id_of(&mut world, "corpse").expect("the corpse kind")
    };
    let zombie = cdda_sim::MonsterKindId(1);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 80,
        }],
    );
    cdda_sim::monster_registry::set_flags_and_species(
        &mut world,
        &[(
            zombie,
            ["REVIVES".to_owned()].into_iter().collect(),
            std::collections::BTreeSet::new(),
        )],
    );
    cdda_sim::creature::set_corpse_kind(
        &mut world,
        corpse_item,
        std::collections::BTreeMap::from([(zombie, 62_500)]),
    );
    cdda_sim::monster_groups::set_groups(
        &mut world,
        vec![cdda_content::monster_groups::MonsterGroup {
            id: "GROUP_ZOMBIE".to_owned(),
            entries: vec![cdda_content::monster_groups::MonsterGroupEntry {
                kind: cdda_content::monster_groups::EntryKind::Monster("mon_zombie".to_owned()),
                weight: 1,
                cost_multiplier: 1,
                pack: (1, 1),
            }],
        }],
    );
    cdda_sim::calendar::set_turn(&mut world, 3 * cdda_sim::calendar::TURNS_PER_DAY);
    let graveyard = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec![".".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        place_corpses: vec![cdda_content::mapgen::AreaCorpse {
            group: "GROUP_ZOMBIE".to_owned(),
            age_days: 2,
            x: (0, 0),
            y: (0, 0),
        }],
        ..a_plain_mapgen()
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house".to_owned(), graveyard)]);
    cdda_sim::mapgen_tile::generate(&mut world, "house", (0, 0, 0));
    let corpse = cdda_sim::items_at(&mut world, (0, 0))
        .into_iter()
        .find(|item| world.get::<cdda_sim::CorpseOf>(*item).is_some())
        .expect("**a body lies on the square**");
    let body = world.get::<cdda_sim::CorpseOf>(corpse).expect("the corpse");
    assert_eq!(body.kind, zombie, "**it is the monster the group named**");
    assert_eq!(
        body.died_on,
        cdda_sim::calendar::TURNS_PER_DAY,
        "**dated two days back from a turn on the third day**"
    );
    assert!(body.revives, "**and it can still get up**");
}
