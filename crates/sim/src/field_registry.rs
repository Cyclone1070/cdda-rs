//! Which field is which, inside the store.
//!
//! Reference: `field_type`'s factory (`src/field_type.cpp`), which numbers every field type - `fd_fire` is 14, `fd_smoke`
//! is 19, and the numbers are the order the data registered them in. The port had those numbers as constants and no way to
//! go from a name to one, so a spell saying `fd_fatigue` had nothing to act on.
//!
//! The numbers are not invented here: they are the **position** of a name in the list the oracle dumps
//! (`dump_fields` walks `get_all_field_types().get_all()`), which is the same order the reference's `int_id` is assigned in.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use cdda_core_types::field::FieldId;

use crate::singletons;

/// Every field type's name, by the number the store uses for it.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldNames {
    /// The name of each field, at the index that is its id.
    names: Vec<String>,
    /// The number of each field, by name.
    by_name: BTreeMap<String, FieldId>,
}

/// Marks the entity carrying [`FieldNames`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheFieldNames;

/// Installs the field names the store will answer from, each with the number the reference gave it.
///
/// The numbers are **stated**, not derived from the list's order: the reference's list contains an entry whose position and
/// whose number disagree, and a port that counted positions instead would key every field rule to the wrong field by one.
/// `dump_fields` now carries each field's own `num`, so the caller hands over what the reference says rather than a guess.
pub fn set_field_names(world: &mut World, kinds: &[(&str, u32)]) {
    let mut by_name = BTreeMap::new();
    let highest = kinds.iter().map(|(_, num)| *num).max().unwrap_or(0);
    #[allow(clippy::cast_possible_truncation)]
    let mut names = vec![String::new(); highest as usize + 1];
    for (name, num) in kinds {
        by_name.insert((*name).to_owned(), FieldId(*num));
        #[allow(clippy::cast_possible_truncation)]
        let at = *num as usize;
        if let Some(slot) = names.get_mut(at) {
            slot.clone_from(&(*name).to_owned());
        }
    }
    let entity = singletons::singleton::<FieldNames, TheFieldNames>(world);
    world.entity_mut(entity).insert(FieldNames { names, by_name });
}

/// The number a field name refers to, or nothing when the store has never heard of it.
#[must_use]
pub fn id_of(world: &mut World, name: &str) -> Option<FieldId> {
    let entity = singletons::singleton::<FieldNames, TheFieldNames>(world);
    world
        .get::<FieldNames>(entity)
        .and_then(|names| names.by_name.get(name).copied())
}

/// The name of a field, as the data spells it.
#[must_use]
pub fn name_of(world: &mut World, id: FieldId) -> Option<String> {
    let entity = singletons::singleton::<FieldNames, TheFieldNames>(world);
    world
        .get::<FieldNames>(entity)
        .and_then(|names| names.names.get(id.0 as usize).cloned())
        .filter(|name| !name.is_empty())
}

/// How many field names the store knows.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let entity = singletons::singleton::<FieldNames, TheFieldNames>(world);
    world
        .get::<FieldNames>(entity)
        .map_or(0, |names| names.by_name.len())
}
