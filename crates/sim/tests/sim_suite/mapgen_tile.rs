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
        fill_ter: None,
        predecessor_mapgen: None,
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
        fill_ter: Some("t_house_one".to_owned()),
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec![".F.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('F', "t_house_one".to_owned()),
        ]
        .into_iter()
        .collect(),
        furniture: [('F', "f_desk".to_owned())].into_iter().collect(),
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
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "field".to_owned(),
        rows: vec!["T.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('T', "t_house_one".to_owned()),
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
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["I.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('I', "t_house_one".to_owned()),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: [(
            'I',
            cdda_content::mapgen::ItemPlacement {
                item: Some("rock".to_owned()),
                group: None,
                chance: 100,
                amount: (3, 3),
            },
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
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["G.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('G', "t_house_one".to_owned()),
        ]
        .into_iter()
        .collect(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: [(
            'G',
            cdda_content::mapgen::ItemPlacement {
                item: None,
                group: Some("loot_group".to_owned()),
                chance: 100,
                amount: (2, 2),
            },
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
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', "t_house_one".to_owned())].into_iter().collect(),
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
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        terrain: [('#', "t_house_two".to_owned())].into_iter().collect(),
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
        fill_ter: None,
        predecessor_mapgen: None,
    };
    let roof = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', "t_house_one".to_owned())].into_iter().collect(),
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
        fill_ter: None,
        predecessor_mapgen: Some("house_inside".to_owned()),
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
            }],
        }],
    );
    let horde = MapgenObject {
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', "t_house_one".to_owned())].into_iter().collect(),
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
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned()],
        terrain: [('.', "t_house_one".to_owned())].into_iter().collect(),
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
        weight: 1,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["T.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('T', "t_house_one".to_owned()),
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
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["WG".to_owned()],
        terrain: [
            ('W', "t_house_one".to_owned()),
            ('G', "t_house_one".to_owned()),
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
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["..".to_owned(), "..".to_owned()],
        terrain: [('.', "t_house_one".to_owned())].into_iter().collect(),
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
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["....".to_owned()],
        terrain: [('.', "t_house_one".to_owned())].into_iter().collect(),
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
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["L.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('L', "t_house_one".to_owned()),
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
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
            ('V', "t_house_one".to_owned()),
            ('R', "t_house_one".to_owned()),
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
        signs: BTreeMap::new(),
        gas_pumps: BTreeMap::new(),
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["S.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('S', "t_house_one".to_owned()),
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
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
        om_terrain: "house".to_owned(),
        rows: vec!["G.".to_owned()],
        terrain: [
            ('.', "t_house_one".to_owned()),
            ('G', "t_house_one".to_owned()),
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
        weight: 1,
        fields: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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
