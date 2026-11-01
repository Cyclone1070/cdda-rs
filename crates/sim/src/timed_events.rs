//! Events that happen at a named time in the future.
//!
//! Reference: `timed_events` (`timed_event.cpp`), the global map the dialogue's `alter_timed_events` writes into and `u_timed_event_occurred` reads back. The port
//! keeps it as one component on one entity - the same shape as every other store here - and answers from the clock rather than from a flag, because "has it
//! happened" is a question about turns, not about a bit somebody had to remember to flip.

use bevy_ecs::prelude::{Entity, With, World};

/// When each named event is due, in turns since the cataclysm.
///
/// A key with no turn is an event nobody has scheduled; a key with a turn in the past has **happened**, which is the reference's own reading
/// (`timed_event::turn_` compared against the calendar).
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Default, PartialEq)]
pub struct TimedEvents(pub std::collections::BTreeMap<String, i64>);

/// Marks the one entity that holds the world's timed events.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Copy, Default)]
pub struct TheTimedEvents;

/// Schedules every event named `key` for `turns` from now, replacing anything already scheduled under that name.
///
/// Reference: `timed_events::set_all` (`timed_event.cpp`): the key is the event, the turn is now plus the duration the data gave, and a key set twice ends up with
/// the later turn - which the reference does by erasing the old entry before adding the new one.
pub fn schedule(world: &mut World, key: &str, turns: i64) {
    let Some(now) = crate::calendar::of(world).map(|calendar| calendar.turn) else {
        return;
    };
    let due = now.saturating_add(turns);
    let element = crate::singletons::singleton::<TimedEvents, TheTimedEvents>(world);
    if let Some(mut events) = world.get_mut::<TimedEvents>(element) {
        events.0.insert(key.to_owned(), due);
    }
}

/// Whether a named event is due at or before the current turn.
///
/// Reference: `timed_events::queued` and the dialogue condition built on it: an event that was scheduled and has come due has **happened**, and one nobody
/// scheduled has not. Answering `false` for an unknown key is the reference's own answer, not a refusal - the data asks about events it never scheduled.
#[must_use]
pub fn has_happened(world: &World, key: &str) -> Option<bool> {
    let now = crate::calendar::of(world).map(|calendar| calendar.turn)?;
    let element = holder(world)?;
    let events = world.get::<TimedEvents>(element)?;
    Some(events.0.get(key).is_some_and(|due| *due <= now))
}

/// The entity holding the timed events, when the world has one.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheTimedEvents>>()?;
    query.iter(world).next()
}
