//! A square is an ordinary entity, and the ground standing on it is another one.
//!
//! The reference keeps a square's terrain, furniture and trap in per-square arrays inside a submap
//! (`mapdata.h`, `submap`). That is the structure this port does not import: what a square holds is
//! a set of entities at its position, each carrying its own component, and the square itself carries
//! nothing but its position.

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::FireMap as _;
use cdda_sim::Position;
use cdda_sim::square::{Furniture, Ground, Square, Terrain, materialise, squares, terrain_at};

const AIR: u32 = 1;
const WALL: u32 = 2;
const CHAIR: u32 = 9;

/// One submap, which is the level's extent: twelve squares on a side.
const SIDE: u32 = 12;

/// A one-submap level with the marked squares given their code and the rest left as air.
fn level(marks: &[(i32, i32, u32)]) -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, AIR);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
                continue;
            };
            level.set_terrain(x, y, AIR, 0);
        }
    }
    for (x, y, code) in marks {
        level.set_terrain(*x, *y, *code, 0);
    }
    level
}

#[test]
fn every_square_of_the_level_is_an_entity() {
    let mut world = World::new();
    let counts = materialise(&mut world, &level(&[]));

    let expected = usize::try_from(SIDE * SIDE).unwrap_or(0);
    assert_eq!(
        counts.squares,
        SIDE * SIDE,
        "one submap is twelve squares on a side"
    );
    assert_eq!(
        counts.ground,
        SIDE * SIDE,
        "and a piece of ground under each"
    );
    assert_eq!(
        squares(&world).len(),
        expected,
        "which is what the store says as well"
    );
}

#[test]
fn the_ground_carries_the_terrain_code_and_the_square_itself_carries_nothing() {
    let mut world = World::new();
    materialise(&mut world, &level(&[(1, 1, WALL)]));

    assert_eq!(
        terrain_at(&world, (1, 1, 0)),
        Some(WALL),
        "the wall's square answers with the wall"
    );
    assert_eq!(
        terrain_at(&world, (2, 1, 0)),
        Some(AIR),
        "and its neighbour with air"
    );

    // The doctrine, as a query: a square entity is never the thing that holds the terrain.
    let mut both = world.query_filtered::<Entity, (With<Square>, With<Terrain>)>();
    assert_eq!(
        both.iter(&world).count(),
        0,
        "no square entity carries a terrain code"
    );
    let mut ground = world.query_filtered::<Entity, (With<Ground>, With<Terrain>)>();
    assert_eq!(
        ground.iter(&world).count(),
        usize::try_from(SIDE * SIDE).unwrap_or(0),
        "every piece of ground carries one"
    );
}

#[test]
fn furniture_is_an_entity_of_its_own_on_the_square() {
    let mut world = World::new();
    let mut level = level(&[]);
    level.set_furniture(2, 2, CHAIR, 0);
    let counts = materialise(&mut world, &level);

    assert_eq!(counts.furniture, 1, "one chair was made");
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (2, 2, 0)),
        Some(CHAIR),
        "and it answers on its own square"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (2, 3, 0)),
        None,
        "and nowhere else"
    );

    let mut furniture = world.query::<(&Position, &Furniture)>();
    let found: Vec<_> = furniture
        .iter(&world)
        .map(|(position, piece)| (position.x, position.y, piece.code))
        .collect();
    assert_eq!(found, vec![(2, 2, CHAIR)], "the chair is at its square");
}

#[test]
fn a_world_of_entities_alone_answers_what_a_character_can_see() {
    // No `Map` resource at all: the ground entities are the only source of the terrain, and the
    // sight rule has to read them. This is the gate on "one store" - if the answer needed the
    // loaded level as well, the world would have two sources of truth.
    let mut world = World::new();
    let mut level = level(&[]);
    for y in 0..SIDE {
        let Ok(y) = i32::try_from(y) else { continue };
        level.set_terrain(6, y, WALL, 0);
    }
    materialise(&mut world, &level);
    cdda_sim::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: cdda_sim::terrain_table::TerrainTable::from_prototypes([
                (AIR, true),
                (WALL, false),
            ]),
            furniture: cdda_sim::terrain_table::TerrainTable::new(),
        },
    );

    assert!(
        !cdda_sim::square::squares(&world).is_empty(),
        "the world's squares are entities and nothing else stands behind them"
    );
    assert!(
        !cdda_sim::sees_through_fields(&world, (3, 3), (9, 3), 20, false),
        "the wall stands between them"
    );
    assert!(
        cdda_sim::sees_through_fields(&world, (3, 3), (5, 3), 20, false),
        "and the near square is in plain sight"
    );
}

#[test]
fn setting_the_terrain_writes_the_ground_and_not_a_copy() {
    // The world after loading has one source for terrain: the ground entities. A change has to go
    // there, or a system reading the store and a test reading the level would disagree.
    let mut world = World::new();
    materialise(&mut world, &level(&[(1, 1, WALL)]));
    assert_eq!(terrain_at(&world, (1, 1, 0)), Some(WALL));

    cdda_sim::square::set_terrain(&mut world, (1, 1, 0), AIR);

    assert_eq!(
        terrain_at(&world, (1, 1, 0)),
        Some(AIR),
        "the ground entity carries the new code"
    );
    let mut grounds = world.query_filtered::<Entity, (With<Ground>, With<Terrain>)>();
    assert_eq!(
        grounds.iter(&world).count(),
        usize::try_from(SIDE * SIDE).unwrap_or(0),
        "and no second piece of ground was made on that square"
    );
}

#[test]
fn the_fire_facts_of_a_square_are_a_component_on_it() {
    // The turn asks the ground a handful of questions - can it burn, is there a floor under it,
    // does it hold smoke down - and the answers belong on the square, not in a table the caller
    // hands to the rules.
    let mut world = World::new();
    materialise(&mut world, &level(&[]));

    cdda_sim::fire_facts::set_fire_facts(
        &mut world,
        (3, 4, 0),
        cdda_sim::TerrainFacts {
            no_floor: true,
            flammable: true,
            ..cdda_sim::TerrainFacts::default()
        },
    );

    let facts = cdda_sim::fire_facts::fire_facts_at(&world, (3, 4, 0));
    assert!(facts.no_floor, "the hole is a hole");
    assert!(facts.flammable, "and the floor burns");
    assert!(
        !cdda_sim::fire_facts::fire_facts_at(&world, (3, 5, 0)).no_floor,
        "the square beside it has a floor"
    );
    assert_eq!(
        cdda_sim::fire_facts::fire_facts_squares(&world),
        1,
        "only the square that was told about carries facts"
    );
}

#[test]
fn the_turn_s_questions_about_fire_come_from_the_world() {
    // The store is the authority: a world built out of facts and fires, with no map handed to it,
    // still describes itself completely enough for the fire rules to be asked.
    let mut world = World::new();
    materialise(&mut world, &level(&[]));
    cdda_sim::fire_facts::set_fire_facts(
        &mut world,
        (5, 5, 0),
        cdda_sim::TerrainFacts {
            flammable: true,
            ..cdda_sim::TerrainFacts::default()
        },
    );
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(500);
    entry.set_intensity(2);
    world.spawn((
        cdda_sim::Field {
            id: cdda_sim::FIRE,
            entry,
            priority: 0,
        },
        Position { x: 6, y: 5, z: 0 },
    ));

    let facts = cdda_sim::MapFacts::from_world(&world);

    assert!(
        facts.can_burn((5, 5, 0)),
        "the ground's own answer is in the sheet"
    );
    assert!(
        !facts.can_burn((5, 6, 0)),
        "and the square beside it is bare"
    );
    assert_eq!(
        facts.adjacent_fires((5, 5, 0)),
        1,
        "the fire standing beside it is counted"
    );
    assert!(
        facts.neighbour((6, 5, 0)).has_fire,
        "and is found where it is"
    );
}

#[test]
fn a_turn_runs_on_a_world_that_is_only_entities() {
    // No map is handed to the turn: the fire's ground, the fire itself and the square's answers all
    // come out of the store, which is what makes the turn callable by a game loop.
    let mut world = World::new();
    materialise(&mut world, &level(&[]));
    cdda_sim::fire_facts::set_fire_facts(
        &mut world,
        (5, 5, 0),
        cdda_sim::TerrainFacts {
            flammable: true,
            ..cdda_sim::TerrainFacts::default()
        },
    );
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(500);
    entry.set_intensity(2);
    let fire = world
        .spawn((
            cdda_sim::Field {
                id: cdda_sim::FIRE,
                entry,
                priority: 0,
            },
            Position { x: 5, y: 5, z: 0 },
        ))
        .id();

    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(7));
    cdda_sim::turn::set_scent_source(&mut world, (5, 5), 0);
    cdda_sim::turn::run_turn(&mut world);

    let aged = world
        .get::<cdda_sim::Field>(fire)
        .map(|field| field.entry.age());
    assert!(
        aged.is_some_and(|age| age != 500),
        "the fire took its turn: the store supplied its fuel"
    );
}
