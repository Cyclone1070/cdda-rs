//! Where a teleport puts a creature, and when it refuses to.
//!
//! Reference: `teleport::teleport_creature` and `teleport::teleport_to_point` (`teleport.cpp:152-260`). The rule is two steps and they fail differently: the first **draws**
//! a square - an angle and a distance, up to twenty times, accepting the last draw whether or not it landed on something - and the second decides what happens when the
//! creature arrives somewhere solid. A safe teleport refuses; an unsafe one is death, not damage the creature might survive.
//!
//! The draws come from the store's own generator, as every other draw in the port does, so a seed replays the same jump.

use bevy_ecs::prelude::*;

/// How many times the reference redraws a landing square before it accepts the last one.
///
/// Reference: `while( here.impassable( new_pos ) && tries < 20 )` (`teleport.cpp:164-168`).
pub const TRIES: i32 = 20;

/// What came of a teleport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Teleport {
    /// The creature is somewhere else now.
    Moved {
        /// Where it was.
        from: (i32, i32, i32),
        /// Where it is.
        to: (i32, i32, i32),
    },
    /// The landing square was solid and the teleport was safe, so nothing happened.
    Refused,
    /// The landing square was solid, the teleport was not safe, and the creature died there.
    Died,
    /// The creature was already standing where the teleport would have put it.
    Unchanged,
}

/// Whether a creature can stand on a square at all.
///
/// The port's terrain table answers this through the same route the search uses (`routing::square_cost`), so a square a route refuses is a square a teleport refuses.
fn standable(world: &mut World, square: (i32, i32, i32)) -> bool {
    crate::routing::square_cost(world, square).is_some()
}

/// Draws a landing square `min_distance` to `max_distance` away and puts the creature there.
///
/// Reference: `teleport::teleport_creature` (`teleport.cpp:152-170`): twelve-ish lines that draw an angle in **degrees as radians** - `std::cos( rangle )` on a value from
/// `rng( 0, 360 )`, which is the reference's own arithmetic and not a mistake to tidy - and a distance, and keep the last draw if twenty of them land on something solid.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn creature(world: &mut World, who: Entity, min_distance: i32, max_distance: i32, safe: bool) -> Teleport {
    let Some(origin) = world.get::<crate::Position>(who).map(|at| (at.x, at.y, at.z)) else {
        return Teleport::Unchanged;
    };
    // Reference: the arguments are checked before a single draw (`teleport.cpp:154-157`), and a rule that has already refused does not touch the dice.
    if min_distance > max_distance || min_distance < 0 || max_distance < 0 {
        return Teleport::Refused;
    }
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return Teleport::Refused;
    };
    let mut landing = origin;
    let mut tries = 0;
    loop {
        let angle = rolls.rng(0, 360);
        let distance = rolls.rng(min_distance, max_distance);
        #[allow(clippy::cast_precision_loss)]
        let radians = f64::from(angle);
        landing = (
            origin.0 + (f64::from(distance) * radians.cos()) as i32,
            origin.1 + (f64::from(distance) * radians.sin()) as i32,
            landing.2,
        );
        tries += 1;
        if !standable_refused(world, landing) || tries >= TRIES {
            break;
        }
    }
    crate::singletons::put_rolls(world, rolls);
    to_point(world, who, landing, safe)
}

/// Whether the reference would redraw: its loop asks `impassable`, so a standable square ends it.
fn standable_refused(world: &mut World, square: (i32, i32, i32)) -> bool {
    !standable(world, square)
}

/// Puts the creature on a square, or says what happened instead.
///
/// Reference: `teleport::teleport_to_point` (`teleport.cpp:172-260`). Three answers matter and the port takes all three: the same square is no teleport at all; a solid
/// landing **refuses** when the teleport is safe (`"You cannot teleport safely."`); and a solid landing without safety kills the creature outright - `apply_damage` of
/// 9,999 to the torso, which is the reference's way of saying "there is no body left to find".
pub fn to_point(world: &mut World, who: Entity, target: (i32, i32, i32), safe: bool) -> Teleport {
    let Some(origin) = world.get::<crate::Position>(who).map(|at| (at.x, at.y, at.z)) else {
        return Teleport::Unchanged;
    };
    if origin == target {
        return Teleport::Unchanged;
    }
    if !standable(world, target) {
        if safe {
            return Teleport::Refused;
        }
        crate::creature::hurt(world, who, 9_999.0);
        return Teleport::Died;
    }
    if let Some(mut place) = world.get_mut::<crate::Position>(who) {
        place.x = target.0;
        place.y = target.1;
        place.z = target.2;
    }
    // A moved thing that kept its old index entry would stand on two squares, which is the ghost the architecture gate exists for.
    crate::PositionIndex::rebuild(world);
    Teleport::Moved { from: origin, to: target }
}
