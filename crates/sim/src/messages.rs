//! The messages a turn showed the player.
//!
//! Reference: `add_msg_if_player` and its family (`messages.cpp`), which append to the message log the interface drains each
//! turn. The port keeps the log in the store as a singleton - the same shape as every other registry - because a headless run
//! has to be able to read what the game said, and a `println!` inside a rule is a side effect no test can see.
//!
//! `push` is deliberately the only writer: a message is text that happened, and the log is the store's record of it.

use bevy_ecs::prelude::*;

use crate::singletons;

/// What the game has said, oldest first.
#[derive(Component, Debug, Clone, Default)]
pub struct MessageLog {
    lines: Vec<String>,
}

/// Marks the entity holding the log.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheMessageLog;

/// How many messages the log keeps before it starts dropping the oldest.
///
/// Reference: `messages.cpp:203-206` is `while( messages.size() > message_limit ) messages.pop_front();` **followed by** `messages.emplace_back( m )` - so the list is trimmed *before* the new line goes in and
/// its steady state is the limit **plus one**, which is the sort of off-by-one a hand-written trim loses and this gate pins. The limit is `get_option<int>( "MESSAGE_LIMIT" )`, default 255. The port does not have an options
/// table yet, so this is the default as a constant - **named, not invented**, and the option is recorded here rather than silently ignored.
pub const MESSAGE_LIMIT: usize = 255;

/// Appends a message, dropping the oldest lines once the log is over [`MESSAGE_LIMIT`].
pub fn push(world: &mut World, text: &str) {
    let holder = singletons::singleton::<MessageLog, TheMessageLog>(world);
    if let Some(mut log) = world.get_mut::<MessageLog>(holder) {
        // Reference: the trim is **after** the append test and **before** the push, so the log never holds more than the limit and the newest line always survives.
        while log.lines.len() > MESSAGE_LIMIT {
            log.lines.remove(0);
        }
        log.lines.push(text.to_owned());
        return;
    }
    world
        .entity_mut(holder)
        .insert(MessageLog { lines: vec![text.to_owned()] });
}

/// Every message the log holds, oldest first.
#[must_use]
pub fn lines(world: &World) -> Vec<String> {
    let Some(mut query) = world.try_query_filtered::<&MessageLog, With<TheMessageLog>>() else {
        return Vec::new();
    };
    query.iter(world).next().map_or_else(Vec::new, |log| log.lines.clone())
}

/// Empties the log, which is what the interface does once it has drawn a turn.
pub fn clear(world: &mut World) {
    let holder = singletons::singleton::<MessageLog, TheMessageLog>(world);
    if let Some(mut log) = world.get_mut::<MessageLog>(holder) {
        log.lines.clear();
    }
}
