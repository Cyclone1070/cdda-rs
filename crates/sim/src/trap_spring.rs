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
    spring_with(world, who, square, true)
}

/// The same, and whether the creature **may dodge** it.
///
/// Reference: `map::creature_on_trap( Creature &c, bool may_avoid )` (`map.cpp:9881`), called with `true` for the player (`game.cpp:11361`) and `false` for monsters
/// (`game.cpp:12221`). A monster does not notice traps, so its call skips the avoidance branch entirely.
pub fn spring_with(
    world: &mut World,
    who: Entity,
    square: (i32, i32, i32),
    may_avoid: bool,
) -> bool {
    let Some(kind) = crate::square::trap_at(world, square) else {
        return false;
    };
    let Some(trap) = crate::trap_registry::get(world, &kind) else {
        return false;
    };
    if trap.action.as_deref() != Some("eocs") || trap.eocs.is_empty() {
        return false;
    }
    // **A benign trap is not a trap.** Reference: `map::maybe_trigger_trap` returns before doing anything for a `benign` trap (`map.cpp:9923-9926`) - a cot is a trap in the
    // data and nothing should happen to whoever lies on it.
    if trap.benign {
        return false;
    }
    // **Some traps only care about the player.** Reference: `map.cpp:9928-9930` skips the trigger when the trap carries `AVATAR_ONLY` and the creature is not the avatar.
    if trap.flags.contains("AVATAR_ONLY") && crate::missions::avatar(world) != Some(who) {
        return false;
    }
    // **A rider is above the trap.** Reference: `map::creature_on_trap` returns at once for a character `in_vehicle` or gliding (`map.cpp:9886-9888`) - the wheels, not the
    // passenger, meet the trap, and a vehicle's own collisions are a different rule.
    if world.get::<crate::vehicles::Riding>(who).is_some() {
        return false;
    }
    // **A trap that is noticed is stepped over.** Reference: `map.cpp:9932-9939` - the roll is skipped for `UNDODGEABLE` traps and for a creature that may not avoid, and a
    // trap that is dodged is announced and remembered unless it is always invisible.
    if !trap.flags.contains("UNDODGEABLE")
        && may_avoid
        && crate::trap_avoidance::avoid_trap(world, who, &trap, square)
    {
        if !trap.always_invisible && !crate::trap_avoidance::knows(world, who, square) {
            crate::messages::push(world, &format!("You've spotted a {}!", trap.name));
            crate::trap_avoidance::remember(world, who, square);
        }
        return false;
    }
    // **The trap says what happened**, in the words the data gave it. Reference: `map.cpp:9941-9944` - the line is shown when the trap is not `always_invisible` and the data
    // supplied **both** halves (`has_trigger_msg`, `trap.h:258-260`). The player hears `trigger_message_u`; an NPC or monster hears `trigger_message_npc` with `<npcname>`
    // replaced by its own name (`Creature::replace_with_npc_name`, `creature.cpp:3408-3412`).
    if !trap.always_invisible
        && let (Some(player), Some(other)) =
            (trap.trigger_u.as_deref(), trap.trigger_npc.as_deref())
    {
        let line = if crate::missions::avatar(world) == Some(who) {
            player.to_owned()
        } else {
            let name = world
                .get::<crate::creature::Creature>(who)
                .map_or("Someone", |creature| creature.name.as_str());
            other.replace("<npcname>", name)
        };
        crate::messages::push(world, &line);
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
