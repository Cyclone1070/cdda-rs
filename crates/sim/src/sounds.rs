//! Sounds a creature made, and the fact that they last less than a turn.
//!
//! Reference: `sounds::sound` (`sounds.cpp:317-336`) puts an event in `sounds_since_last_turn`, which the turn loop drains -
//! so a sound is not a thing in the world, it is a thing that happened. The port keeps that shape: a sound is an entity
//! carrying [`Sound`], the turn that made it hears it, and [`clear`] takes the entity out again.
//!
//! Volume is the reference's own: a sound's volume at distance `d` is `volume - d`, so volume is a radius as much as a
//! loudness, and a spell that makes noise states the volume in the same units every other sound uses.

use bevy_ecs::prelude::*;

/// Something that made a sound this turn.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Sound {
    /// Where it came from.
    pub position: (i32, i32, i32),
    /// How loud, in the reference's units - the number of squares over which it is still audible.
    pub volume: i32,
}

/// Makes a sound, and answers the entity holding it.
///
/// Reference: `sounds::sound` refuses a negative volume and says nothing about it. The port refuses the same way: a volume
/// below zero is not a quiet sound, it is a mistake, and creating a silent sound would hide it.
pub fn make_sound(world: &mut World, position: (i32, i32, i32), volume: i32) -> Option<Entity> {
    if volume < 0 {
        return None;
    }
    Some(world.spawn(Sound { position, volume }).id())
}

/// Every sound made this turn, in the order the store holds them.
#[must_use]
pub fn heard_this_turn(world: &mut World) -> Vec<Sound> {
    let mut query = world.query::<&Sound>();
    query.iter(world).copied().collect()
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
