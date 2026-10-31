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
