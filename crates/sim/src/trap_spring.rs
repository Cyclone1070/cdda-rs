//! **What happens when a creature steps on a trap.**
//!
//! Reference: `trap::trigger` (`trap.cpp`) reached from `map::creature_on_trap`, and the `eocs` action in particular (`trapfunc::eocs`, `trapfunc.cpp:382-400`). That one is
//! small and is the shape this module takes: build a dialogue with the creature as the speaker, hand it the trap's own square in the invocation's context under the name
//! **`trap_location`**, run each of the trap's programs, and **remove the trap**.
//!
//! Two details are the rule rather than incidentals:
//!
//! - the square goes into the **context** (`write_var_value( var_type::context, "trap_location", ... )`), which is why the telepad's own program adjusts and teleports the
//!   value the trap handed it;
//! - the trap is removed **whether or not** the programs did anything, because a sprung trap is gone - and the removal is the last thing the reference does.
//!
//! Only traps whose action is `eocs` are sprung here. The named actions (`bubble`, `tripwire`, the eighty-odd in `trapfunc.cpp`) are their own rules and are not ported;
//! a trap with one is left alone rather than half-triggered, and `docs/parity.md` names it.

use bevy_ecs::prelude::*;

/// Springs the trap under a creature, and answers whether there was one to spring.
///
/// Reference: `trapfunc::eocs` (`trapfunc.cpp:382-400`) called from `trap::trigger`. Nothing happens when the square carries no trap, when the trap's action is not `eocs`,
/// or when the data names no programs - in each case the trap stays where it is, because a trap the port cannot act on is better left set than silently removed.
pub fn spring(world: &mut World, who: Entity, square: (i32, i32, i32)) -> bool {
    let Some(kind) = crate::square::trap_at(world, square) else {
        return false;
    };
    let Some(trap) = crate::trap_registry::get(world, &kind) else {
        return false;
    };
    if trap.action.as_deref() != Some("eocs") || trap.eocs.is_empty() {
        return false;
    }
    // Reference: `dialogue d( get_talker_for( critter ), nullptr ); write_var_value( var_type::context, "trap_location", &d, trap_location );`
    let previous = crate::dialogue_context::values(world, who);
    let mut context = previous.clone();
    context.insert(
        "trap_location".to_owned(),
        format!("{},{},{}", square.0, square.1, square.2),
    );
    crate::dialogue_context::put(world, who, context);
    for program in &trap.eocs {
        crate::eoc_registry::run_program(world, who, None, program, 0);
    }
    crate::dialogue_context::restore(world, who, Some(previous));
    // **The trap is gone**, which the reference does after the programs whatever they did.
    crate::square::remove_trap(world, square);
    true
}
