//! A bite going into a stomach, which is the one step between the food rules and the digest stage.
//!
//! Reference: `Character::consume_effects` (`consumption.cpp:1689-1714`) computes the summary and hands it to `stomach.ingest`, which adds it unchanged. The summary
//! itself is [`crate::food_summary`]'s rule, proven against all 1,686 comestibles in the corpus; **this module is the wire**, because a rule the game cannot reach is not
//! ported (`docs/13-test-inventory.md`).
//!
//! The reference also runs the item's own effects (a drug's `poison`, a mutagen's mutation) at this point. The port refuses a bite whose summary it cannot build and
//! leaves the item where it is; the effects a bite can carry are named in `docs/parity.md` as waiting for the effect table's item side.

use bevy_ecs::prelude::*;

use crate::body::Body;
use crate::food_summary::{Diet, FoodSummary, ItemFacts, food_summary};

/// Why a bite did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiteError {
    /// The creature has no body to put it in.
    NoBody,
    /// What is being eaten is not food or drink at all.
    NotFood,
}

/// Eats one bite: builds the summary the reference builds and puts it in the stomach.
///
/// Reference: `Character::consume_effects` (`consumption.cpp:1689-1714`) - the summary is **computed from the item's own facts**, and the item is taken by the caller
/// before this runs, so an item that cannot be eaten is still there afterwards.
///
/// # Errors
///
/// Returns [`BiteError`] when the creature has no body or the item is not food. Nothing is changed when it fails.
pub fn eat_a_bite(
    world: &mut World,
    creature: Entity,
    facts: &ItemFacts<'_>,
    flags: &std::collections::BTreeSet<String>,
    relative_rot: f32,
    diet: Diet,
    registry: &std::collections::BTreeMap<String, cdda_content::vitamins::Vitamin>,
) -> Result<FoodSummary, BiteError> {
    let summary = food_summary(facts, flags, relative_rot, diet, registry);
    if summary.water_ml == 0 && summary.solids_ml == 0 && summary.calories == 0 && summary.vitamins.is_empty() {
        return Err(BiteError::NotFood);
    }
    let Some(mut body) = world.get::<Body>(creature).cloned() else {
        return Err(BiteError::NoBody);
    };
    body.stomach.ingest(&summary);
    world.entity_mut(creature).insert(body);
    Ok(summary)
}
