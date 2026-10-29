//! A route across real squares, with the store answering what each one costs.
//!
//! The search itself is [`cdda_sim::pathfinding::greedy_path`], gated against the reference's own
//! expected paths. What is new here is the *scorer*: the cost of entering a square comes from the world,
//! through `map::combined_movecost`'s rule, and a square described as impassable is refused rather than
//! charged for.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::combined_move_cost;
use cdda_sim::pathfinding::{Direction, manhattan_distance};
use cdda_sim::routing::route;
use cdda_sim::{Position, singletons};

/// Builds a world whose squares are all floor except the ones named, which are walls.
fn a_world(width: i32, height: i32, walls: &[(i32, i32)]) -> World {
    let mut world = World::new();
    let mut prototypes = cdda_sim::Prototypes::default();
    prototypes.terrain.insert_move_cost(1, 2); // floor
    prototypes.terrain.insert_move_cost(2, 0); // wall
    singletons::set_prototypes(&mut world, prototypes);
    for y in 0..height {
        for x in 0..width {
            let code = if walls.contains(&(x, y)) { 2 } else { 1 };
            let entity = world
                .spawn((
                    Position { x, y, z: 0 },
                    cdda_sim::square::Terrain { code },
                ))
                .id();
            singletons::index_mut(&mut world).insert(entity, (x, y, 0));
        }
    }
    world
}

#[test]
fn a_route_goes_round_a_wall_and_costs_both_squares_a_step() {
    // The same shape the reference's own u-bend test uses: a wall at x = 1 on the first two rows.
    let world = a_world(3, 3, &[(1, 0), (1, 1)]);

    let path = route(&world, (0, 0), (2, 0), (3, 3));

    let points: Vec<(i32, i32)> = path.iter().map(|node| (node.x, node.y)).collect();
    assert_eq!(
        points,
        vec![(2, 0), (2, 1), (2, 2), (1, 2), (0, 2), (0, 1), (0, 0)],
        "the wall sends the route down and around, as it does in the reference's own case"
    );
    assert_eq!(path.len(), 7, "seven squares");
}

#[test]
fn an_inpassable_square_is_refused_rather_than_charged_for() {
    // A wall across the whole map: no route exists, and the search must say so rather than walk through.
    let world = a_world(3, 1, &[(1, 0)]);

    let path = route(&world, (0, 0), (2, 0), (3, 1));

    assert!(path.is_empty(), "a wall with no way round ends the search empty");
}

#[test]
fn the_scorer_charges_what_combined_move_cost_says() {
    // Two floors: 2 and 2, straight, is (2 + 2) * 50 / 2 = 100 moves; diagonally 142.
    assert_eq!(combined_move_cost(2, 2, 0, 1), 100);
    assert_eq!(combined_move_cost(2, 2, 0, 2), 142);
    // And the score the search sees is what the store answers, not a constant.
    let world = a_world(2, 1, &[]);
    let scorer = cdda_sim::routing::scorer(&world, (0, 0));
    let straight = scorer((1, 0), Some((0, 0)));
    assert_eq!(straight.node_cost, 100, "the cost of entering the floor");
    assert_eq!(
        straight.estimated_dest_cost,
        manhattan_distance((1, 0), (0, 0)) * 100,
        "and the estimate is one step's worth per square away"
    );
    let _ = Direction::North;
}

/// The reference's own terrain data, in the shape its own loader reads, becomes a walkable world.
///
/// Source: `corpus/mapdata.json` is `--dump-mapdata`, which writes every terrain and furniture prototype
/// with `type` and a string `symbol` - the fields `map_data_common_t::load` reads - rather than the
/// `kind` and code-point `symbol` the other dumps use for the port's own tests. That difference is the
/// whole point of the dump: without it the port's loader cannot be pointed at the reference's data at
/// all, and the two shapes were being bridged by hand-written fixtures.
#[test]
fn the_reference_s_terrain_data_loads_and_keeps_its_move_costs() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/mapdata.json");
    let text = std::fs::read(path).expect("the mapdata dump exists");
    let parsed = cdda_json::parse(&text).expect("the dump is valid JSON");
    let loaded = cdda_content::mapdata::load_objects(&parsed, "corpus/mapdata.json")
        .expect("the reference's own shape loads");
    let seen = loaded.len();
    let terrain: Vec<cdda_content::mapdata::MapData> = loaded
        .into_iter()
        .filter(|prototype| prototype.kind == cdda_content::mapdata::MapDataKind::Terrain)
        .collect();
    let terrains = terrain.len();
    let (table, codes) = cdda_sim::terrain_table::TerrainTable::from_map_data(terrain);

    let floor = codes.get("t_floor").copied().expect("the data has a floor");
    assert_eq!(table.move_cost(floor), Some(2), "a floor costs two to stand on");
    let wall = codes.get("t_wall").copied().expect("the data has a wall");
    assert_eq!(
        table.move_cost(wall),
        Some(0),
        "a wall costs nothing to stand on, which means it cannot be entered at all"
    );
    assert!(
        terrains > 1_000,
        "only {terrains} of {seen} prototypes were terrain, which is too few to be the corpus"
    );
    assert!(
        codes.len() == terrains,
        "every terrain got a code: {} codes for {terrains} terrains",
        codes.len()
    );
}

/// A prototype becomes a code in exactly one place, and its move cost comes with it.
///
/// The fixture is written here rather than read from the corpus dump on purpose: the dump states
/// `"kind"` where the loader reads `"type"` (`mapdata.cpp`'s own field name), so the two cannot be
/// joined without deciding which is right. That decision is recorded in parity instead of being made
/// by a test. What this pins is the part that is settled - the table takes the cost off the prototype
/// and hands back the code the square will use.
#[test]
fn a_prototype_becomes_a_code_and_keeps_its_move_cost() {
    // The symbols are strings, as the loader reads them (`symbol entries must be strings`): the
    // corpus dump writes code points instead, which is the second reason the dump's shape and the
    // loader's shape are not the same question.
    let text = r#"[
        {"type":"terrain","id":"t_floor","name":"floor","symbol":".","color":"c_cyan","move_cost":2,"light":0,"coverage":0,"transparent":true,"flags":[]},
        {"type":"terrain","id":"t_wall","name":"wall","symbol":"X","color":"c_light_gray","move_cost":0,"light":0,"coverage":0,"transparent":false,"flags":[]}
    ]"#;
    let loaded = cdda_content::mapdata::load_objects(
        &cdda_json::parse(text.as_bytes()).expect("the fixture parses"),
        "fixture",
    )
    .expect("the fixture loads");
    let terrain: Vec<cdda_content::mapdata::MapData> = loaded
        .into_iter()
        .filter(|prototype| prototype.kind == cdda_content::mapdata::MapDataKind::Terrain)
        .collect();
    assert_eq!(terrain.len(), 2, "both prototypes are terrain");

    let (table, codes) = cdda_sim::terrain_table::TerrainTable::from_map_data(terrain);

    let floor = codes.get("t_floor").copied().expect("the floor got a code");
    assert_eq!(table.move_cost(floor), Some(2), "and kept its cost");
    let wall = codes.get("t_wall").copied().expect("the wall got a code");
    assert_eq!(table.move_cost(wall), Some(0), "a wall costs nothing to stand on, which means never");
}
