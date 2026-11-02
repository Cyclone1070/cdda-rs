//! Whether a creature can see.
//!
//! Reference: `talker_character_const::can_see` (`talker_character.cpp:917`):
//!
//! ```text
//! return !is_blind() && ( !in_sleep_state() || has_flag( json_flag_SEESLEEP ) );
//! ```
//!
//! The port answers the part it can express. `in_sleep_state` (`creature.cpp:2221`) is any of three effects - sleep, lying down, or an NPC suspended -
//! and every one of those is a component the creature carries, so it is answered exactly. `is_blind` is *not*: the reference asks for a worn item's
//! flag, a character flag and a limb score, and the port has none of those systems yet. It is therefore answered **false**, which makes the port say a
//! creature can see in the cases where the reference would say it cannot - the divergence is recorded in `docs/parity.md` rather than hidden here.

use bevy_ecs::prelude::*;

/// The effects that leave a creature unable to watch: the reference's `in_sleep_state`.
pub const ASLEEP: [&str; 3] = ["sleep", "lying_down", "npc_suspend"];

/// Whether a creature can see.
#[must_use]
pub fn can_see(world: &World, creature: Entity) -> bool {
    !ASLEEP
        .iter()
        .any(|effect| crate::applied_effects::has(world, creature, effect))
}

/// Whether a creature can see a square, casting a line through the terrain.
///
/// Reference: `talker_character_const::can_see_location` (`talker_character.cpp:923-928`) is `Creature::sees( here, pos )`, which narrows the range with the
/// **ambient light at the target** (`creature.cpp:636-676`) and then asks `map::sees`. The port has the cast ([`crate::sees`]) and no per-square ambient light yet,
/// so it casts at `MAX_VIEW_DISTANCE`: in the dark it sees too far, and it never sees through a wall. When a light cache lands, this is the line that narrows.
#[must_use]
pub fn can_see_location(world: &World, creature: Entity, target: (i32, i32)) -> bool {
    if !can_see(world, creature) {
        return false;
    }
    let Some(at) = world.get::<crate::Position>(creature) else {
        return false;
    };
    crate::sees(
        world,
        (at.x, at.y),
        target,
        cdda_core_types::light::MAX_VIEW_DISTANCE,
    )
}
