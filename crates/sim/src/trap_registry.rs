//! Which trap is which, inside the store.
//!
//! The twin of [`crate::effect_registry`], and needed for the same reason: a square says `tr_telepad` by name, and the rule that springs it needs the trap's `action` and its
//! programs. Reference: `trap::loadid`/`tr_id::obj()` (`trap.cpp`), which answers a name with the trap the data defined.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use cdda_content::traps::Trap;

use crate::singletons;

/// Every trap the store can answer for, by id.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct TrapTypes {
    /// The traps, in the order they were installed.
    kinds: Vec<Trap>,
    /// Where each one is in `kinds`.
    by_id: BTreeMap<String, usize>,
}

/// Marks the entity carrying [`TrapTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheTrapTypes;

/// Installs the traps the store will answer from.
pub fn set_traps(world: &mut World, traps: &[Trap]) {
    let mut by_id = BTreeMap::new();
    for (at, trap) in traps.iter().enumerate() {
        by_id.insert(trap.id.clone(), at);
    }
    let entity = singletons::singleton::<TrapTypes, TheTrapTypes>(world);
    world.entity_mut(entity).insert(TrapTypes {
        kinds: traps.to_vec(),
        by_id,
    });
}

/// The trap with this id, when the store knows it.
#[must_use]
pub fn get(world: &World, id: &str) -> Option<Trap> {
    let element = singletons::holder::<TheTrapTypes>(world)?;
    let table = world.get::<TrapTypes>(element)?;
    let at = table.by_id.get(id)?;
    table.kinds.get(*at).cloned()
}

/// How many traps the store holds.
#[must_use]
pub fn len(world: &World) -> usize {
    singletons::holder::<TheTrapTypes>(world)
        .and_then(|element| world.get::<TrapTypes>(element))
        .map_or(0, |table| table.kinds.len())
}
