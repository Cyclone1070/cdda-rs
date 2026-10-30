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

/// What everything a creature carries weighs, in grams.
///
/// Reference: `Character::weight_carried`, which sums the items in the inventory. The port sums the entities that point
/// at the creature, which is the same set without a container to walk.
#[must_use]
pub fn weight_carried(world: &mut World, creature: Entity, names: &crate::item_registry::ItemNames) -> i64 {
    let mut total = 0;
    let mut query = world.query::<(&Carried, &GroundItem)>();
    for (carried, item) in query.iter(world) {
        if carried.0 == creature {
            total += names.weight_of(item.kind) * i64::from(item.charges.max(1));
        }
    }
    total
}

/// How much room everything a creature carries takes, in millilitres.
///
/// Reference: `Character::volume_carried`.
#[must_use]
pub fn volume_carried(world: &mut World, creature: Entity, names: &crate::item_registry::ItemNames) -> i64 {
    let mut total = 0;
    let mut query = world.query::<(&Carried, &GroundItem)>();
    for (carried, item) in query.iter(world) {
        if carried.0 == creature {
            total += i64::from(names.volume_of(item.kind)) * i64::from(item.charges.max(1));
        }
    }
    total
}

/// How many of one thing a creature is carrying.
#[must_use]
pub fn how_many(world: &mut World, creature: Entity, item: &str, name_of: impl Fn(crate::ItemTypeId) -> Option<String>) -> i32 {
    carried_by_id(world, creature, name_of)
        .get(item)
        .copied()
        .unwrap_or(0)
}

/// How much of one named thing a creature carries: the stacks, and the charges in them.
///
/// This is the `&World` form of [`carried_by_id`], for callers that must not change the store - answering a dialogue
/// question is one of those, and a question that needed a mutable world would be a question no schedule can ask from
/// inside a read pass.
#[must_use]
pub fn count_of(world: &World, creature: Entity, name: &str) -> (i32, i32) {
    let mut stacks = 0;
    let mut charges = 0;
    let Some(mut query) = world.try_query::<(&Carried, &GroundItem)>() else {
        return (0, 0);
    };
    for (carried, item) in query.iter(world) {
        if carried.0 != creature {
            continue;
        }
        if crate::item_registry::name_in(world, item.kind).as_deref() != Some(name) {
            continue;
        }
        stacks += 1;
        charges += item.charges.max(1);
    }
    (stacks, charges)
}
