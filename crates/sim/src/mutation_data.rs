//! The mutation tables, held once for the whole world and read by the rules that need them.
//!
//! The port keeps data in **entities with marker components** - the same shape every other table uses (`singletons::singleton`) - so a rule asks the world for the table
//! rather than being handed it by a caller who had to remember it.

use bevy_ecs::prelude::*;
use cdda_content::mutations::{Categories, Mutations};

/// The mutation branches, as a component so the world can hold them.
///
/// The content crate's tables are plain data - they know nothing about the ECS - so the port wraps each in a component here. **The wrapper is the seam**: the loader stays
/// free of `bevy_ecs`, and the store still holds the table as an entity rather than a global.
#[derive(Debug, Clone, Default, Component)]
pub struct MutationTable(pub Mutations);

/// The mutation categories, wrapped the same way.
#[derive(Debug, Clone, Default, Component)]
pub struct MutationCategoryTable(pub Categories);

/// The mutation branches, marked so the world can find them.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheMutations;

/// The mutation categories, marked the same way.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheMutationCategories;

/// Installs the mutation tables.
pub fn install(world: &mut World, mutations: Mutations, categories: Categories) {
    let entity = crate::singletons::singleton::<MutationTable, TheMutations>(world);
    world.entity_mut(entity).insert(MutationTable(mutations));
    let entity = crate::singletons::singleton::<MutationCategoryTable, TheMutationCategories>(world);
    world.entity_mut(entity).insert(MutationCategoryTable(categories));
}

/// The mutation branches, when the world has them.
///
/// The reference keeps these in one global table (`mutation_branch::get_all`); the port keeps them as a component on a marked entity, and the reader returns a **copy** so a
/// caller that is about to mutate the world does not hold a borrow of it.
#[must_use]
pub fn holder(world: &World) -> Option<Mutations> {
    let entity = crate::singletons::holder::<TheMutations>(world)?;
    world.get::<MutationTable>(entity).map(|table| table.0.clone())
}

/// The mutation categories, when the world has them.
#[must_use]
pub fn categories(world: &World) -> Option<Categories> {
    let entity = crate::singletons::holder::<TheMutationCategories>(world)?;
    world
        .get::<MutationCategoryTable>(entity)
        .map(|table| table.0.clone())
}
