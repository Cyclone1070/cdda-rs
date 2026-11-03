//! Terrain changes the data describes instead of the code.
//!
//! Reference: `ter_furn_transform` (`magic_ter_fur_transform.cpp:63-130`) - a named table whose `terrain` entries say *for a terrain, these results* - and
//! `map::transform_radius` (`map.cpp`), which walks the squares and applies it. The port keeps the table as a singleton beside the other prototype tables and keys it
//! by **name**, because that is what the data writes: [`crate::terrain_types`] turns the name into the code a square carries.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Entity, World};
use std::collections::BTreeMap;

/// The terrain transforms the game defines, by id and then by the terrain they change.
#[derive(Component, Debug, Clone, Default)]
pub struct TerrainTransforms(pub BTreeMap<String, BTreeMap<String, String>>);

/// Marks the entity holding [`TerrainTransforms`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheTerrainTransforms;

/// Installs the transform table, replacing whatever was there.
pub fn set_transforms(
    world: &mut World,
    transforms: impl IntoIterator<Item = (String, BTreeMap<String, String>)>,
) {
    let table = TerrainTransforms(transforms.into_iter().collect());
    let element = crate::singletons::singleton::<TerrainTransforms, TheTerrainTransforms>(world);
    world.entity_mut(element).insert(table);
}

/// What one transform does to one terrain's name, or nothing when it does not touch it.
#[must_use]
/// Applies a transform to every square within `radius` of `centre`, answering how many changed.
///
/// Reference: `map::transform_radius` (`map.cpp`) walks `points_in_radius( pos, radius )` and applies the transform to each square. The port walks the same square
/// block and writes the new terrain through [`crate::square::set_terrain`], so the index stays the one truth about where a terrain is.
pub fn apply(world: &mut World, transform: &str, centre: (i32, i32, i32), radius: i32) -> usize {
    let Some(table) = result_of_name(world, transform) else {
        return 0;
    };
    let mut changed = 0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let at = (centre.0 + dx, centre.1 + dy, centre.2);
            let Some(code) = crate::square::terrain_at(world, at) else {
                continue;
            };
            let Some(name) = crate::terrain_types::name_of(world, code) else {
                continue;
            };
            let Some(wanted) = table.get(&name) else {
                continue;
            };
            let Some(new_code) = crate::terrain_types::code_of(world, wanted) else {
                continue;
            };
            crate::square::set_terrain(world, at, new_code);
            changed += 1;
        }
    }
    changed
}

/// The terrain entries of one transform, cloned so the squares can be written while it is read.
fn result_of_name(world: &World, transform: &str) -> Option<BTreeMap<String, String>> {
    let element = crate::singletons::holder::<TheTerrainTransforms>(world)?;
    let table = world.get::<TerrainTransforms>(element)?;
    table.0.get(transform).cloned()
}

/// The entity holding the transform table, when the world has one.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheTerrainTransforms>(world)
}
