//! Sounds a creature made, and the fact that they last less than a turn.
//!
//! Reference: `sounds::sound` (`sounds.cpp:317-336`) puts an event in `sounds_since_last_turn`, which the turn loop drains -
//! so a sound is not a thing in the world, it is a thing that happened. The port keeps that shape: a sound is an entity
//! carrying [`Sound`], the turn that made it hears it, and [`clear`] takes the entity out again.
//!
//! Volume is the reference's own: a sound's volume at distance `d` is `volume - d`, so volume is a radius as much as a
//! loudness, and a spell that makes noise states the volume in the same units every other sound uses.

use bevy_ecs::prelude::*;

/// What kind of sound was made: the reference's `sounds::sound_t` (`sounds.h:23-38`).
///
/// The distinction is not decoration - [`is_provocative`] reads it, and the reference's own answer **surprises everyone who assumes it**: **`combat` is *not* provocative**, and neither is
/// `alarm`. Only `speech`, `electronic_speech`, `alert` and `order` are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoundCategory {
    /// The world going about its business.
    #[default]
    Background,
    /// Wind, rain, thunder.
    Weather,
    /// A sonar ping or another sound the player should be shown.
    Sensory,
    /// Music.
    Music,
    /// Something moving.
    Movement,
    /// Somebody talking.
    Speech,
    /// A robot talking, a radio.
    ElectronicSpeech,
    /// Somebody working.
    Activity,
    /// Somebody breaking something.
    DestructiveActivity,
    /// An alarm.
    Alarm,
    /// Violence: a gunshot, a blow.
    Combat,
    /// Loud enough to get attention.
    Alert,
    /// An order, which is the loudest kind of attention.
    Order,
}

/// Whether a sound makes a monster want to investigate: `is_provocative` (`sounds.cpp:287-309`).
///
/// Reference, verbatim: **`speech`, `electronic_speech`, `alert` and `order` return `true`; everything else returns `false`** - including `combat`, `alarm`, `destructive_activity`, `movement` and
/// the rest. **A gunshot does not provoke anything by this rule**, which is the single most counter-intuitive line in the file and the reason it is a function rather than a field: the answer is
/// the reference's, not a guess about what ought to be scary.
#[must_use]
pub fn is_provocative(category: SoundCategory) -> bool {
    matches!(
        category,
        SoundCategory::Speech | SoundCategory::ElectronicSpeech | SoundCategory::Alert | SoundCategory::Order
    )
}

/// Something that made a sound this turn.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Sound {
    /// Where it came from.
    pub position: (i32, i32, i32),
    /// How loud, in the reference's units - the number of squares over which it is still audible.
    pub volume: i32,
    /// What kind of sound it was, which decides whether a monster treats it as something worth looking into.
    pub category: SoundCategory,
    /// What it sounded like: `bang!`, `Thunk!`, `whizz!` - the reference's `sound_event::description`, which its own comment calls **"not an optional parameter"**
    /// (`sounds.cpp:326-329`). A QA run and a player both read this line, so a port that dropped it could not say what was heard.
    pub description: String,
}

/// Makes a sound, and answers the entity holding it.
///
/// Reference: `sounds::sound` refuses a negative volume and says nothing about it. The port refuses the same way: a volume
/// below zero is not a quiet sound, it is a mistake, and creating a silent sound would hide it.
pub fn make_sound(world: &mut World, position: (i32, i32, i32), volume: i32) -> Option<Entity> {
    make_sound_saying(world, position, volume, "")
}

/// Makes a sound that says something, and answers the entity holding it.
///
/// Reference: `sounds::sound( p, vol, category, description )` (`sounds.cpp:317-337`). The reference treats an **empty** description as a bug and complains, but still makes the sound -
/// so the port keeps the sound and the empty string, which is what a caller with nothing to say passes.
pub fn make_sound_saying(world: &mut World, position: (i32, i32, i32), volume: i32, description: &str) -> Option<Entity> {
    make_a_sound_of_kind(world, position, volume, description, SoundCategory::Background)
}

/// Makes a sound of a stated kind, which is what a caller that wants to be heard as combat, speech or an order says.
///
/// Reference: `sounds::sound( p, vol, category, description )` (`sounds.cpp:317-337`), whose third argument decides `is_provocative`.
pub fn make_a_sound_of_kind(
    world: &mut World,
    position: (i32, i32, i32),
    volume: i32,
    description: &str,
    category: SoundCategory,
) -> Option<Entity> {
    if volume < 0 {
        return None;
    }
    Some(
        world
            .spawn(Sound {
                position,
                volume,
                category,
                description: description.to_owned(),
            })
            .id(),
    )
}

/// Every sound made this turn, in the order the store holds them.
#[must_use]
pub fn heard_this_turn(world: &mut World) -> Vec<Sound> {
    let mut query = world.query::<&Sound>();
    query.iter(world).cloned().collect()
}

/// How loud a sound is at a distance, which is the reference's `volume - distance`, and never less than nothing.
#[must_use]
pub fn volume_at(sound: &Sound, from: (i32, i32, i32)) -> i32 {
    let distance = (sound.position.0 - from.0)
        .abs()
        .max((sound.position.1 - from.1).abs())
        .max((sound.position.2 - from.2).abs());
    (sound.volume - distance).max(0)
}

/// Takes every sound out of the store, which is what the end of a turn does.
pub fn clear(world: &mut World) {
    let sounds: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<Sound>>();
        query.iter(world).collect()
    };
    for sound in sounds {
        world.despawn(sound);
    }
}
