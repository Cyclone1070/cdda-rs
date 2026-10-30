//! Which item is which, inside the store.
//!
//! Reference: `item_controller` / `item_factory`'s `itype_id` table, which numbers every item type and answers both
//! directions - the number a stack carries, and the id the data writes in recipes and components.
//!
//! The port kept that table outside the store: callers passed a `name_of` closure around, and the simulation could not
//! answer "what is in my hands" without one. That is a second source of truth wearing a function's clothes, so the
//! names live here, on a singleton entity, and every rule that needs them asks the store.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use crate::singletons;
use crate::ItemTypeId;

/// Every item type's id, by the number the store uses for it.
///
/// Index `n` is [`ItemTypeId(n)`]. Index zero is the null item, as it is in the reference's registry.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemTypes {
    /// The name of each kind, in registry order.
    names: Vec<String>,
    /// The number of each kind, by name.
    by_name: BTreeMap<String, ItemTypeId>,
}

/// Marks the entity carrying [`ItemTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheItemTypes;

/// Installs the registry the store will answer from.
///
/// Later calls replace it, which is what a second world load does. The null item stays at zero whatever the data says,
/// because a stack with no kind has to have a number that means nothing.
pub fn set_item_types(world: &mut World, ids: &[&str]) {
    let mut names = vec![String::new()];
    let mut by_name = BTreeMap::new();
    for id in ids {
        #[allow(clippy::cast_possible_truncation)]
        let number = ItemTypeId(names.len() as u32);
        by_name.insert((*id).to_owned(), number);
        names.push((*id).to_owned());
    }
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world
        .entity_mut(entity)
        .insert(ItemTypes { names, by_name });
}

/// A copy of the registry that rules can hold while they work on the store.
///
/// Taken as a value because a rule that mutates the store cannot also be holding a borrow of it - and a rule that has
/// to ask the store for every name it meets would have to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemNames {
    /// The name of each kind, in registry order.
    pub names: Vec<String>,
    /// The number of each kind, by name.
    pub by_name: BTreeMap<String, ItemTypeId>,
}

impl ItemNames {
    /// The name of a kind, as the data spells it.
    #[must_use]
    pub fn name_of(&self, kind: ItemTypeId) -> Option<String> {
        self.names
            .get(kind.0 as usize)
            .filter(|name| !name.is_empty())
            .cloned()
    }

    /// The kind a name refers to.
    #[must_use]
    pub fn id_of(&self, name: &str) -> Option<ItemTypeId> {
        self.by_name.get(name).copied()
    }
}

/// Takes a copy of the registry.
#[must_use]
pub fn snapshot(world: &mut World) -> ItemNames {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world.get::<ItemTypes>(entity).map_or_else(ItemNames::default, |registry| ItemNames {
        names: registry.names.clone(),
        by_name: registry.by_name.clone(),
    })
}

/// The name of a kind, as the data spells it.
#[must_use]
pub fn name_of(world: &mut World, kind: ItemTypeId) -> Option<String> {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let registry = world.get::<ItemTypes>(entity)?;
    registry.names.get(kind.0 as usize).filter(|name| !name.is_empty()).cloned()
}

/// The kind a name refers to.
#[must_use]
pub fn id_of(world: &mut World, name: &str) -> Option<ItemTypeId> {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let registry = world.get::<ItemTypes>(entity)?;
    registry.by_name.get(name).copied()
}

/// How many kinds the registry knows, not counting the null item.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world.get::<ItemTypes>(entity).map_or(0, |registry| registry.by_name.len())
}
