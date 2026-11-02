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

/// Takes named things out of what a creature carries: whole stacks, or charges from them.
///
/// Reference: `Character::use_amount` and `use_charges` (`npctalk.cpp:3950-3965` calls both). A stack that runs out of charges leaves the store, because an empty
/// stack is not a thing anybody carries - which is the same rule the reference's `item::operator==`-driven removal applies.
///
/// Answers whether everything asked for was there. Nothing is taken when it is not: a partial removal would leave the world in a state neither the data nor the
/// reference describes.
pub fn take(
    world: &mut World,
    creature: Entity,
    name: &str,
    stacks: i32,
    charges: i32,
) -> bool {
    let mut found = Vec::new();
    let Some(mut query) = world.try_query::<(Entity, &crate::creature::Carried, &crate::GroundItem)>() else {
        return false;
    };
    for (entity, carried, item) in query.iter(world) {
        if carried.0 != creature {
            continue;
        }
        if crate::item_registry::name_in(world, item.kind).as_deref() != Some(name) {
            continue;
        }
        found.push((entity, item.charges));
    }
    let (held_stacks, held_charges) = found.iter().fold((0, 0), |(many, held), (_, charges)| {
        (many + 1, held + (*charges).max(1))
    });
    if stacks > 0 && held_stacks < stacks {
        return false;
    }
    if charges > 0 && held_charges < charges {
        return false;
    }
    let mut left_stacks = stacks;
    let mut left_charges = charges;
    for (entity, held) in found {
        if left_stacks > 0 {
            left_stacks -= 1;
            world.entity_mut(entity).despawn();
            continue;
        }
        if left_charges <= 0 {
            break;
        }
        let taken = left_charges.min(held.max(1));
        left_charges -= taken;
        let remaining = held.max(1) - taken;
        if remaining <= 0 {
            world.entity_mut(entity).despawn();
        } else if let Some(mut item) = world.get_mut::<crate::GroundItem>(entity) {
            item.charges = remaining;
        }
    }
    true
}

/// Removes **every** item of one kind from a creature, whatever its charges.
///
/// Reference: `f_remove_item_with` (`npctalk.cpp:4046-4056`) hands the actor's whole inventory to `remove_items_with` with a predicate on the item's type, so every
/// stack of that kind goes and every other kind stays. That is a different rule from [`take`], which removes a named number of stacks and refuses a partial removal.
pub fn remove_all_of(world: &mut World, creature: Entity, name: &str) -> bool {
    let (stacks, _) = count_of(world, creature, name);
    if stacks == 0 {
        return false;
    }
    take(world, creature, name, stacks, 0)
}
