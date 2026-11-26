//! **A placed person is made from their template.**
//!
//! Reference: `npc::load_npc_template` (`npc.cpp:520-568`) copies the template's class, name, attitude, mission and first chat topic onto the person `map::place_npc` has just
//! made. The port's `place_npcs` used to name the person after the **template id** and class them by nothing; this is the join.

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

/// A mapgen with every field empty, so the test states only the ones it means.
fn empty_mapgen() -> MapgenObject {
    MapgenObject {
        mapgensize: (24, 24),
        place_zones: Vec::new(),
        place_monsters: Vec::new(),
        place_vehicles: Vec::new(),
        place_ter_furn_transforms: Vec::new(),
        place_graffiti: Vec::new(),
        place_corpses: Vec::new(),
        faction_owner: Vec::new(),
        place_npcs: Vec::new(),
        om_terrain: String::new(),
        rows: Vec::new(),
        terrain: BTreeMap::new(),
        furniture: BTreeMap::new(),
        parameters: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 100,
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

/// A mapgen that places one person on the square its rows point at, in a world with a little level.
fn a_world(template: &str) -> World {
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
        [("t_house_one".to_owned(), 11u32)].into_iter().collect(),
    );
    let mapgen = MapgenObject {
        mapgensize: (24, 24),
        om_terrain: "house_npc".to_owned(),
        rows: vec![".".to_owned()],
        terrain: [('.', Piece::Id("t_house_one".to_owned()))]
            .into_iter()
            .collect(),
        place_npcs: vec![cdda_content::mapgen::AreaNpc {
            template: template.to_owned(),
            target: false,
            add_trait: Vec::new(),
            unique_id: String::new(),
            x: (0, 0),
            y: (0, 0),
        }],
        ..empty_mapgen()
    };
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let templates = cdda_content::npc_templates::load_paths(&dirs).expect("the templates load");
    cdda_sim::npc_templates::set_templates(&mut world, templates);
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("house_npc".to_owned(), mapgen)]);
    world
}

/// The seer, made from her template: her **name**, her **class**, her attitude and mission - which are enum numbers in the data - and her first chat topic.
#[test]
fn a_placed_person_is_made_from_their_template() {
    let mut world = a_world("SEER_Brigitte_LaCroix");
    assert!(
        cdda_sim::mapgen_update::apply(&mut world, "house_npc", (0, 0, 0)).is_some(),
        "**the mapgen draws**"
    );
    let mut query = world.query::<(Entity, &cdda_sim::npcs::Npc)>();
    let people = query
        .iter(&world)
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    assert_eq!(people.len(), 1, "**one person is placed**");
    let seer = people[0];
    assert_eq!(
        world
            .get::<cdda_sim::creature::Creature>(seer)
            .map(|creature| creature.name.clone())
            .as_deref(),
        Some("Brigitte LaCroix"),
        "**named by the template, not by its id**"
    );
    assert_eq!(
        cdda_sim::npcs::class_of(&world, seer).as_deref(),
        Some("NC_BONE_SEER"),
        "**classed by the template's class**"
    );
    assert_eq!(
        cdda_sim::npcs::attitude_of(&world, seer).as_deref(),
        Some("legacy_6"),
        "**with the attitude the template's number means**"
    );
    assert_eq!(
        cdda_sim::npcs::mission_of(&world, seer).as_deref(),
        Some("shopkeep"),
        "**and the mission its number means**"
    );
    assert_eq!(
        cdda_sim::npcs::first_topic_of(&world, seer).as_deref(),
        Some("TALK_BONE_SEER"),
        "**and her first topic**"
    );
}
