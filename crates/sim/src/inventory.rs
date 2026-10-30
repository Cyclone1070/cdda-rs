//! What a creature is carrying, counted the way a recipe asks for it.
//!
//! The reference keeps a character's items in a `std::list` and asks `inv.has_components( recipe )` against it. The
//! port has no list: a carried item is an entity with a [`crate::creature::Carried`] link, and the square it came from
//! is gone, which is what makes "in hand" a fact about the store rather than a second container beside it.
//!
//! Counting by **name** is the join that matters, because that is how the data asks: a recipe wants two `rag`s, and the
//! store knows kinds by [`crate::ItemTypeId`]. The name of a kind is the item registry's business, so the caller hands
//! in the lookup rather than the module inventing a table.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use crate::creature::Carried;
use crate::GroundItem;

/// How many of each kind of thing a creature is carrying, by the item's own id.
///
/// `name_of` turns a kind into the id the data uses, or [`None`] for a kind it does not know. An unknown kind is
/// counted under no name at all rather than under a made-up one: a recipe must not be satisfied by a thing nobody can
/// name.
#[must_use]
pub fn carried_by_id(
    world: &mut World,
    creature: Entity,
    name_of: impl Fn(crate::ItemTypeId) -> Option<String>,
) -> BTreeMap<String, i32> {
    let mut counted: BTreeMap<String, i32> = BTreeMap::new();
    let mut query = world.query::<(&Carried, &GroundItem)>();
    for (carried, item) in query.iter(world) {
        if carried.0 != creature {
            continue;
        }
        let Some(name) = name_of(item.kind) else {
            continue;
        };
        // Charges are how many of a thing a single stack holds: two rags in one stack are two rags.
        *counted.entry(name).or_insert(0) += item.charges.max(1);
    }
    counted
}

/// How many of one thing a creature is carrying.
#[must_use]
pub fn how_many(world: &mut World, creature: Entity, item: &str, name_of: impl Fn(crate::ItemTypeId) -> Option<String>) -> i32 {
    carried_by_id(world, creature, name_of)
        .get(item)
        .copied()
        .unwrap_or(0)
}
