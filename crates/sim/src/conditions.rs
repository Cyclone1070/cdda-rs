//! Whether a dialogue condition holds for a creature.
//!
//! Reference: the callable at the heart of `conditional_t` (`condition.cpp:2660-2760`), which answers a question about one
//! dialoguer and one talker. The port answers from the store: what a creature is wearing, what it has, where it is.

use bevy_ecs::prelude::*;

use cdda_content::dialogue::{Condition, Expression, Operator};

use crate::applied_effects;

/// Whether a condition holds for a creature.
///
/// Reference: `f_has_effect` asks the creature for the effect and compares intensity - `intensity.evaluate( d ) <=
/// target.get_intensity()`, so an intensity of zero means "any", and a missing effect is false rather than an error.
#[must_use]
pub fn holds(world: &World, creature: Entity, condition: &Condition) -> bool {
    match condition {
        Condition::HasEffect { effect, intensity } => {
            applied_effects::intensity_of(world, creature, effect)
                .is_some_and(|worn| *intensity <= worn)
        }
        Condition::And(conditions) => conditions
            .iter()
            .all(|condition| holds(world, creature, condition)),
        Condition::Or(conditions) => conditions
            .iter()
            .any(|condition| holds(world, creature, condition)),
        Condition::Not(inner) => !holds(world, creature, inner),
        // Reference: a `math` condition holds when its expression is non-zero, and an expression the store cannot answer
        // holds no more than a zero one does.
        Condition::Math(expression) => {
            matches!(value(world, creature, expression), Some(result) if result != 0.0)
        }
    }
}

/// What an expression works out to, or nothing when something in it cannot be answered.
///
/// Nothing propagates through arithmetic rather than turning into zero: an expression whose variable nobody set has no
/// value, and the reference's null `diag_value` is the same refusal.
#[must_use]
pub fn value(world: &World, creature: Entity, expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Number(number) => Some(*number),
        Expression::Variable { scope, name } => {
            let holder = if scope == "global_val" {
                crate::dialogue_values::global_holder(world)?
            } else {
                creature
            };
            crate::dialogue_values::number_of(world, holder, name)
        }
        Expression::Binary { op, left, right } => {
            let left = value(world, creature, left)?;
            let right = value(world, creature, right)?;
            Some(match op {
                Operator::Add => left + right,
                Operator::Subtract => left - right,
                Operator::Multiply => left * right,
                Operator::Divide => left / right,
                Operator::Remainder => left % right,
                Operator::Equal => f64::from(u8::from((left - right).abs() < f64::EPSILON)),
                Operator::NotEqual => f64::from(u8::from((left - right).abs() >= f64::EPSILON)),
                Operator::Less => f64::from(u8::from(left < right)),
                Operator::AtMost => f64::from(u8::from(left <= right)),
                Operator::Greater => f64::from(u8::from(left > right)),
                Operator::AtLeast => f64::from(u8::from(left >= right)),
            })
        }
    }
}
