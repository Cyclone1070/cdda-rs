//! The furniture ids the game defines, by name.
//!
//! The twin of [`crate::terrain_types`], and needed for the same reason: a square's furniture is a `u32` code and the data names it - `f_scrap_antenna`,
//! `f_bookcase`. Anything that searches for furniture by name, or reports a code back as a name, reads this one table, so a search and a report cannot disagree.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Entity, World};
use std::collections::BTreeMap;

/// Every furniture the game defines, in both directions.
#[derive(Component, Debug, Clone, Default)]
pub struct FurnitureTypes {
    /// The code each name belongs to.
    pub codes: BTreeMap<String, u32>,
    /// The name each code belongs to.
    pub names: BTreeMap<u32, String>,
    /// The flags each code carries.
    pub flags: BTreeMap<u32, std::collections::BTreeSet<String>>,
}

/// Marks the entity holding [`FurnitureTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheFurnitureTypes;

/// Gives furniture the flags the data defines for it, by name.
pub fn set_flags(world: &mut World, furniture: impl IntoIterator<Item = (String, Vec<String>)>) {
    let Some(holder) = holder(world) else {
        return;
    };
    if let Some(mut table) = world.get_mut::<FurnitureTypes>(holder) {
        for (name, flags) in furniture {
            let Some(code) = table.codes.get(&name).copied() else {
                continue;
            };
            table.flags.insert(code, flags.into_iter().collect());
        }
    }
}

/// Whether a furniture code carries a flag.
#[must_use]
pub fn has_flag(world: &World, code: u32, flag: &str) -> bool {
    holder(world)
        .and_then(|holder| world.get::<FurnitureTypes>(holder))
        .and_then(|table| table.flags.get(&code))
        .is_some_and(|flags| flags.contains(flag))
}

/// Installs the furniture table from name-code pairs, replacing whatever was there.
pub fn set_types(world: &mut World, furniture: impl IntoIterator<Item = (String, u32)>) {
    let mut table = FurnitureTypes::default();
    for (name, code) in furniture {
        table.codes.insert(name.clone(), code);
        table.names.insert(code, name);
    }
    let element = crate::singletons::singleton::<FurnitureTypes, TheFurnitureTypes>(world);
    world.entity_mut(element).insert(table);
}

/// The entity holding the furniture table, when the world has one.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheFurnitureTypes>(world)
}

/// The code a furniture name belongs to, or nothing when the game defines no such furniture.
#[must_use]
pub fn code_of(world: &World, name: &str) -> Option<u32> {
    let element = holder(world)?;
    world
        .get::<FurnitureTypes>(element)
        .and_then(|types| types.codes.get(name).copied())
}

/// The name a furniture code belongs to, or nothing when the code is not in the table.
#[must_use]
pub fn name_of(world: &World, code: u32) -> Option<String> {
    let element = holder(world)?;
    world
        .get::<FurnitureTypes>(element)
        .and_then(|types| types.names.get(&code).cloned())
}
