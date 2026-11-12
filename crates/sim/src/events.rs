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

/// The event vocabulary in the world, so a dispatch can name a position.
#[derive(Debug, Default, Component)]
pub struct EventVocabulary {
    table: cdda_content::events::EventTable,
}

/// Marks the entity holding the [`EventVocabulary`].
#[derive(Debug, Default, Clone, Copy, Component)]
pub struct TheEventVocabulary;

/// Installs the reference's event table, which is what an argument's position means.
pub fn install_vocabulary(world: &mut World, table: cdda_content::events::EventTable) {
    let entity = singletons::singleton::<EventVocabulary, TheEventVocabulary>(world);
    world.entity_mut(entity).insert(EventVocabulary { table });
}

/// What each position of an event's arguments is called, or nothing when the world has no table for that type.
#[must_use]
pub fn field_names(world: &World, kind: &str) -> Option<Vec<String>> {
    singletons::holder::<TheEventVocabulary>(world)
        .and_then(|entity| world.get::<EventVocabulary>(entity))
        .and_then(|held| held.table.fields_of(kind).map(<[String]>::to_vec))
}

/// Answers every event waiting, and says how many programs ran.
///
/// Reference: `eoc_events::notify` (`effect_on_condition.cpp:519-580`). The reference picks a character for the program from the event's own fields - `avatar_id`,
/// `character`, `attacker`, `killer`, `npc`, in that order - and falls back to the avatar; it puts every field of the event into the dialogue's **context**, so a program
/// reads them with `context_val( .. )`. The port binds the arguments by position through the dumped table, which is the same statement with the order made explicit.
pub fn dispatch(world: &mut World, avatar: Entity) -> usize {
    let waiting = drain(world);
    let mut ran = 0;
    for event in waiting {
        let names = field_names(world, &event.kind).unwrap_or_default();
        let mut bindings = std::collections::BTreeMap::new();
        for (position, value) in event.args.iter().enumerate() {
            if let Some(name) = names.get(position) {
                bindings.insert(name.clone(), value.clone());
            }
        }
        for id in crate::eoc_registry::event_programs(world, &event.kind) {
            let previous = crate::dialogue_context::put(world, avatar, bindings.clone());
            let _ = crate::eoc_registry::run_program(world, avatar, None, &id, 0);
            crate::dialogue_context::restore(world, avatar, previous);
            ran += 1;
        }
    }
    ran
}
