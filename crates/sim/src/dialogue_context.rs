//! The values a program is handed when another one calls it.
//!
//! Reference: `dialogue` carries a `context` map (`dialogue.h`), filled by a `run_eocs` call's `variables` (`npctalk.cpp:6332-6338`) and
//! read by `context_val( '…' )` and by `expects_vars` (`condition.cpp:1111-1131`). It lives for the length of the call, which is why it
//! sits on the speaker and not in a global: nothing else can see it, and it is gone when the call returns.
//!
//! A map is a component rather than something the evaluator threads through its arguments, because the values belong *to the
//! conversation*, and a conversation is a thing in the store.

use bevy_ecs::prelude::*;

/// The context values of the call being run.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogueContext(pub std::collections::BTreeMap<String, String>);

/// The values set right now, or an empty map when no call is running.
#[must_use]
pub fn values(world: &World, speaker: Entity) -> std::collections::BTreeMap<String, String> {
    world
        .get::<DialogueContext>(speaker)
        .map_or_else(std::collections::BTreeMap::new, |context| context.0.clone())
}

/// Whether a name is set in the context.
#[must_use]
pub fn knows(world: &World, speaker: Entity, name: &str) -> bool {
    world
        .get::<DialogueContext>(speaker)
        .is_some_and(|context| context.0.contains_key(name))
}

/// Replaces the context with these values, returning what was there before.
///
/// Returning the old map is what makes a call nestable: the caller puts it back when the program returns, so a program that calls
/// another does not lose its own values.
pub fn put(
    world: &mut World,
    speaker: Entity,
    values: std::collections::BTreeMap<String, String>,
) -> Option<std::collections::BTreeMap<String, String>> {
    if let Some(mut context) = world.get_mut::<DialogueContext>(speaker) {
        return Some(std::mem::replace(&mut context.0, values));
    }
    world.entity_mut(speaker).insert(DialogueContext(values));
    None
}

/// Puts a previous context back, or removes the component when there was none.
pub fn restore(
    world: &mut World,
    speaker: Entity,
    previous: Option<std::collections::BTreeMap<String, String>>,
) {
    match previous {
        Some(values) => {
            if let Some(mut context) = world.get_mut::<DialogueContext>(speaker) {
                context.0 = values;
            }
        }
        None => {
            world.entity_mut(speaker).remove::<DialogueContext>();
        }
    }
}
