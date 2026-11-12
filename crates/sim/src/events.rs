//! The **event queue**: something happened, and what the game does about it.
//!
//! Reference: `cata::event` plus the event bus (`event.h:960-1080`, `event_bus` in `event.cpp`). A rule that notices a change sends an event; anything watching that kind answers it.
//! The reference's bus calls subscribers as it sends, which is why a subscriber can change the world in the middle of the sender. The port keeps **one queue**, drained at a
//! declared point in the turn, so a sender never re-enters the rules that sent it - the same decision this port makes everywhere a call would otherwise nest.
//!
//! Nothing here knows what an event *means*: the argument names are the reference's own table (`cdda_content::events::EventTable`), and the meanings live in the programs that
//! answer an event.

use bevy_ecs::prelude::*;

use crate::singletons;

/// Something that happened, with its arguments in the order the reference's table gives them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The event type's name, exactly as the data writes it.
    pub kind: String,
    /// The arguments, positionally - position `n` is the type's `n`th field.
    pub args: Vec<String>,
}

impl Event {
    /// An event of a named kind with its arguments.
    #[must_use]
    pub fn new(kind: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            kind: kind.into(),
            args,
        }
    }
}

/// The queue itself, which is the only place a sent event waits.
#[derive(Debug, Default, Component)]
pub struct EventQueue {
    pending: Vec<Event>,
    /// Every event sent since the world was made, in order, so a test can say what happened without draining the queue.
    sent: Vec<Event>,
}

/// Marks the one entity that holds the [`EventQueue`].
#[derive(Debug, Default, Clone, Copy, Component)]
pub struct TheEventQueue;

/// The queue, made on first use - a world with nothing to report has no entity for it.
pub fn queue(world: &mut World) -> bevy_ecs::change_detection::Mut<'_, EventQueue> {
    let entity = singletons::singleton::<EventQueue, TheEventQueue>(world);
    world
        .get_mut::<EventQueue>(entity)
        .expect("the marker names the queue")
}

/// Sends an event, which waits until the queue is drained.
pub fn send(world: &mut World, event: Event) {
    let mut held = queue(world);
    held.pending.push(event.clone());
    held.sent.push(event);
}

/// Takes everything waiting, in the order it was sent.
pub fn drain(world: &mut World) -> Vec<Event> {
    std::mem::take(&mut queue(world).pending)
}

/// Whether anything is waiting.
#[must_use]
pub fn has_pending(world: &World) -> bool {
    singletons::holder::<TheEventQueue>(world).is_some_and(|entity| {
        world
            .get::<EventQueue>(entity)
            .is_some_and(|held| !held.pending.is_empty())
    })
}

/// Every event ever sent, in order - what the world has reported, whether or not anyone answered.
#[must_use]
pub fn sent(world: &World) -> Vec<Event> {
    singletons::holder::<TheEventQueue>(world)
        .and_then(|entity| world.get::<EventQueue>(entity))
        .map_or_else(Vec::new, |held| held.sent.clone())
}
