//! Whether anything a creature is wearing carries a flag.
//!
//! Reference: `Character::worn_with_flag` (`character.cpp`), which walks the worn list and asks each item's type. The port has no worn list on the
//! wearer: an item is an entity, and `WornBy` on the item says who wears it - so the walk is over items, and the answer costs one pass.

use bevy_ecs::prelude::*;

/// Whether anything a creature wears carries a flag.
///
/// A creature wearing nothing answers `false`, which is an answer rather than a refusal: that is what the reference says for a character in rags.
#[must_use]
pub fn worn_with_flag(world: &World, creature: Entity, flag: &str) -> bool {
    let mut query = world.try_query::<(&crate::creature::WornBy, &crate::GroundItem)>();
    let Some(mut query) = query.take() else {
        return false;
    };
    query.iter(world).any(|(worn, item)| {
        worn.0 == creature && crate::item_registry::kind_has_flag(world, item.kind, flag)
    })
}

/// Every flag carried by what a creature wears, which a test or a dump reads.
#[must_use]
pub fn worn_flags_of(world: &World, creature: Entity) -> Vec<String> {
    let mut flags: Vec<String> = Vec::new();
    let mut query = world.try_query::<(&crate::creature::WornBy, &crate::GroundItem)>();
    let Some(mut query) = query.take() else {
        return flags;
    };
    for (worn, item) in query.iter(world) {
        if worn.0 != creature {
            continue;
        }
        let Some(holder) = crate::singletons::holder::<crate::item_registry::TheItemFlags>(world)
        else {
            continue;
        };
        let Some(carried) = world.get::<crate::item_registry::ItemFlags>(holder) else {
            continue;
        };
        if let Some(names) = carried.0.get(&item.kind) {
            flags.extend(names.iter().cloned());
        }
    }
    flags.sort_unstable();
    flags.dedup();
    flags
}
