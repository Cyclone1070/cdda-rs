//! Applying a mapgen update to the world.
//!
//! Reference: `mapgen::update` (`mapgen.cpp`): an update mapgen is drawn like any other and its terrain written over the squares it covers, which
//! is how a lab's corridors are opened or a wall is torn out by a script. The port keeps the mapgens as data in one entity and writes their terrain
//! into the squares the store already holds.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

/// Every mapgen the world can apply, keyed by the id the data gives it.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct Mapgens(pub BTreeMap<String, cdda_content::mapgen::MapgenObject>);

/// The marker that says a single entity holds the mapgens.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheMapgens;

/// What number the world calls each terrain, which is how a drawn id reaches a square.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct TerrainCodes(pub BTreeMap<String, u32>);

/// The marker that says a single entity holds the terrain codes.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheTerrainCodes;

/// Installs the mapgens, replacing whatever the world held.
pub fn set_mapgens(
    world: &mut World,
    mapgens: impl IntoIterator<Item = (String, cdda_content::mapgen::MapgenObject)>,
) {
    let holder = crate::singletons::singleton::<Mapgens, TheMapgens>(world);
    world.entity_mut(holder).insert(Mapgens(mapgens.into_iter().collect()));
}

/// Installs the terrain codes, so a drawn terrain id can be written to a square.
pub fn set_terrain_codes(world: &mut World, codes: BTreeMap<String, u32>) {
    let holder = crate::singletons::singleton::<TerrainCodes, TheTerrainCodes>(world);
    world.entity_mut(holder).insert(TerrainCodes(codes));
}

/// Applies an update mapgen at a square, answering how many squares it wrote.
///
/// Reference: `mapgen::update` (`mapgen.cpp`) draws the mapgen and writes the terrain it names, leaving squares it says nothing about alone - which
/// is why an update mapgen may cover only part of a tile.
#[must_use]
pub fn apply(world: &mut World, update: &str, origin: (i32, i32, i32)) -> Option<usize> {
    let mapgen = crate::singletons::holder::<TheMapgens>(world)
        .and_then(|holder| world.get::<Mapgens>(holder))
        .and_then(|mapgens| mapgens.0.get(update).cloned())?;
    let codes = crate::singletons::holder::<TheTerrainCodes>(world)
        .and_then(|holder| world.get::<TerrainCodes>(holder))
        .map(|codes| codes.0.clone())
        .unwrap_or_default();
    let mut written = 0;
    for (row, line) in mapgen.rows.iter().enumerate() {
        for (column, symbol) in line.chars().enumerate() {
            let Some(terrain) = mapgen.terrain.get(&symbol) else {
                continue;
            };
            let Some(code) = codes.get(terrain).copied() else {
                continue;
            };
            let (Some(column), Some(row)) = (offset(column), offset(row)) else {
                continue;
            };
            crate::square::set_terrain(world, (origin.0 + column, origin.1 + row, origin.2), code);
            written += 1;
        }
    }
    Some(written)
}

/// A row or column as a world offset, or nothing when it is too large to be one.
fn offset(index: usize) -> Option<i32> {
    i32::try_from(index).ok()
}

/// How many mapgens the world holds.
#[must_use]
pub fn census(world: &World) -> usize {
    crate::singletons::holder::<TheMapgens>(world)
        .and_then(|holder| world.get::<Mapgens>(holder))
        .map_or(0, |mapgens| mapgens.0.len())
}
