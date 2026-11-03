//! What a creature hears, and how far off the noise it thinks it is.
//!
//! Reference: `monster::hear_sound` (`src/monster.cpp:3925-3980`). A sound's volume falls off with distance, a creature with good hearing hears twice as loud, and the
//! louder the sound the less its guess wanders - which is the whole arithmetic of a monster walking towards a thrown rock. **What this module does not do is decide what
//! the creature then wants**: the reference calls `wander_to`, and a goal is a rule the port has not written.

use bevy_ecs::prelude::*;

/// What a creature heard, and where it thinks the noise came from.
///
/// Reference: `monster::wander_to( target, wander_turns )` - the reference keeps the destination and a turn count in the monster itself. The port keeps them in a component,
/// because a monster holding a memory is a monster with a piece of state, and state in this port is a component or it does not exist.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeardSound {
    /// The square it thinks the noise came from, which may be a few squares off.
    pub at: (i32, i32, i32),
    /// How loud it was, which is how much the guess could wander.
    pub volume: i32,
    /// How many turns it will keep walking towards that guess.
    pub wander_turns: i32,
}

/// How loud a sound is to a creature `distance` squares away, or nothing when it is out of earshot.
///
/// Reference: `monster::hear_sound` (`monster.cpp:3932-3937`): `volume = goodhearing ? 2 * vol - dist : vol - dist`, and a volume of zero or less is not heard at all.
/// The doubling is on the **volume**, not on the distance, which is why good hearing stretches past the noise rather than shortening the walk.
#[must_use]
pub fn heard_volume(volume: i32, distance: i32, good_hearing: bool) -> Option<i32> {
    let heard = if good_hearing {
        2 * volume - distance
    } else {
        volume - distance
    };
    if heard <= 0 { None } else { Some(heard) }
}

/// How far a creature's guess can be off, from how loud the sound was.
///
/// Reference: `monster::hear_sound` (`monster.cpp:3945-3955`): a band table, and **a loud sound is not a precise one - it is a *precise* one**: under 2 is ten squares
/// out, under 5 is five, under 10 is three, under 20 is one, and anything louder is guessed exactly. The table is written as a ladder of `<` tests rather than as a lookup,
/// and the port keeps that shape because the boundaries are the rule.
#[must_use]
pub fn max_error(volume: i32) -> i32 {
    match volume {
        i32::MIN..=1 => 10,
        2..=4 => 5,
        5..=9 => 3,
        10..=19 => 1,
        _ => 0,
    }
}

/// How long a creature keeps walking towards a noise.
///
/// Reference: `monster::hear_sound` (`monster.cpp:3958`): `wander_turns = volume * ( goodhearing ? 6 : 1 )`, so good hearing both hears further **and** remembers longer.
#[must_use]
pub fn wander_turns(volume: i32, good_hearing: bool) -> i32 {
    volume * if good_hearing { 6 } else { 1 }
}

/// Gives every creature that can hear the sound it just heard, and forgets the sounds.
///
/// Reference: `sounds::process_sound` (`sounds.cpp:186-260`) walks the sounds made this turn and calls `hear_sound` on every monster within earshot, then clears the list -
/// so a sound is heard once and by whoever was close enough. The port's version of "who can hear" is the `HEARS` flag the monster data carries: a creature with no flag at
/// all is deaf, which is what the data means by leaving it off.
///
/// The guess is the source jittered by up to [`max_error`] on each axis, drawn from the game's own dice; the walk lasts [`wander_turns`] and becomes a [`crate::creature::Wander`],
/// which is the port's `wander_to`. A creature that has a goal of its own keeps it: `monster::move` reads the wander only when there is no goal.
pub fn hear_sounds(world: &mut World) -> usize {
    let heard = crate::sounds::heard_this_turn(world);
    if heard.is_empty() {
        return 0;
    }
    let mut listeners: Vec<(Entity, (i32, i32, i32))> = Vec::new();
    {
        let mut query = world.query::<(Entity, &crate::Position)>();
        for (entity, place) in query.iter(world) {
            if crate::creature_flags::has_flag(world, entity, "HEARS") {
                listeners.push((entity, (place.x, place.y, place.z)));
            }
        }
    }
    let mut convinced = 0;
    for (entity, here) in listeners {
        let good_hearing = crate::creature_flags::has_flag(world, entity, "GOODHEARING");
        // The loudest thing this creature could hear is the one it walks towards, so the loudest answer wins.
        let mut loudest: Option<HeardSound> = None;
        for sound in &heard {
            let distance = (sound.position.0 - here.0)
                .abs()
                .max((sound.position.1 - here.1).abs())
                .max((sound.position.2 - here.2).abs());
            let Some(volume) = heard_volume(sound.volume, distance, good_hearing) else {
                continue;
            };
            if loudest.is_some_and(|known| known.volume >= volume) {
                continue;
            }
            let error = max_error(volume);
            let mut dice = crate::singletons::take_rolls(world);
            let wobble = dice.as_mut().map_or((0, 0), |rolls| {
                (rolls.rng(-error, error), rolls.rng(-error, error))
            });
            if let Some(rolls) = dice {
                crate::singletons::put_rolls(world, rolls);
            }
            loudest = Some(HeardSound {
                at: (
                    sound.position.0 + wobble.0,
                    sound.position.1 + wobble.1,
                    sound.position.2,
                ),
                volume,
                wander_turns: wander_turns(volume, good_hearing),
            });
        }
        if let Some(heard_at) = loudest {
            world.entity_mut(entity).insert(heard_at);
            // The guess is **also** where the creature now walks: `monster::hear_sound` ends in `wander_to`, which is what turns a noise into a monster arriving.
            crate::creature::set_wander(world, entity, heard_at.at, heard_at.wander_turns);
            convinced += 1;
        }
    }
    // A sound is over once it has been heard: the reference clears its list at the same point in the turn.
    crate::sounds::clear(world);
    convinced
}
