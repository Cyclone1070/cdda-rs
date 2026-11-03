//! Which vitamin is which, inside the store.
//!
//! Reference: the `vitamin` factory (`src/vitamin.cpp`), which answers a `vitamin_id` by name and hands back the type's caps and its rate. [`crate::digest`] keeps what a
//! body holds; what it could not do was ask what kind of thing a level belongs to, so a drug vitamin could not be told from a nutrient and a level could not be clamped to
//! the type's own ceiling.
//!
//! The store keeps the types themselves rather than a paraphrase of their numbers, for the same reason [`crate::effect_registry`] does: a second copy of a field is drift
//! waiting to happen.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use cdda_content::vitamins::Vitamin;

use crate::singletons;

/// Every vitamin type the store can answer for, by id.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct VitaminTypes(BTreeMap<String, Vitamin>);

/// Marks the entity carrying [`VitaminTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheVitaminTypes;

/// Installs the vitamin types the store will answer from.
pub fn set_vitamin_types(world: &mut World, vitamins: &BTreeMap<String, Vitamin>) {
    let entity = singletons::singleton::<VitaminTypes, TheVitaminTypes>(world);
    world
        .entity_mut(entity)
        .insert(VitaminTypes(vitamins.clone()));
}

/// The table, for a rule that needs the whole thing at once - the body update clamps every level it holds.
///
/// The reference reads the factory directly; the port answers a **copy**, so a caller that is about to change the world does not hold a borrow of it.
#[must_use]
pub fn vitamins_of(world: &World) -> BTreeMap<String, Vitamin> {
    let Some(entity) = singletons::holder::<TheVitaminTypes>(world) else {
        return BTreeMap::new();
    };
    world
        .get::<VitaminTypes>(entity)
        .map(|types| types.0.clone())
        .unwrap_or_default()
}

/// How many types the store knows.
#[must_use]
pub fn count(world: &World) -> usize {
    let Some(entity) = singletons::holder::<TheVitaminTypes>(world) else {
        return 0;
    };
    world
        .get::<VitaminTypes>(entity)
        .map_or(0, |types| types.0.len())
}
