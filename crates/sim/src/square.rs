//! Squares and what stands on them, as entities.
//!
//! The reference keeps a square's terrain, furniture and trap in per-square arrays inside a submap
//! (`submap` in `src/mapdata.h`). This module is where that structure stops: materialising a level
//! makes one entity per square and one entity per thing standing on it, each holding its own code.
//!
//! A square entity carries a position and nothing else. The ground on it is a separate entity with
//! the same position, which is what the architecture means by "a square holds no contents" - the
//! ground is not in a slot, it is simply there.

use crate::Position;
use bevy_ecs::prelude::*;

/// A square of the level: an address for per-square values, holding no contents.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct Square;

/// A surface standing on a square.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct Ground;

/// The terrain code of a piece of ground.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct Terrain {
    /// The code the loaded terrain table and every answer read.
    pub code: u32,
}

/// A piece of furniture standing on a square.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct Furniture {
    /// The code the loaded furniture table reads.
    pub code: u32,
}

/// What a materialised level made, so a caller can check it rather than count by hand.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Materialised {
    /// How many square entities exist.
    pub squares: u32,
    /// How many pieces of ground were laid.
    pub ground: u32,
    /// How many pieces of furniture were placed.
    pub furniture: u32,
}

/// Makes one entity per square of the level, plus the ground and furniture standing on them.
///
/// The codes come from the loaded level and nowhere else: this is the one place a per-square array
/// is read, and after it runs the store holds the level.
pub fn materialise(world: &mut World, level: &cdda_core_types::level::Level) -> Materialised {
    // A level is whatever submaps were made, so the walk is over the submaps that exist rather than
    // over a square guessed from the origin: a windowed level starts wherever its first submap does.
    let Ok(side) = i32::try_from(cdda_core_types::submap::SUBMAP_SIDE) else {
        return Materialised::default();
    };
    let mut places = level.submap_places();
    places.sort_unstable();
    let mut made = Materialised::default();
    for (submap_x, submap_y) in places {
        for row in 0..side {
            for column in 0..side {
                let xi = submap_x * side + column;
                let yi = submap_y * side + row;
                world.spawn((Position { x: xi, y: yi, z: 0 }, Square));
                made.squares += 1;

                let Some(code) = level.terrain_at(xi, yi) else {
                    continue;
                };
                world.spawn((Position { x: xi, y: yi, z: 0 }, Ground, Terrain { code }));
                made.ground += 1;

                let furniture = level.furniture_at(xi, yi).unwrap_or(0);
                if furniture != 0 {
                    world.spawn((
                        Position { x: xi, y: yi, z: 0 },
                        Furniture { code: furniture },
                    ));
                    made.furniture += 1;
                }
            }
        }
    }
    crate::PositionIndex::rebuild(world);
    made
}
/// Loads a level into the world: entities for every square and everything standing on them.
///
/// The level itself stays in the world as well **for now**, because the light engine still reads it
/// for its caches; a caller that wants the store alone materialises without loading. When the light
/// engine has moved onto the store this stops inserting the level, and nothing else has to change.
pub fn load_level(world: &mut World, level: cdda_core_types::level::Level) -> Materialised {
    let made = materialise(world, &level);
    world.insert_resource(crate::Map(level));
    made
}

/// Every square entity in the store.
#[must_use]
pub fn squares(world: &World) -> Vec<Entity> {
    let mut query = world.try_query::<(Entity, &Square)>();
    let Some(query) = query.as_mut() else {
        return Vec::new();
    };
    query.iter(world).map(|(entity, _)| entity).collect()
}

/// The entities standing at a square, in the order the index holds them.
fn entities_at(world: &World, square: (i32, i32, i32)) -> Vec<Entity> {
    match crate::singletons::index(world) {
        Some(index) => index.at_level(square.0, square.1, square.2).to_vec(),
        None => Vec::new(),
    }
}

/// The terrain code of a square, if any ground stands on it.
#[must_use]
pub fn terrain_at(world: &World, square: (i32, i32, i32)) -> Option<u32> {
    entities_at(world, square)
        .into_iter()
        .find_map(|entity| world.get::<Terrain>(entity).map(|terrain| terrain.code))
}

/// The furniture code of a square, if any furniture stands on it.
#[must_use]
pub fn furniture_at(world: &World, square: (i32, i32, i32)) -> Option<u32> {
    entities_at(world, square)
        .into_iter()
        .find_map(|entity| world.get::<Furniture>(entity).map(|piece| piece.code))
}

/// The square entity at a position, if the level covers it.
#[must_use]
pub fn square_at(world: &World, square: (i32, i32, i32)) -> Option<Entity> {
    entities_at(world, square)
        .into_iter()
        .find(|entity| world.get::<Square>(*entity).is_some())
}
