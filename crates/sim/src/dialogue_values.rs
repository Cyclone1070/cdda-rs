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
    /// A square, which is what a location variable holds.
    ///
    /// Reference: the dialogue's variable map holds a `tripoint` for these (`d.set_value( var, tripoint_abs_ms )`), so a location is a value
    /// like any other rather than a second kind of variable.
    Position {
        /// The square's column.
        x: i32,
        /// The square's row.
        y: i32,
        /// The level.
        z: i32,
    },
}

/// A set of dialogue variables, by name.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct DialogueValues {
    values: BTreeMap<String, Value>,
}

/// How many variables exist in the world, across every holder.
///
/// A census that only asks "did the program answer" cannot tell a program that did something from one that answered after doing nothing. The count of
/// variables, with the entity count and the lines said, is the cheapest fingerprint that separates the two.
#[must_use]
pub fn how_many(world: &World) -> usize {
    let Some(mut query) = world.try_query::<&DialogueValues>() else {
        return 0;
    };
    query.iter(world).map(|held| held.values.len()).sum()
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

/// The square a variable holds, in the scope a target names.
///
/// Reference: `read_var_value` (`npctalk.cpp`) answering a `tripoint`, which is what a `target_var` is.
#[must_use]
pub fn position_of(
    world: &mut bevy_ecs::prelude::World,
    speaker: bevy_ecs::prelude::Entity,
    listener: Option<bevy_ecs::prelude::Entity>,
    target: &cdda_content::dialogue::AssignmentTarget,
) -> Option<(i32, i32, i32)> {
    let (holder, name) = match target {
        cdda_content::dialogue::AssignmentTarget::Variable { scope, name } => (
            crate::conditions::holder_for_scope(world, speaker, listener, scope)?,
            name,
        ),
        cdda_content::dialogue::AssignmentTarget::Bare(name) => (speaker, name),
    };
    match value_of(world, holder, name) {
        Some(Value::Position { x, y, z }) => Some((x, y, z)),
        _ => None,
    }
}

/// A variable's value, or nothing when the holder does not know it.
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

/// Takes a variable away, answering whether it was there.
///
/// Reference: `dialogue::actor::remove_value` (`condition.cpp`), which `u_lose_var` (`npctalk.cpp:3656`) calls. Removing a variable that was never set is
/// **not** an error in the reference, so it is not one here either.
pub fn remove(world: &mut World, holder: Entity, name: &str) -> bool {
    let Some(mut values) = world.get_mut::<DialogueValues>(holder) else {
        return false;
    };
    values.values.remove(name).is_some()
}

/// The many-valued door: `u_val`, `npc_val` and `global_val` all resolve to a holder, and this is what they share.
///
/// Reference: `read_var_info` (`condition.cpp:408-437`) classes a variable by which member names it, and `get_value` then
/// reads it from the player, an NPC or the global map. The port has one creature and one global map, so the two cases the
/// data uses most - `u_val` and `global_val` - both arrive here.
/// The entity holding the global variables, without needing a creature to ask through.
#[must_use]
pub fn global_holder(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheGlobalValues>>()?;
    query.iter(world).next()
}

/// The holder a variable's scope names, and nothing when the scope is one the port cannot resolve.
///
/// `holder_for` answers for a scope with a creature in hand; this one answers when there is none.
#[must_use]
pub fn holder_for(world: &mut World, creature: Entity, global: bool) -> Entity {
    if global {
        singletons::singleton::<DialogueValues, TheGlobalValues>(world)
    } else {
        creature
    }
}
