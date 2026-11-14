//! The messages a turn showed the player.
//!
//! Reference: `add_msg_if_player` and its family (`messages.cpp`), which append to the message log the interface drains each
//! turn. The port keeps the log in the store as a singleton - the same shape as every other registry - because a headless run
//! has to be able to read what the game said, and a `println!` inside a rule is a side effect no test can see.
//!
//! `push` is deliberately the only writer: a message is text that happened, and the log is the store's record of it.

use bevy_ecs::prelude::*;

use crate::singletons;

/// How a message reads: the reference's `game_message_type` (`enums.h:361-379`), in its own order.
///
/// The names are the reference's, shortened only where C++ spells the same idea: `m_good` is [`MessageKind::Good`]. **`Neutral` is the default for an untyped message**, which is easy to get wrong
/// because the most common call site in the reference is the typed one, `m_info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MessageKind {
    /// Something good happened to the player.
    Good,
    /// Something bad happened to the player.
    Bad,
    /// Both at once, such as a mutation with a good and a bad half.
    Mixed,
    /// A warning about a danger.
    Warning,
    /// Information about something - the type most examine and inventory messages use.
    Info,
    /// Nothing notable happened, such as a miss.
    #[default]
    Neutral,
    /// Only shown while debug mode is on.
    Debug,
    /// A hit to the head, for the floating damage numbers.
    Headshot,
    /// A critical hit, for the same.
    Critical,
    /// A glancing hit, for the same.
    Grazing,
}

/// One message, with the severity it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// What it said.
    pub text: String,
    /// How it reads.
    pub kind: MessageKind,
}

/// What the game has said, oldest first.
#[derive(Component, Debug, Clone, Default)]
pub struct MessageLog {
    entries: Vec<Message>,
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

/// Appends an untyped message, which the reference stores as [`MessageKind::Neutral`].
///
/// Reference: `add_msg_string( msg )` forwards to `m_neutral` (`messages.cpp:174-176`).
pub fn push(world: &mut World, text: &str) {
    push_kind(world, text, MessageKind::Neutral);
}

/// Appends a message with a severity, dropping the oldest lines once the log is over [`MESSAGE_LIMIT`].
///
/// Reference: `add_msg_string( msg, type, flags )` (`messages.cpp:182-205`). Three rules come before anything is stored:
///
/// * **an empty message is refused** (`if( msg.empty() ... ) return;`) - an empty line would otherwise be a line the player sees and the turn digest counts;
/// * **`m_debug` is refused while debug mode is off**, which is the port's only mode;
/// * and only then does the trim run, **before** the push, so the newest line always survives.
pub fn push_kind(world: &mut World, text: &str, kind: MessageKind) {
    if text.is_empty() || kind == MessageKind::Debug {
        return;
    }
    let entry = Message {
        text: text.to_owned(),
        kind,
    };
    let holder = singletons::singleton::<MessageLog, TheMessageLog>(world);
    if let Some(mut log) = world.get_mut::<MessageLog>(holder) {
        while log.entries.len() > MESSAGE_LIMIT {
            log.entries.remove(0);
        }
        log.entries.push(entry);
        return;
    }
    world.entity_mut(holder).insert(MessageLog {
        entries: vec![entry],
    });
}

/// Every message the log holds, oldest first.
#[must_use]
pub fn lines(world: &World) -> Vec<String> {
    let Some(mut query) = world.try_query_filtered::<&MessageLog, With<TheMessageLog>>() else {
        return Vec::new();
    };
    query.iter(world).next().map_or_else(Vec::new, |log| {
        log.entries.iter().map(|entry| entry.text.clone()).collect()
    })
}

/// Every message with its severity, oldest first.
#[must_use]
pub fn entries(world: &World) -> Vec<Message> {
    let Some(mut query) = world.try_query_filtered::<&MessageLog, With<TheMessageLog>>() else {
        return Vec::new();
    };
    query
        .iter(world)
        .next()
        .map_or_else(Vec::new, |log| log.entries.clone())
}

/// Empties the log, which is what the interface does once it has drawn a turn.
pub fn clear(world: &mut World) {
    let holder = singletons::singleton::<MessageLog, TheMessageLog>(world);
    if let Some(mut log) = world.get_mut::<MessageLog>(holder) {
        log.entries.clear();
    }
}

/// How many smileys one message may get from a character's high, as a function of how long they have been high.
///
/// Reference: `modify_msg_with_exclamations` (`messages.cpp:255-268`):
///
/// ```text
/// // Slowly rises. 0 at 0, 1 at 22.5, 2 at 45 mins. Asymptote at 3.
/// const float max_per_msg = 3 - 3 / ( 1 + 0.001 * in_minutes * in_minutes );
/// ```
///
/// **The reference's comment is the oracle**, and the curve is not the straight line a "a smiley every so often" would be: it rises slowly at first and then flattens, so a character who has been high
/// for a quarter of an hour sees about one smiley per message and never more than three however long it lasts.
#[must_use]
pub fn weed_smiley_cap(minutes: f64) -> f64 {
    /// The reference's own three-point ceiling: `max_per_msg` approaches this and never reaches it.
    const ASYMPTOTE: f64 = 3.0;
    /// The reference's `0.001`, which sets where the curve turns over.
    const CURVE: f64 = 0.001;
    ASYMPTOTE - ASYMPTOTE / CURVE.mul_add(minutes * minutes, 1.0)
}

/// Where a smiley may be inserted into a message: the end, and the spaces the reference allows.
///
/// Reference: `modify_msg_with_exclamations` (`messages.cpp:271-318`). The message's **end** is always a candidate. A space is a candidate unless it follows:
///
/// * **the first word** (`space_locations` is empty, so the first space is `wordskip`),
/// * one of the reference's six **forbidden words**, compared **with its leading space** - `" that"`, `" with"`, `" this"`, `" over"`, `" your"`, `" onto"`,
/// * a word of **one to four characters** (`word.size() > 1 && word.size() <= 4`; a one-character word cannot occur, because two spaces in a row are skipped first),
/// * or **another space** (`prev_space == j - 1`).
///
/// **The forbidden list carries its leading space because the reference slices from the previous space**, so its comparison is against `" that"` rather than `"that"`. A port that compared bare
/// words would allow a smiley after "that" and be wrong in the one case the list exists for.
///
/// The positions are **byte offsets before any insertion**, which is what the reference's own `smil_loc += 3` bookkeeping corrects for as it inserts.
#[must_use]
pub fn smiley_slots(message: &str) -> Vec<usize> {
    /// The reference's `forbidden_words`, each with the leading space its comparison uses.
    const FORBIDDEN: [&str; 6] = [" that", " with", " this", " over", " your", " onto"];
    let mut slots = vec![message.len()];
    let mut spaces: Vec<usize> = Vec::new();
    for (index, character) in message.char_indices() {
        if character != ' ' {
            continue;
        }
        let mut skip = spaces.is_empty();
        if let Some(previous) = spaces.last().copied() {
            if previous + 1 == index {
                skip = true;
            }
            let word = message.get(previous..index).unwrap_or("");
            if FORBIDDEN.contains(&word) {
                skip = true;
            }
            if word.len() > 1 && word.len() <= 4 {
                skip = true;
            }
        }
        spaces.push(index);
        if !skip {
            slots.push(index);
        }
    }
    slots
}

/// Writes a smiley at each of these positions, in the order given, keeping the later ones straight.
///
/// Reference: `modify_msg_with_exclamations` (`messages.cpp:318-334`): the smiley is `" :)"` - **three bytes, space included** - inserted at its position, and then
///
/// ```text
/// for( size_t &smil_loc : smiley_locations ) if( smil_loc > smiley_location ) smil_loc += 3;
/// ```
///
/// The reference's own comment is *"we increment to account for the insertion of the smiley string"*. **The positions the caller passes are the ones collected before any insertion**, so the shift is the
/// rule: applying the raw offsets in order would drift three bytes per smiley and silently mangle the message.
#[must_use]
pub fn apply_smileys(message: &str, positions: &[usize]) -> String {
    /// What one smiley adds, including its leading space - the reference's `" :)"`.
    const SMILEY_BYTES: usize = 3;
    let mut text = message.to_owned();
    for position in positions {
        let shift = positions
            .iter()
            .take_while(|earlier| *earlier != position)
            .filter(|earlier| *earlier < position)
            .count()
            * SMILEY_BYTES;
        let at = position + shift;
        if at <= text.len() {
            text.insert_str(at, " :)");
        }
    }
    text
}
