//! What a dialogue effect does to the store.
//!
//! Reference: `npctalk_funcs.cpp`, one function per effect name, driven by `talk_effect_fun_t`. The port starts with the effect
//! the data uses most - `math`, 304 of the 1,612 effect entries in the game's effect-on-conditions - because the expression
//! parser and the variable store already exist and only the **assignment** half was missing.

use bevy_ecs::prelude::*;

use cdda_content::dialogue::{Assignment, AssignmentOp};

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
    let Some(value) = crate::conditions::value(world, speaker, listener, &assignment.expression)
    else {
        return Applied::CannotAnswer;
    };
    let previous = match assignment.op {
        AssignmentOp::Set => 0.0,
        _ => dialogue_values::number_of(world, speaker, &assignment.name).unwrap_or(0.0),
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
    dialogue_values::set(world, speaker, &assignment.name, Value::Number(result));
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
