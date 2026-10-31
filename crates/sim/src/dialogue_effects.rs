//! What a dialogue effect does to the store.
//!
//! Reference: `npctalk_funcs.cpp`, one function per effect name, driven by `talk_effect_fun_t`. The port starts with the effect
//! the data uses most - `math`, 304 of the 1,612 effect entries in the game's effect-on-conditions - because the expression
//! parser and the variable store already exist and only the **assignment** half was missing.

use bevy_ecs::prelude::*;

use cdda_content::dialogue::{Assignment, AssignmentOp, Effect};

use crate::dialogue_values::{self, Value};

/// What came of running an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// It did what it does.
    Done,
    /// The store could not answer something the effect needed, so nothing was changed.
    CannotAnswer,
}

/// Runs a `math` statement against the speaker's variables.
///
/// Reference: the `math` effect sets a dialogue variable, and the reference's arithmetic on a variable that is not set treats it
/// as **zero** - `dialogue::get_value` answers a null value and the arithmetic reads it as nothing. The port does the same, but
/// only for the ops where that is meaningful: `x += 1` on an unset `x` is one, while `x = y` with an unanswerable `y` changes
/// nothing at all.
pub fn run_assignment(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    assignment: &Assignment,
) -> Applied {
    // An assignment reads an unset variable as zero - `dialogue::get_value` answers a null value and the arithmetic treats it
    // as nothing - so `"x = x + 20"` on an unset `x` writes 20 rather than refusing.
    // The dice come out of the store for the length of the evaluation and go back afterwards, so a `rng` inside a `math` statement
    // draws from the game's own generator and the sequence stays replayable.
    let mut dice = crate::singletons::take_rolls(world);
    let drawn = crate::conditions::value_drawing(
        world,
        speaker,
        listener,
        &assignment.expression,
        true,
        dice.as_mut(),
    );
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    let Some(value) = drawn else {
        return Applied::CannotAnswer;
    };
    // A destination naming a scope resolves the way the condition side resolves one: `global_val` is the shared map, and
    // everything else is the speaker's own values.
    let holder = match assignment.target.scope() {
        Some("global_val") => match dialogue_values::global_holder(world) {
            Some(global) => global,
            None => return Applied::CannotAnswer,
        },
        Some("n_val" | "npc_val") => match listener {
            Some(other) => other,
            None => return Applied::CannotAnswer,
        },
        _ => speaker,
    };
    let name = assignment.target.name();
    let previous = match assignment.op {
        AssignmentOp::Set => 0.0,
        _ => dialogue_values::number_of(world, holder, name).unwrap_or(0.0),
    };
    let result = match assignment.op {
        AssignmentOp::Set | AssignmentOp::Add => previous + value,
        AssignmentOp::Subtract => previous - value,
        AssignmentOp::Multiply => previous * value,
        AssignmentOp::Divide => {
            if value == 0.0 {
                return Applied::CannotAnswer;
            }
            previous / value
        }
    };
    dialogue_values::set(world, holder, name, Value::Number(result));
    Applied::Done
}

/// Runs every statement of a `math` effect, stopping at the first the store cannot answer.
///
/// Reference: a `math` entry holds a list of statements and `set_math` runs them in order. Stopping matters: a statement that
/// could not be worked out would otherwise leave the following ones running on a value nobody intended.
pub fn run_math(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    assignments: &[Assignment],
) -> Applied {
    for assignment in assignments {
        if run_assignment(world, speaker, listener, assignment) == Applied::CannotAnswer {
            return Applied::CannotAnswer;
        }
    }
    Applied::Done
}

/// Whose trait, effect or value an effect changes.
///
/// Reference: every `f_*` in `npctalk.cpp` takes an `is_npc` flag and asks `d.actor( is_npc )` - the listener for the `npc_`
/// spelling, the speaker for the `u_` one. One helper, so a rule cannot get the two the wrong way round in one place and right in
/// another.
fn actor(speaker: Entity, listener: Option<Entity>, about_listener: bool) -> Option<Entity> {
    if about_listener {
        return listener;
    }
    Some(speaker)
}


/// Stores the actor's square in a dialogue variable.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`), plain form: the value is `d.const_actor( is_npc )->pos_abs()`, and the scope names where
/// it goes the same way `math` names its target.
fn store_location(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    scope: &str,
    name: &str,
    about_listener: bool,
) -> Applied {
    let Some(creature) = actor(speaker, listener, about_listener) else {
        return Applied::CannotAnswer;
    };
    let Some(position) = world.get::<crate::Position>(creature).copied() else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position {
            x: position.x,
            y: position.y,
            z: position.z,
        },
    );
    Applied::Done
}

/// Runs the **last** case a switch's value reaches.
///
/// Reference: `f_switch` (`npctalk.cpp:6998-7020`): every case is tried in the data's order and each one the value reaches replaces the chosen
/// one, so the last match runs - not the first, and not the nearest.
fn run_switch(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    on: &cdda_content::dialogue::Expression,
    cases: &[(cdda_content::dialogue::Expression, Vec<Effect>)],
) -> Applied {

        let drawn = |world: &mut World, expression: &cdda_content::dialogue::Expression| {
            let mut dice = crate::singletons::take_rolls(world);
            let value = crate::conditions::value_drawing(
                world,
                speaker,
                listener,
                expression,
                true,
                dice.as_mut(),
            );
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            value
        };
        let Some(switched) = drawn(world, on) else {
            return Applied::CannotAnswer;
        };
        // Reference: every case the value reaches replaces the chosen one, so the **last** match runs.
        let mut chosen: Option<&Vec<Effect>> = None;
        for (case, effects) in cases {
            let Some(reached) = drawn(world, case) else {
                return Applied::CannotAnswer;
            };
            if switched >= reached {
                chosen = Some(effects);
            }
        }
        match chosen {
            Some(effects) => run_effects(world, speaker, listener, effects),
            None => Applied::Done,
        }
    }

/// Runs one effect entry.
pub fn run_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    match effect {
        Effect::Math(assignments) => run_math(world, speaker, listener, assignments),
        Effect::Message(text) => {
            crate::messages::push(world, text);
            Applied::Done
        }
        Effect::LocationVariable {
            scope,
            name,
            listener: about_listener,
        } => store_location(world, speaker, listener, scope, name, *about_listener),
        Effect::Switch { on, cases } => run_switch(world, speaker, listener, on, cases),
        Effect::If {
            condition,
            then,
            otherwise,
        } => {
            let branch = if crate::conditions::holds_in_store(world, speaker, listener, condition) {
                then
            } else {
                otherwise
            };
            run_effects(world, speaker, listener, branch)
        }
        Effect::AddEffect {
            effect,
            duration,
            intensity,
            listener: about_listener,
        } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            let permanent = duration.is_none();
            crate::applied_effects::add(
                world,
                creature,
                crate::effects::ActiveEffect {
                    id: effect.clone(),
                    intensity: *intensity,
                    duration: duration.unwrap_or(0),
                    permanent,
                },
            );
            Applied::Done
        }
        Effect::RemoveEffect { effect, listener: about_listener } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            crate::applied_effects::remove(world, creature, effect);
            Applied::Done
        }
        Effect::LearnTrait { trait_name, listener: about_listener } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            crate::traits::add_trait(world, creature, trait_name);
            Applied::Done
        }
        Effect::LoseTrait { trait_name, listener: about_listener } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            // Reference: `unset_mutation` on a trait the creature does not have does nothing, and a `lose_trait` that finds
            // nothing to lose has still done its job - so the answer is `Done` either way.
            crate::traits::remove_trait(world, creature, trait_name);
            Applied::Done
        }
        // A named program needs the registry and a depth, which the program runner (`eoc_registry::run_program`) supplies. Reaching
        // here means a list ran a program without one, which is a caller mistake, not a rule - so it refuses rather than guessing.
        Effect::RunEoc(_) => Applied::CannotAnswer,
    }
}

/// Runs a list of effects in order, stopping at the first the store cannot answer.
///
/// Reference: `talk_effect_t::apply` walks its list and runs each; the port stops early on a refusal for the same reason it
/// stops inside a `math` list - a later effect that assumed an earlier one's work would otherwise run on nothing.
pub fn run_effects(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effects: &[Effect],
) -> Applied {
    for effect in effects {
        if run_effect(world, speaker, listener, effect) == Applied::CannotAnswer {
            return Applied::CannotAnswer;
        }
    }
    Applied::Done
}
