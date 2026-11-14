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
    /// Whether the sound is something worth investigating: its **category** or its **loudness**, whichever says so (`monster.cpp:3939`).
    pub provocative: bool,
}

/// Whether a creature can hear at all: `monster::can_hear` (`monster.cpp:1318-1321`), one line and two conditions.
///
/// Reference: `return has_flag( mon_flag_HEARS ) && !has_effect( effect_deaf );` - **a deaf monster is still a monster with ears**, so the flag alone is not the answer. The port had the flag
/// and not the effect, which meant anything that deafened a monster changed nothing about what it heard.
#[must_use]
pub fn can_hear(world: &World, creature: Entity) -> bool {
    crate::creature_flags::has_flag(world, creature, "HEARS")
        && !crate::applied_effects::has(world, creature, "deaf")
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
            if can_hear(world, entity) {
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
            // Reference: `provocative || volume >= normal_roll( 30, 5 )` (`monster.cpp:3939`) - the loudness half is drawn here, once per sound heard.
            let mut dice = crate::singletons::take_rolls(world);
            let provocative = dice.as_mut().is_some_and(|rolls| {
                crate::sounds::heard_as_provocative(sound.category, volume, rolls)
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
                provocative,
            });
        }
        if let Some(heard_at) = loudest {
            // Reference: `monster::hear_sound`'s two guards (`monster.cpp:3940-3944` and `:3966-3970`), which are what stop a monster on the trail of something interesting from being
            // distracted by something duller: **a provocative sound always wins over a dull one that is already being followed**, and **a sound that would not hold it as long is ignored**.
            let walking = world.get::<crate::creature::Wander>(entity).copied();
            let following = walking.map_or(0, |wander| wander.turns_left);
            let already_provoked = walking.is_some_and(|wander| wander.provocative);
            if already_provoked && !heard_at.provocative && following > 0 {
                continue;
            }
            if heard_at.wander_turns < following {
                continue;
            }
            if let Some(mut wander) = world.get_mut::<crate::creature::Wander>(entity) {
                wander.provocative = heard_at.provocative;
            }
            world.entity_mut(entity).insert(heard_at);
            // Reference: `monster::hear_sound`'s tail (`monster.cpp:3969-3981`), which is **three** outcomes and not one:
            //
            // | Morale | Anger | What it does |
            // |---|---|---|
            // | `>= 0` | `>= 10` | walks **towards** the noise |
            // | `< 0` | anything | walks to the **mirror point** - `pos + ( pos - target )`, the same distance the other side - because "monsters afraid of sound should not go towards sound" |
            // | `>= 0` | `< 10` | walks nowhere at all |
            //
            // **A frightened monster flees the noise and an uninterested one ignores it**, and a port that always walked towards the sound would have both wrong in the same line.
            let morale = world
                .get::<crate::creature::Morale>(entity)
                .map_or(0, |morale| morale.0);
            let anger = world
                .get::<crate::creature::Anger>(entity)
                .map_or(0, |anger| anger.0);
            let destination = if morale < 0 {
                let here_at =
                    world
                        .get::<crate::Position>(entity)
                        .copied()
                        .unwrap_or(crate::Position {
                            x: here.0,
                            y: here.1,
                            z: here.2,
                        });
                (
                    here_at.x + (here_at.x - heard_at.at.0),
                    here_at.y + (here_at.y - heard_at.at.1),
                    here_at.z,
                )
            } else if anger < 10 {
                continue;
            } else {
                heard_at.at
            };
            // The guess is **also** where the creature now walks: `monster::hear_sound` ends in `wander_to`, which is what turns a noise into a monster arriving.
            crate::creature::set_wander(world, entity, destination, heard_at.wander_turns);
            convinced += 1;
        }
    }
    // A sound is over once it has been heard: the reference clears its list at the same point in the turn.
    crate::sounds::clear(world);
    convinced
}
