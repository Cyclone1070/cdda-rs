//! A square is an ordinary entity, and the ground standing on it is another one.
//!
//! The reference keeps a square's terrain, furniture and trap in per-square arrays inside a submap
//! (`mapdata.h`, `submap`). That is the structure this port does not import: what a square holds is
//! a set of entities at its position, each carrying its own component, and the square itself carries
//! nothing but its position.

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::square::{Furniture, Ground, Square, Terrain, materialise, squares, terrain_at};
use cdda_sim::Position;

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
    assert_eq!(counts.ground, SIDE * SIDE, "and a piece of ground under each");
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
        world.get_resource::<cdda_sim::Map>().is_none(),
        "the test world holds no level resource to fall back on"
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
