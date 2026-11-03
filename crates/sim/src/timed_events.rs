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
pub struct TimedEvents(pub std::collections::BTreeMap<String, TimedEvent>);

/// One scheduled event: when it is due, and the text it carries.
///
/// Reference: `timed_event` (`timed_event.h:20-60`) holds a type, a turn and a **string** - the reference puts a place's new name in it for `OVERRIDE_PLACE` and reads it back
/// from the display (`display.cpp:1243-1246`). The port keeps the same pair rather than a bare turn, because an event with no payload cannot rename anything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimedEvent {
    /// The turn it comes due.
    pub due: i64,
    /// The text it carries, empty when it carries none.
    pub text: String,
}

/// Marks the one entity that holds the world's timed events.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Copy, Default)]
pub struct TheTimedEvents;

/// Schedules every event named `key` for `turns` from now, replacing anything already scheduled under that name.
///
/// Reference: `timed_events::set_all` (`timed_event.cpp`): the key is the event, the turn is now plus the duration the data gave, and a key set twice ends up with
/// the later turn - which the reference does by erasing the old entry before adding the new one.
pub fn schedule(world: &mut World, key: &str, turns: i64) {
    schedule_carrying(world, key, turns, "");
}

/// Schedules an event that carries text, which is how a place gets renamed.
///
/// Reference: `timed_events::add( OVERRIDE_PLACE, ..., string_id )` (`npctalk.cpp:4810-4814`) - the same event, with the name in it.
pub fn schedule_carrying(world: &mut World, key: &str, turns: i64, text: &str) {
    let Some(now) = crate::calendar::of(world).map(|calendar| calendar.turn) else {
        return;
    };
    let due = now.saturating_add(turns);
    let element = crate::singletons::singleton::<TimedEvents, TheTimedEvents>(world);
    if let Some(mut events) = world.get_mut::<TimedEvents>(element) {
        events
            .0
            .insert(key.to_owned(), TimedEvent { due, text: text.to_owned() });
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
    Some(events.0.get(key).is_some_and(|event| event.due <= now))
}

/// The entity holding the timed events, when the world has one.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheTimedEvents>>()?;
    query.iter(world).next()
}

/// The text an event carries, while it is still pending.
///
/// Reference: `display::current_position_text` (`display.cpp:1243-1246`) answers the event's `string_id` while an `OVERRIDE_PLACE` is pending and the place's real name
/// otherwise - so a rename lasts exactly as long as the event does.
#[must_use]
pub fn text_of(world: &World, key: &str) -> Option<String> {
    let now = crate::calendar::of(world).map(|calendar| calendar.turn)?;
    let element = holder(world)?;
    world
        .get::<TimedEvents>(element)?
        .0
        .get(key)
        .filter(|event| event.due > now)
        .map(|event| event.text.clone())
}
