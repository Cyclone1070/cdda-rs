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
pub struct TimedEvents {
    /// The events, by the name they are known by.
    pub events: std::collections::BTreeMap<String, TimedEvent>,
    /// How many effect-payload events have ever been scheduled, which is what makes their keys distinct.
    ///
    /// Reference: the dialogue's delayed calls all arrive as one `timed_event_type`, so a second one due on the same turn would **replace** the first. The port keeps
    /// them all, and counts so a key is never reused - a counter, not a length, because erasing one must not let the next take its name.
    pub scheduled: u64,
}

/// One scheduled event: when it is due, and the text it carries.
///
/// Reference: `timed_event` (`timed_event.h:20-60`) holds a type, a turn and a **string** - the reference puts a place's new name in it for `OVERRIDE_PLACE` and reads it back
/// from the display (`display.cpp:1243-1246`). The port keeps the same pair rather than a bare turn, because an event with no payload cannot rename anything.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimedEvent {
    /// The turn it comes due.
    pub due: i64,
    /// The text it carries, empty when it carries none.
    pub text: String,
    /// The square it happens on, for the events that name one.
    ///
    /// Reference: `timed_event`'s `point` member (`timed_event.h:20-60`), which `UPDATE_MAPGEN` and `REVERT_LOCATION` fill with where the work is to be done. The port
    /// carries it here rather than folding a square into the text, because every reader of the text would then have to parse it.
    pub place: Option<(i32, i32, i32)>,
    /// An effect to run when it comes due, for the events the dialogue scheduled.
    ///
    /// Reference: `timed_event`'s `effect_on_condition` payload (`timed_event.h:20-60`), which is how `run_eocs` with a `time_in_future` runs later.
    pub effect: Option<cdda_content::dialogue::Effect>,
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
    schedule_carrying_at(world, key, turns, text, None);
}

/// Schedules an event that carries text **and a square**, which is what an update mapgen in the future needs.
///
/// Reference: `timed_events::add( UPDATE_MAPGEN, tif, -1, project_to<coords::ms>( omt_pos ), ..., update_id, key )` (`npctalk.cpp:4854-4860`): the event holds the
/// place, the update's id as its string, and an optional key that later recognises it.
pub fn schedule_carrying_at(
    world: &mut World,
    key: &str,
    turns: i64,
    text: &str,
    place: Option<(i32, i32, i32)>,
) {
    let Some(now) = crate::calendar::of(world).map(|calendar| calendar.turn) else {
        return;
    };
    let due = now.saturating_add(turns);
    let element = crate::singletons::singleton::<TimedEvents, TheTimedEvents>(world);
    if let Some(mut events) = world.get_mut::<TimedEvents>(element) {
        events.events.insert(
            key.to_owned(),
            TimedEvent {
                due,
                text: text.to_owned(),
                place,
                effect: None,
            },
        );
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
    Some(events.events.get(key).is_some_and(|event| event.due <= now))
}

/// The entity holding the timed events, when the world has one.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheTimedEvents>>()?;
    query.iter(world).next()
}

/// The square a named event was scheduled for, or nothing when it names none.
///
/// Reference: `timed_event`'s `point` member (`timed_event.h:20-60`) - `UPDATE_MAPGEN` and `REVERT_LOCATION` fill it with where the work is to be done.
#[must_use]
pub fn place_of(world: &World, key: &str) -> Option<(i32, i32, i32)> {
    let events = world.get::<TimedEvents>(holder(world)?)?;
    events.events.get(key).and_then(|event| event.place)
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
        .events
        .get(key)
        .filter(|event| event.due > now)
        .map(|event| event.text.clone())
}

/// Schedules a dialogue effect to run `turns` from now.
///
/// Reference: `f_run_eocs`' `time_in_future` (`npctalk.cpp:6332-6338`) queues the call as a timed event; the key is the port's own, because the reference keys its map by
/// event **type** and would drop a second call due the same turn, where the data means both to run.
pub fn schedule_effect(world: &mut World, turns: i64, effect: cdda_content::dialogue::Effect) {
    let Some(now) = crate::calendar::of(world).map(|calendar| calendar.turn) else {
        return;
    };
    let due = now.saturating_add(turns);
    let element = crate::singletons::singleton::<TimedEvents, TheTimedEvents>(world);
    let Some(mut events) = world.get_mut::<TimedEvents>(element) else {
        return;
    };
    let ordinal = events.scheduled;
    events.scheduled = events.scheduled.saturating_add(1);
    // Zero padded, so the key's order is the order the effects were scheduled in even past nine of them.
    let key = format!("@effect:{ordinal:012}");
    events.events.insert(
        key,
        TimedEvent {
            due,
            text: String::new(),
            place: None,
            effect: Some(effect),
        },
    );
}

/// How many events are waiting, of any kind.
#[must_use]
pub fn waiting(world: &World) -> usize {
    holder(world)
        .and_then(|element| world.get::<TimedEvents>(element))
        .map_or(0, |events| events.events.len())
}

/// Runs every event due at or before the current turn, and takes each out before running it.
///
/// Reference: `timed_events::process` (`timed_event.cpp:100-130`): it walks the map, erases each event whose turn has come, then processes it - so an event that
/// schedules itself again is not erased a second time by this pass. `UPDATE_MAPGEN` applies its update at the square it carries, a dialogue payload runs through the
/// program runner (which carries the depth a nested call needs), and any other name is a generic event that has simply happened - the reference's own `default`.
pub fn process_due(world: &mut World, speaker: Entity) {
    let Some(now) = crate::calendar::of(world).map(|calendar| calendar.turn) else {
        return;
    };
    let Some(element) = holder(world) else {
        return;
    };
    let due: Vec<(String, TimedEvent)> = world
        .get::<TimedEvents>(element)
        .map(|events| {
            events
                .events
                .iter()
                .filter(|(_, event)| event.due <= now)
                .map(|(key, event)| (key.clone(), event.clone()))
                .collect()
        })
        .unwrap_or_default();
    if due.is_empty() {
        return;
    }
    // **Taken out before they run**, so a handler that looks at the store sees the event as gone.
    if let Some(mut events) = world.get_mut::<TimedEvents>(element) {
        for (key, _) in &due {
            events.events.remove(key);
        }
    }
    for (key, event) in due {
        if let Some(effect) = event.effect {
            run_payload(world, speaker, &effect);
            continue;
        }
        if key == "UPDATE_MAPGEN"
            && let Some(place) = event.place
        {
            let _ = crate::mapgen_update::apply(world, &event.text, place);
            continue;
        }
        // **A revert puts back the squares the event names** (`timed_event.cpp:336-340`): its submap, not every square the store remembers.
        if key == "REVERT_SUBMAP"
            && let Some(place) = event.place
        {
            let _ = crate::revert::restore_at(world, place);
            continue;
        }
        // **The temple's flood is one step of a rising water level**, and it schedules the next step itself: `timed_event.cpp:271-274` adds `TEMPLE_FLOOD` again two or
        // three turns out, and stops when a step changes nothing because the chamber is full.
        if key == "TEMPLE_FLOOD" {
            let flood = crate::temple_flood::run(world);
            if let Some(line) = crate::temple_flood::line(flood.reached_player) {
                crate::messages::push(world, line);
            }
            if flood.flooded {
                let mut rolls = crate::singletons::take_rolls(world);
                let turns = rolls.as_mut().map_or(2, |dice| i64::from(dice.rng(2, 3)));
                if let Some(dice) = rolls.take() {
                    crate::singletons::put_rolls(world, dice);
                }
                schedule(world, "TEMPLE_FLOOD", turns);
            }
        }
    }
}

/// Runs one dialogue payload the way the turn loop runs a scheduled effect.
fn run_payload(world: &mut World, speaker: Entity, effect: &cdda_content::dialogue::Effect) {
    // A scheduled call is a `run_eocs` payload, and a call is run by the **program runner** - which carries the depth a nested call needs - rather than by the
    // single-effect runner.
    if let cdda_content::dialogue::Effect::RunEoc(programs) = effect {
        for program in programs {
            let _ = crate::eoc_registry::run_ref(world, speaker, None, program, 0);
        }
        return;
    }
    let _ = crate::dialogue_effects::run_effect(world, speaker, None, effect);
}
