//! The effect-on-conditions the data defines, by id, and how to run one.
//!
//! Reference: `effect_on_conditions::load` fills `effect_on_conditions::eoc_map`, and `dialogue::effect`'s `run_eocs` looks a
//! name up there and runs it (`npctalk.cpp`). The port keeps the same shape - a singleton holding programs by id - so an effect
//! can name another one and the machine composes the way the data expects.
//!
//! Recursion is bounded on purpose. The data nests programs freely (`EOC_A` runs `EOC_B`), and a loop in that graph would hang a
//! headless run with no window to interrupt, so a program may only run [`DEPTH_LIMIT`] deep. The reference has no such guard;
//! the port needs one because it cannot be Ctrl-C'd out of a turn loop in a test.

use bevy_ecs::prelude::*;
use cdda_content::dialogue::Program;
use std::collections::BTreeMap;

use crate::singletons;
use crate::dialogue_effects::Applied;

/// How deep a program may run another program before the port refuses.
pub const DEPTH_LIMIT: usize = 16;

/// Every program, by id.
#[derive(Component, Debug, Clone, Default)]
pub struct Programs {
    by_id: BTreeMap<String, Program>,
}

/// Marks the entity holding the programs.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ThePrograms;

/// Installs the programs into the store, replacing whatever was there.
pub fn set_programs(world: &mut World, programs: &[Program]) {
    let mut by_id = BTreeMap::new();
    for program in programs {
        by_id.insert(program.id.clone(), program.clone());
    }
    let holder = singletons::singleton::<Programs, ThePrograms>(world);
    if let Some(mut existing) = world.get_mut::<Programs>(holder) {
        existing.by_id = by_id;
        return;
    }
    world.entity_mut(holder).insert(Programs { by_id });
}

/// A program by id, or nothing when the data does not define it.
#[must_use]
pub fn program_of(world: &World, id: &str) -> Option<Program> {
    let mut query = world.try_query_filtered::<&Programs, With<ThePrograms>>()?;
    query.iter(world).next()?.by_id.get(id).cloned()
}

/// How many programs the store holds.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let holder = singletons::singleton::<Programs, ThePrograms>(world);
    world.get::<Programs>(holder).map_or(0, |programs| programs.by_id.len())
}

/// Runs a program by id, and the programs it names, up to [`DEPTH_LIMIT`].
///
/// Reference: `dialogue::effect` runs a named effect-on-condition with the same dialoguers; the port hands the same speaker and
/// listener down, so a program's `u_val` and `n_val` mean the same thing at every depth.
pub fn run_program(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    id: &str,
    depth: usize,
) -> Applied {
    if depth >= DEPTH_LIMIT {
        return Applied::CannotAnswer;
    }
    let Some(program) = program_of(world, id) else {
        return Applied::CannotAnswer;
    };
    if let Some(condition) = &program.condition
        && !crate::conditions::holds_with(world, speaker, listener, condition)
    {
        return Applied::Done;
    }
    for effect in &program.effects {
        match effect {
            cdda_content::dialogue::Effect::RunEoc(programs) => {
                for program in programs {
                    let started = match program {
                        cdda_content::dialogue::ProgramRef::Named(name) => {
                            run_program(world, speaker, listener, name, depth + 1)
                        }
                        cdda_content::dialogue::ProgramRef::Inline(inline) => {
                            run_inline(world, speaker, listener, inline, depth + 1)
                        }
                    };
                    if started == Applied::CannotAnswer {
                        return Applied::CannotAnswer;
                    }
                }
            }
            other => {
                if crate::dialogue_effects::run_effect(world, speaker, listener, other)
                    == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
        }
    }
    Applied::Done
}

/// Runs a program written where it is used.
///
/// Reference: `load_inline_eoc` (`npctalk.cpp:512-541`), which turns an inline object into an effect-on-condition with a derived id and
/// runs it in place. The port runs it without installing it, because an inline program has no id anything else can name.
pub fn run_inline(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    program: &Program,
    depth: usize,
) -> Applied {
    if depth >= DEPTH_LIMIT {
        return Applied::CannotAnswer;
    }
    if let Some(condition) = &program.condition
        && !crate::conditions::holds_with(world, speaker, listener, condition)
    {
        return Applied::Done;
    }
    for effect in &program.effects {
        match effect {
            cdda_content::dialogue::Effect::RunEoc(programs) => {
                for named in programs {
                    let started = match named {
                        cdda_content::dialogue::ProgramRef::Named(name) => {
                            run_program(world, speaker, listener, name, depth + 1)
                        }
                        cdda_content::dialogue::ProgramRef::Inline(inline) => {
                            run_inline(world, speaker, listener, inline, depth + 1)
                        }
                    };
                    if started == Applied::CannotAnswer {
                        return Applied::CannotAnswer;
                    }
                }
            }
            other => {
                if crate::dialogue_effects::run_effect(world, speaker, listener, other)
                    == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
        }
    }
    Applied::Done
}
