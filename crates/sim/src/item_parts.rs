//! What a thing is made of.
//!
//! Reference: `item::components`, a `std::map<itype_id, std::vector<item>>` that every item carries (`item.h`), walked by the rules that take a thing apart - the `FRAGILE_MELEE` breakdown
//! (`melee.cpp:301-330`), disassembly, repair.
//!
//! **The port does not nest containers.** Bevy's entities are the only nesting the architecture allows: a part is an entity of its own, carrying [`PartOf`], and "made of" is that link. The part list
//! is therefore **derived** - a query - so a part that is destroyed is gone from its parent's list with no bookkeeping and no chance of a dangling reference.

use bevy_ecs::prelude::*;

/// Marks an entity as a part of another: the whole it came out of, and which part it is in that whole's list.
///
/// `order` exists because the reference's list is a `std::vector` and its order is the order the parts were installed; a query cannot promise an order, so the order is data.
#[derive(Component, Debug, Clone, Copy)]
pub struct PartOf {
    /// The thing this is a part of.
    pub parent: Entity,
    /// Where in the parent's part list it sits, which is the order the data installed.
    pub order: u32,
}

/// Makes `part` a part of `whole`, at the end of the whole's list.
pub fn add_part(world: &mut World, whole: Entity, part: Entity) {
    let order = u32::try_from(parts_of(world, whole).len()).unwrap_or(u32::MAX);
    world.entity_mut(part).insert(PartOf { parent: whole, order });
}

/// Every part of `whole`, in the order the data installed them.
#[must_use]
pub fn parts_of(world: &World, whole: Entity) -> Vec<Entity> {
    let Some(mut with_ids) = world.try_query::<(Entity, &PartOf)>() else {
        return Vec::new();
    };
    let mut found: Vec<(u32, Entity)> = with_ids
        .iter(world)
        .filter(|(_, link)| link.parent == whole)
        .map(|(entity, link)| (link.order, entity))
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, entity)| entity).collect()
}
