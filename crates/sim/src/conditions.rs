//! Whether a dialogue condition holds for a creature.
//!
//! Reference: the callable at the heart of `conditional_t` (`condition.cpp:2660-2760`), which answers a question about one
//! dialoguer and one talker. The port answers from the store: what a creature is wearing, what it has, where it is.

use bevy_ecs::prelude::*;

use cdda_content::dialogue::Condition;

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
    }
}
