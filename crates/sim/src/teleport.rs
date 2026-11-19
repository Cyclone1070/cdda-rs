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
pub fn creature(
    world: &mut World,
    who: Entity,
    min_distance: i32,
    max_distance: i32,
    safe: bool,
) -> Teleport {
    let Some(origin) = world
        .get::<crate::Position>(who)
        .map(|at| (at.x, at.y, at.z))
    else {
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
    to_point_with(world, who, target, safe, false, false)
}

/// The same rule with the reference's `force` at hand.
///
/// Reference: `teleport_to_point` takes `force` and `force_safe` (`teleport.cpp:172-180`). They differ in **one** place and agree in two: both bypass the anchor guard
/// (`if( !force && !force_safe && ... )`, `:188-201`), both turn a solid landing into the nearest open square rather than a refusal (`:214-225`), and only `force_safe` also
/// refuses to land on a square somebody is already standing on (`:252-270`). The dialogue's `u_teleport` is where the flags come from.
pub fn to_point_with(
    world: &mut World,
    who: Entity,
    target: (i32, i32, i32),
    safe: bool,
    force: bool,
    force_safe: bool,
) -> Teleport {
    let Some(origin) = world
        .get::<crate::Position>(who)
        .map(|at| (at.x, at.y, at.z))
    else {
        return Teleport::Unchanged;
    };
    if origin == target {
        return Teleport::Unchanged;
    }
    // Reference: a target above `OVERMAP_HEIGHT` or below `-OVERMAP_DEPTH` is refused before any landing rule (`teleport.cpp:181-186`). The port asks the same bounds the
    // falling rules use, so the world has one height rather than two.
    if target.2 > crate::falling::OVERMAP_HEIGHT || target.2 < -crate::falling::OVERMAP_DEPTH {
        return Teleport::Refused;
    }
    // **A locked creature does not move.** Reference: `teleport.cpp:191-201` refuses a teleportee wearing an item or carrying an effect with `DIMENSIONAL_ANCHOR`, or an effect
    // with `TELEPORT_LOCK`. Three sources, one bypass: all three are tested here, and the bypass (`force`/`force_safe`) belongs to the caller that has one, which this rule does not.
    if !force
        && !force_safe
        && (crate::creature::wears_flag(world, who, "DIMENSIONAL_ANCHOR")
            || crate::applied_effects::has_flag(world, who, "DIMENSIONAL_ANCHOR")
            || crate::applied_effects::has_flag(world, who, "TELEPORT_LOCK"))
    {
        return Teleport::Refused;
    }
    // **A forced jump does not land in the wall, it lands beside it.** Reference: `if( force || force_safe )` walks `closest_points_first( target, 5 )` and takes the first
    // square that is `passable_through` (`teleport.cpp:214-225`); only an unforced, unsafe jump into a solid square kills (`:226-241`).
    let landing = if standable(world, target) {
        target
    } else if force || force_safe {
        forced_landing(world, target).unwrap_or(target)
    } else if safe {
        return Teleport::Refused;
    } else {
        crate::creature::hurt(world, who, 9_999.0);
        return Teleport::Died;
    };
    // **A `force_safe` jump lands where nobody stands.** Reference: `teleport.cpp:252-270` - with the flag set and a creature already at the target, the reference walks outward
    // to the nearest square that is passable **and empty**. Without the flag it telefrags, which the port has not written and names in `docs/parity.md`.
    let landing = if force_safe && creature_there(world, landing) {
        free_landing(world, landing).unwrap_or(landing)
    } else {
        landing
    };
    // **A rider is put down first.** Reference: `teleport.cpp:203-206` unboards a character in a vehicle before the landing rules run, so the teleportee is not left sitting in a
    // seat that stayed behind. The port's `unboard` is the same link the vehicle system reads, so a teleported driver stops driving without a second flag to keep in step.
    crate::vehicles::unboard(world, who);
    if let Some(mut place) = world.get_mut::<crate::Position>(who) {
        place.x = landing.0;
        place.y = landing.1;
        place.z = landing.2;
    }
    // A moved thing that kept its old index entry would stand on two squares, which is the ghost the architecture gate exists for.
    crate::PositionIndex::rebuild(world);
    Teleport::Moved {
        from: origin,
        to: landing,
    }
}

/// The square a forced teleport lands on when the target itself cannot be stood on.
///
/// Reference: `teleport::teleport_to_point` (`teleport.cpp:215-223`) - for a `force` or `force_safe` teleport into a solid square, the reference walks
/// `closest_points_first( target, 5 )`, drops the target itself, and takes the first square that is `passable_through`. When none of the forty-eight squares within five is
/// passable it keeps the solid target, which is why this answers `None` rather than inventing somewhere further away.
///
/// The port searches the same rings in the same order - by distance, then by x, then by y - and asks the port's own standability rule, so a forced landing and a walked step
/// agree about which squares are open.
#[must_use]
pub fn forced_landing(world: &mut World, target: (i32, i32, i32)) -> Option<(i32, i32, i32)> {
    for distance in 1..=5_i32 {
        for dy in -distance..=distance {
            for dx in -distance..=distance {
                if dx.abs().max(dy.abs()) != distance {
                    continue;
                }
                let square = (target.0 + dx, target.1 + dy, target.2);
                if standable(world, square) {
                    return Some(square);
                }
            }
        }
    }
    None
}

/// Whether a creature already stands on a square.
fn creature_there(world: &World, square: (i32, i32, i32)) -> bool {
    crate::singletons::index(world)
        .map_or(&[][..], |index| {
            index.at_level(square.0, square.1, square.2)
        })
        .iter()
        .any(|entity| world.get::<crate::creature::Creature>(*entity).is_some())
}

/// **The nearest square that is both passable and empty**, for a `force_safe` landing.
///
/// Reference: `teleport.cpp:252-270` walks `closest_points_first( abs_ms, 5 )`, drops the target itself, and takes the first square that is passable - or out of bounds, which the
/// reference treats as free - and holds no creature. The port searches the same rings through [`forced_landing`] and then asks the occupancy question, which is the same
/// two-part test with the two halves in the reference's own order.
#[must_use]
pub fn free_landing(world: &mut World, target: (i32, i32, i32)) -> Option<(i32, i32, i32)> {
    for distance in 1..=5_i32 {
        for dy in -distance..=distance {
            for dx in -distance..=distance {
                if dx.abs().max(dy.abs()) != distance {
                    continue;
                }
                let square = (target.0 + dx, target.1 + dy, target.2);
                if standable(world, square) && !creature_there(world, square) {
                    return Some(square);
                }
            }
        }
    }
    None
}
