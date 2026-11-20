//! Applying a mapgen update to the world.
//!
//! Reference: `mapgen::update` (`mapgen.cpp`): an update mapgen is drawn like any other and its terrain written over the squares it covers, which
//! is how a lab's corridors are opened or a wall is torn out by a script. The port keeps the mapgens as data in one entity and writes their terrain
//! into the squares the store already holds.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

/// Every mapgen the world can apply, keyed by the id the data gives it.
#[derive(Debug, Clone, Default, PartialEq, Component)]
/// **Every** mapgen filed under a name, because an overmap terrain may have several and the game picks among them by weight (`mapgen.cpp:1078-1088`). An update mapgen
/// is filed once, so the list a caller reading an update wants is its first entry - which is why this is a list and not a map of maps.
pub struct Mapgens(pub BTreeMap<String, Vec<cdda_content::mapgen::MapgenObject>>);

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
    let mut filed: BTreeMap<String, Vec<cdda_content::mapgen::MapgenObject>> = BTreeMap::new();
    for (name, mapgen) in mapgens {
        filed.entry(name).or_default().push(mapgen);
    }
    world.entity_mut(holder).insert(Mapgens(filed));
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
        // An update mapgen is filed once; a terrain's several mapgens are not this path's business, so the first is the one an update writes.
        .and_then(|mapgens| {
            mapgens
                .0
                .get(update)
                .and_then(|filed| filed.first())
                .cloned()
        })?;
    Some(write(world, &mapgen, origin))
}

/// Writes one mapgen's terrain at an origin, answering how many squares it filled.
///
/// This is the single writer of a drawn mapgen: [`apply`] writes an update's, and [`crate::mapgen_tile::generate`] writes whichever one the weight picked for a tile. Two
/// writers would eventually disagree about what a symbol means, which is exactly the second source of truth the architecture forbids.
pub fn write(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
) -> usize {
    let codes = crate::singletons::holder::<TheTerrainCodes>(world)
        .and_then(|holder| world.get::<TerrainCodes>(holder))
        .map(|codes| codes.0.clone())
        .unwrap_or_default();
    // **The fill comes first**, the whole chunk at the mapgen's own terrain (`mapgen.cpp:5100-5110`), so a map that draws six squares still fills its twenty-four. The count is
    // of squares **touched**, so a row drawn over a filled square is not counted twice: a chunk is a chunk however many layers land on it.
    let side = crate::overmap::SQUARES_PER_OMT;
    let filled = mapgen
        .fill_ter
        .as_deref()
        .and_then(|fill| codes.get(fill).copied());
    if let Some(code) = filled {
        for column in 0..side {
            for row in 0..side {
                crate::square::set_terrain(
                    world,
                    (origin.0 + column, origin.1 + row, origin.2),
                    code,
                );
            }
        }
    }
    let mut written = if filled.is_some() {
        usize::try_from(side * side).unwrap_or(0)
    } else {
        0
    };
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
            if filled.is_none() {
                written += 1;
            }
        }
    }
    written
}

/// Places the fields a mapgen scatters, answering how many patches landed.
///
/// Reference: `mapgen::place_fields` (`mapgen.cpp`): `repeat` patches on squares chosen inside the rectangle, each at a strength rolled between
/// the two ends - and a patch on a square that already holds the field makes it stronger rather than replacing it.
pub fn place_fields(world: &mut World, update: &str, origin: (i32, i32, i32)) -> Option<usize> {
    let mapgen = crate::singletons::holder::<TheMapgens>(world)
        .and_then(|holder| world.get::<Mapgens>(holder))
        // An update mapgen is filed once; a terrain's several mapgens are not this path's business, so the first is the one an update writes.
        .and_then(|mapgens| {
            mapgens
                .0
                .get(update)
                .and_then(|filed| filed.first())
                .cloned()
        })?;
    if mapgen.fields.is_empty() {
        return Some(0);
    }
    let mut placed = 0;
    for placement in &mapgen.fields {
        let Some(field) = crate::field_registry::id_of(world, &placement.field) else {
            continue;
        };
        for _ in 0..placement.repeat.max(0) {
            let mut dice = crate::singletons::take_rolls(world);
            let drawn = dice.as_mut().map(|dice| {
                (
                    dice.rng(placement.x.0, placement.x.1),
                    dice.rng(placement.y.0, placement.y.1),
                    dice.rng(placement.intensity.0, placement.intensity.1),
                )
            });
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            let Some((x, y, strength)) = drawn else {
                return Some(placed);
            };
            crate::add_field_strength(
                world,
                (origin.0 + x, origin.1 + y, origin.2),
                field,
                strength,
            );
            placed += 1;
        }
    }
    Some(placed)
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
