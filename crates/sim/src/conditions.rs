//! Whether a dialogue condition holds for a creature.
//!
//! Reference: the callable at the heart of `conditional_t` (`condition.cpp:2660-2760`), which answers a question about one
//! dialoguer and one talker. The port answers from the store: what a creature is wearing, what it has, where it is.

use bevy_ecs::prelude::*;

use cdda_content::dialogue::{Condition, Expression, Operator, StringSource};

use crate::applied_effects;

/// Whether a condition holds for a creature talking to nobody in particular.
///
/// Reference: a `const_dialogue` always has two actors - the player (`u`) and an NPC (`n`) - and many conditions can be asked
/// of either. A question about the NPC asked with no NPC present is unanswerable, which is false rather than zero.
#[must_use]
pub fn holds(world: &World, creature: Entity, condition: &Condition) -> bool {
    holds_with(world, creature, None, condition)
}

/// Whether a condition holds, in a conversation with someone.
///
/// The two dialoguers are the reference's `actor( false )` and `actor( true )`: `u_val` is the speaker's, `n_val` and
/// `npc_val` are the listener's.
#[must_use]
pub fn holds_with(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    condition: &Condition,
) -> bool {
    match condition {
        Condition::HasEffect { effect, intensity } => {
            applied_effects::intensity_of(world, speaker, effect)
                .is_some_and(|worn| *intensity <= worn)
        }
        Condition::And(conditions) => conditions
            .iter()
            .all(|condition| holds_with(world, speaker, listener, condition)),
        Condition::Or(conditions) => conditions
            .iter()
            .any(|condition| holds_with(world, speaker, listener, condition)),
        Condition::Not(inner) => !holds_with(world, speaker, listener, inner),
        Condition::HasItems {
            item,
            count,
            charges,
        } => {
            let Some(name) = text_of(world, speaker, listener, item) else {
                return false;
            };
            let (stacks, total_charges) = crate::inventory::count_of(world, speaker, &name);
            let enough_charges = charges.is_none_or(|wanted| total_charges >= wanted);
            let enough_count = count.is_none_or(|wanted| stacks >= wanted);
            enough_charges && enough_count
        }
        Condition::CompareString(sources) => {
            let mut seen: Vec<String> = Vec::with_capacity(sources.len());
            for source in sources {
                let Some(text) = text_of(world, speaker, listener, source) else {
                    return false;
                };
                if seen.contains(&text) {
                    return true;
                }
                seen.push(text);
            }
            false
        }
        Condition::Math(expression) => matches!(
            value(world, speaker, listener, expression),
            Some(result) if result != 0.0
        ),
    }
}

/// What an expression works out to, or nothing when something in it cannot be answered.
///
/// Nothing propagates through arithmetic rather than turning into zero: an expression whose variable nobody set has no
/// value, and the reference's null `diag_value` is the same refusal.
#[must_use]
pub fn value(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    expression: &Expression,
) -> Option<f64> {
    match expression {
        Expression::Number(number) => Some(*number),
        Expression::Variable { scope, name } => {
            let holder = holder_of(world, speaker, listener, scope)?;
            crate::dialogue_values::number_of(world, holder, name)
        }
        Expression::Binary { op, left, right } => {
            let left = value(world, speaker, listener, left)?;
            let right = value(world, speaker, listener, right)?;
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

/// Which entity a variable's scope reads, or nothing when the scope names somebody who is not here.
///
/// Reference: `read_var_info` (`condition.cpp:408-437`) classes a variable by the member that names it, and
/// `const_dialogue` then reads it from `actor( false )` or `actor( true )`. A bare identifier is looked up in the dialogue's
/// own context (`const_dialogue::maybe_get_value`, `npctalk.cpp:2659-2662`), which the port keeps on the speaker - the one
/// dialoguer that always exists.
fn holder_of(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    scope: &str,
) -> Option<Entity> {
    match scope {
        "global_val" => crate::dialogue_values::global_holder(world),
        "n_val" | "npc_val" => listener,
        _ => Some(speaker),
    }
}

/// The text a string source names, or nothing when the store cannot answer for it.
fn text_of(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    source: &StringSource,
) -> Option<String> {
    match source {
        StringSource::Literal(text) => Some(text.clone()),
        StringSource::Variable { scope, name } => {
            let holder = holder_of(world, speaker, listener, scope)?;
            crate::dialogue_values::text_of(world, holder, name)
        }
    }
}
