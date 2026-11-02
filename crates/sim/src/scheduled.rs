//! Effects that happen at a turn in the future.
//!
//! Reference: the reference's timed events (`timed_event.cpp`) carry a payload - an effect-on-condition, a transform, a light level - and the game processes the ones
//! that have come due once per turn. The port keeps the payloads in one component on one entity, keyed by the turn they are due, and runs them from a stage of the
//! turn loop - the same shape as every other store here.

use bevy_ecs::prelude::{Component, Entity, With, World};

use cdda_content::dialogue::Effect;

/// Effects waiting for a turn, by the turn they are due.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ScheduledEffects(pub std::collections::BTreeMap<i64, Vec<Effect>>);

/// Marks the one entity that holds the scheduled effects.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheScheduledEffects;

/// Schedules an effect to run `turns` from now.
///
/// Reference: `timed_event` with a payload: the due turn is the calendar plus the duration the data gave, and two effects due on the same turn both run, in the order
/// they were scheduled.
pub fn schedule_effect(world: &mut World, turns: i64, effect: Effect) {
    let Some(now) = crate::calendar::of(world).map(|calendar| calendar.turn) else {
        return;
    };
    let due = now.saturating_add(turns);
    let holder = crate::singletons::singleton::<ScheduledEffects, TheScheduledEffects>(world);
    if let Some(mut waiting) = world.get_mut::<ScheduledEffects>(holder) {
        waiting.0.entry(due).or_default().push(effect);
    }
}

/// Every effect due at or before a turn, taken out of the store.
pub fn due_at(world: &mut World, turn: i64) -> Vec<Effect> {
    let Some(holder) = holder(world) else {
        return Vec::new();
    };
    let mut taken = Vec::new();
    if let Some(mut waiting) = world.get_mut::<ScheduledEffects>(holder) {
        let due: Vec<i64> = waiting.0.range(..=turn).map(|(at, _)| *at).collect();
        for at in due {
            if let Some(effects) = waiting.0.remove(&at) {
                taken.extend(effects);
            }
        }
    }
    taken
}

/// Runs everything that has come due, for the speaker the turn loop speaks for.
///
/// Reference: `game::process_events`, which the turn loop calls once a turn: a payload whose turn has come runs now, and one due later stays where it is.
pub fn run_due(world: &mut World, speaker: Entity) {
    let Some(now) = crate::calendar::of(world).map(|calendar| calendar.turn) else {
        return;
    };
    for effect in due_at(world, now) {
        // A scheduled call is a `run_eocs` payload, and a call is run by the **program runner** - which carries the depth a nested call needs - rather than by the
        // single-effect runner.
        if let Effect::RunEoc(programs) = &effect {
            for program in programs {
                let _ = crate::eoc_registry::run_ref(world, speaker, None, program, 0);
            }
            continue;
        }
        let _ = crate::dialogue_effects::run_effect(world, speaker, None, &effect);
    }
}

/// How many effects are waiting.
#[must_use]
pub fn waiting(world: &mut World) -> usize {
    let Some(holder) = holder(world) else {
        return 0;
    };
    world
        .get::<ScheduledEffects>(holder)
        .map_or(0, |waiting| waiting.0.values().map(Vec::len).sum())
}

/// The entity holding the scheduled effects, when one exists.
fn holder(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheScheduledEffects>>()?;
    query.iter(world).next()
}
