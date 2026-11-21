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

/// How much bashing a square has taken.
///
/// Reference: `map::damage_level( const tripoint &p )` (`src/map.cpp`) reads `m->get_damage()`, a per-square number the submap keeps beside the terrain. It is **state
/// between hits**, not a property of one: a failed hit that exceeded `str_min` leaves damage behind, and the next hit's strength is raised by it. The port keeps it on the
/// square entity, where the square's own facts live.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct BashDamage(pub i32);

/// How much bashing a square has taken, or zero for a square that has taken none.
#[must_use]
pub fn bash_damage_at(world: &World, square: (i32, i32, i32)) -> i32 {
    entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<BashDamage>(*entity).map(|held| held.0))
        .unwrap_or(0)
}

/// Records how much bashing a square has taken.
pub fn set_bash_damage(world: &mut World, square: (i32, i32, i32), damage: i32) {
    let standing = entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Square>(*entity).is_some());
    if let Some(entity) = standing {
        world.entity_mut(entity).insert(BashDamage(damage));
        return;
    }
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            Square,
            BashDamage(damage),
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
}

/// A trap set on a square.
///
/// Reference: `map::tr_at` and `map::trap_set` (`src/map.cpp`), which keep a `trap_id` per square in the submap's own array. The port keeps it the way it keeps furniture -
/// an entity standing on the square, naming the trap the data defines - so "is there a trap here" is a component rather than a code in an array beside the truth.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct Trap {
    /// The trap's id, exactly as the data spells it.
    pub kind: String,
}

/// The trap set on a square, if any.
#[must_use]
pub fn trap_at(world: &World, square: (i32, i32, i32)) -> Option<String> {
    entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<Trap>(*entity).map(|trap| trap.kind.clone()))
}

/// Sets a trap on a square, **only where one is not already set**.
///
/// Reference: `here.trap_set( target, tr_id )` guarded by `if( here.tr_at( target ) == tr_null )` (`magic_spell_effect.cpp:1601-1604`) - and the same guard is
/// `map::trap_set`'s own, which refuses to overwrite. Answering whether the trap was set is what lets a caller tell "placed" from "there was already one".
pub fn set_trap(world: &mut World, square: (i32, i32, i32), kind: &str) -> bool {
    if trap_at(world, square).is_some() {
        return false;
    }
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            Trap {
                kind: kind.to_owned(),
            },
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
    true
}

/// **Takes the trap off a square**, which is what a sprung trap does - the reference's `here.remove_trap( p )` (`trapfunc.cpp:396`), the last thing `trapfunc::eocs` does.
///
/// The entity that carried the trap is destroyed rather than left with an empty field, so the index has nothing to leave behind.
pub fn remove_trap(world: &mut World, square: (i32, i32, i32)) {
    let Some(entity) = entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Trap>(*entity).is_some())
    else {
        return;
    };
    world.entity_mut(entity).despawn();
    crate::singletons::index_mut(world).remove(entity, square);
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
/// After this call the level value is gone: everything it held lives in entities.
pub fn load_level(world: &mut World, level: &cdda_core_types::level::Level) -> Materialised {
    // The level is read here and then dropped: after this call the store is the only place the
    // level's contents exist, which is what "one source of truth" means.
    materialise(world, level)
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
///
/// A slice rather than a copy: this is asked once per ray and once per drawn square, and an
/// allocation per question is the difference between a sweep and a stutter.
fn entities_at(world: &World, square: (i32, i32, i32)) -> &[Entity] {
    match crate::singletons::index(world) {
        Some(index) => index.at_level(square.0, square.1, square.2),
        None => &[],
    }
}

/// The terrain code of a square, if any ground stands on it.
#[must_use]
pub fn terrain_at(world: &World, square: (i32, i32, i32)) -> Option<u32> {
    entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<Terrain>(*entity).map(|terrain| terrain.code))
}

/// What the loader calls a square's terrain, for a line that has to name it.
///
/// Reference: `map::tername` (`map.cpp`), which the traps use to say what burned or froze a creature. The port's squares carry a **code**; the name-to-code table is the content
/// loader's ([`crate::mapgen_update::TerrainCodes`]), so the name is read back out of it rather than copied onto every square. A square whose code the loader has never seen answers
/// nothing, and the caller decides what to say.
#[must_use]
pub fn terrain_id_at(world: &World, square: (i32, i32, i32)) -> Option<String> {
    let code = terrain_at(world, square)?;
    let holder = crate::singletons::holder::<crate::mapgen_update::TheTerrainCodes>(world)?;
    let codes = world.get::<crate::mapgen_update::TerrainCodes>(holder)?;
    codes
        .0
        .iter()
        .find(|(_, registered)| **registered == code)
        .map(|(name, _)| name.clone())
}

/// The furniture code of a square, if any furniture stands on it.
#[must_use]
pub fn furniture_at(world: &World, square: (i32, i32, i32)) -> Option<u32> {
    entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<Furniture>(*entity).map(|piece| piece.code))
}

/// Writes a square's terrain code onto its ground, making ground if the square has none.
///
/// This is the only way terrain changes after loading. There is no second copy to keep in step: the
/// ground entity *is* the square's terrain.
pub fn set_terrain(world: &mut World, square: (i32, i32, i32), code: u32) {
    let ground = entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Terrain>(*entity).is_some());
    if let Some(entity) = ground {
        if let Some(mut terrain) = world.get_mut::<Terrain>(entity) {
            terrain.code = code;
        }
        return;
    }
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            Ground,
            Terrain { code },
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
    // **A square that has no ground still has a square**: the marker is what `squares()` and the revert's memory walk, and ground without one is half a square.
    // Reference: `map::ter_set` writes into a tile that always exists; there is no state in which a tile has terrain and no tile.
    if !entities_at(world, square)
        .iter()
        .any(|entity| world.get::<Square>(*entity).is_some())
    {
        let marker = world
            .spawn((
                Position {
                    x: square.0,
                    y: square.1,
                    z: square.2,
                },
                Square,
            ))
            .id();
        crate::singletons::index_mut(world).insert(marker, square);
    }
}

/// Writes a square's furniture code onto its furniture, making furniture if the square has none.
///
/// A furniture code of zero takes the furniture away, which is what "nothing stands here" means.
pub fn set_furniture(world: &mut World, square: (i32, i32, i32), code: u32) {
    let standing = entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Furniture>(*entity).is_some());
    if let Some(entity) = standing {
        if code == 0 {
            world.entity_mut(entity).despawn();
            crate::PositionIndex::rebuild(world);
        } else if let Some(mut piece) = world.get_mut::<Furniture>(entity) {
            piece.code = code;
        }
        return;
    }
    if code == 0 {
        return;
    }
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            Furniture { code },
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
}

/// The terrain code of every square that has ground on it.
///
/// One sweep of the store, for the systems that work the whole level at once - the sun, the scent
/// table, the field sweep. A point question stays a point question: [`terrain_at`].
#[must_use]
pub fn terrain_by_square(world: &World) -> std::collections::BTreeMap<(i32, i32), u32> {
    let mut found = std::collections::BTreeMap::new();
    let mut query = world.try_query::<(&Position, &Terrain)>();
    let Some(query) = query.as_mut() else {
        return found;
    };
    for (position, terrain) in query.iter(world) {
        found.insert((position.x, position.y), terrain.code);
    }
    found
}

/// The furniture code of every square that has furniture on it.
#[must_use]
pub fn furniture_by_square(world: &World) -> std::collections::BTreeMap<(i32, i32), u32> {
    let mut found = std::collections::BTreeMap::new();
    let mut query = world.try_query::<(&Position, &Furniture)>();
    let Some(query) = query.as_mut() else {
        return found;
    };
    for (position, piece) in query.iter(world) {
        found.insert((position.x, position.y), piece.code);
    }
    found
}

/// How far the squares reach, as the side of the square area that holds them.
///
/// A level is the submaps it has, so its extent is the squares it holds and not a number to guess.
#[must_use]
pub fn extent(world: &World) -> usize {
    let mut query = world.try_query::<&Position>();
    let Some(query) = query.as_mut() else {
        return 0;
    };
    let mut extent = 0;
    for position in query.iter(world) {
        let Ok(x) = usize::try_from(position.x) else {
            continue;
        };
        let Ok(y) = usize::try_from(position.y) else {
            continue;
        };
        extent = extent.max(x + 1).max(y + 1);
    }
    extent
}

/// The square entity at a position, if the level covers it.
#[must_use]
pub fn square_at(world: &World, square: (i32, i32, i32)) -> Option<Entity> {
    entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Square>(*entity).is_some())
}
