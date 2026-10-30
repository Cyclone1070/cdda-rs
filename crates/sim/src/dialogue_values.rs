//! The variables a dialogue reads and writes.
//!
//! Reference: `dialogue::get_value` and `dialogue::set_value` (`dialogue.cpp`), which answer by variable **type**: the
//! player's (`u_val`), an NPC's (`npc_val`), a global one (`global_val`), one carried in the dialogue's own context
//! (`context_val`), or one addressed by name (`var_val`). The port had none of them, which is why 518 `math` conditions and
//! 238 `compare_string` ones were refused: they name variables the store could not answer for.
//!
//! A value is a number or a string, and "absent" is a third answer - not zero and not the empty string. The reference is the
//! same: `maybe_get_value` answers a pointer that may be null, and the conditions that read it treat a null as "no".

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use crate::singletons;

/// One dialogue value: what a variable holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A number, which is what arithmetic and comparisons read.
    Number(f64),
    /// A string, which is what `compare_string` reads.
    Text(String),
}

/// A set of dialogue variables, by name.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct DialogueValues {
    values: BTreeMap<String, Value>,
}

/// Marks the entity holding the **global** variables, which every dialoguer shares.
///
/// Reference: `global_variables`, one map for the whole game. The port keeps it as a singleton so a value written by one
/// conversation is the value the next one reads, which is what "global" means.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheGlobalValues;

/// Every value this set holds, which is what a save has to write and a test can count.
#[must_use]
pub fn all(world: &World, holder: Entity) -> BTreeMap<String, Value> {
    world
        .get::<DialogueValues>(holder)
        .map_or_else(BTreeMap::new, |values| values.values.clone())
}

/// The value a name holds, or nothing when it holds none.
#[must_use]
pub fn value_of(world: &World, holder: Entity, name: &str) -> Option<Value> {
    world
        .get::<DialogueValues>(holder)
        .and_then(|values| values.values.get(name).cloned())
}

/// The number a name holds, or nothing - which is also what a name holding text answers.
#[must_use]
pub fn number_of(world: &World, holder: Entity, name: &str) -> Option<f64> {
    match value_of(world, holder, name) {
        Some(Value::Number(number)) => Some(number),
        _ => None,
    }
}

/// The text a name holds, or nothing - which is also what a name holding a number answers.
#[must_use]
pub fn text_of(world: &World, holder: Entity, name: &str) -> Option<String> {
    match value_of(world, holder, name) {
        Some(Value::Text(text)) => Some(text),
        _ => None,
    }
}

/// Sets a value on a holder.
pub fn set(world: &mut World, holder: Entity, name: &str, value: Value) {
    if let Some(mut values) = world.get_mut::<DialogueValues>(holder) {
        values.values.insert(name.to_owned(), value);
        return;
    }
    let mut fresh = DialogueValues::default();
    fresh.values.insert(name.to_owned(), value);
    world.entity_mut(holder).insert(fresh);
}

/// The many-valued door: `u_val`, `npc_val` and `global_val` all resolve to a holder, and this is what they share.
///
/// Reference: `read_var_info` (`condition.cpp:408-437`) classes a variable by which member names it, and `get_value` then
/// reads it from the player, an NPC or the global map. The port has one creature and one global map, so the two cases the
/// data uses most - `u_val` and `global_val` - both arrive here.
#[must_use]
pub fn holder_for(world: &mut World, creature: Entity, global: bool) -> Entity {
    if global {
        singletons::singleton::<DialogueValues, TheGlobalValues>(world)
    } else {
        creature
    }
}
