//! The terrain ids the game defines, by name.
//!
//! Reference: `ter_t` maps (`mapdata.cpp`) keyed by `ter_id`, and the data writes terrains by **name** - `t_dirt`, `t_grass` - while the port's squares carry a
//! `u32` code. Something has to know which name is which code, and this is it: one singleton beside the other prototype tables, holding both directions so a
//! search by name and a report by code read the same table.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Entity, World};
use std::collections::BTreeMap;

/// Every terrain the game defines, in both directions.
#[derive(Component, Debug, Clone, Default)]
pub struct TerrainTypes {
    /// The code each name belongs to.
    pub codes: BTreeMap<String, u32>,
    /// The name each code belongs to.
    pub names: BTreeMap<u32, String>,
    /// The flags each code carries, as the data's terrain defines them.
    ///
    /// Reference: `ter_t::has_flag` (`mapdata.h`), read by every rule that asks what a tile is like - whether it is transparent, whether it can be walked on.
    pub flags: BTreeMap<u32, std::collections::BTreeSet<String>>,
}

/// Marks the entity holding [`TerrainTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheTerrainTypes;

/// Gives terrains the flags the data defines for them, by name.
///
/// Reference: the `flags` array of a terrain entry (`mapdata.cpp`): the flags belong to the **type**, not to the square, so they are stored once beside the codes.
pub fn set_flags(world: &mut World, terrains: impl IntoIterator<Item = (String, Vec<String>)>) {
    let Some(holder) = holder(world) else {
        return;
    };
    if let Some(mut table) = world.get_mut::<TerrainTypes>(holder) {
        for (name, flags) in terrains {
            let Some(code) = table.codes.get(&name).copied() else {
                continue;
            };
            table.flags.insert(code, flags.into_iter().collect());
        }
    }
}

/// Whether a terrain code carries a flag.
#[must_use]
pub fn has_flag(world: &World, code: u32, flag: &str) -> bool {
    holder(world)
        .and_then(|holder| world.get::<TerrainTypes>(holder))
        .and_then(|table| table.flags.get(&code))
        .is_some_and(|flags| flags.contains(flag))
}

/// Installs the terrain table from name-code pairs, replacing whatever was there.
pub fn set_types(world: &mut World, terrains: impl IntoIterator<Item = (String, u32)>) {
    let mut table = TerrainTypes::default();
    for (name, code) in terrains {
        table.codes.insert(name.clone(), code);
        table.names.insert(code, name);
    }
    let element = crate::singletons::singleton::<TerrainTypes, TheTerrainTypes>(world);
    world.entity_mut(element).insert(table);
}

/// The entity holding the terrain table, when the world has one.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheTerrainTypes>(world)
}

/// The code a terrain name belongs to, or nothing when the game defines no such terrain.
#[must_use]
pub fn code_of(world: &World, name: &str) -> Option<u32> {
    let element = holder(world)?;
    world
        .get::<TerrainTypes>(element)
        .and_then(|types| types.codes.get(name).copied())
}

/// The name a terrain code belongs to, or nothing when the code is not in the table.
#[must_use]
pub fn name_of(world: &World, code: u32) -> Option<String> {
    let element = holder(world)?;
    world
        .get::<TerrainTypes>(element)
        .and_then(|types| types.names.get(&code).cloned())
}
