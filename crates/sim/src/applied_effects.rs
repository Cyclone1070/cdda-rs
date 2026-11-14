//! The effects a creature is wearing, on the creature.
//!
//! Reference: `Creature::effects` (`src/creature.h`), a map from effect type to `effect`, with `add_effect`
//! (`creature.cpp:1731-1856`) putting things in it and `remove_effect` taking them out. [`crate::effects`] already knew
//! what repeated application does to an effect's numbers; what was missing was somewhere to keep the result, which is
//! this: one component on the creature, no table beside it.

use bevy_ecs::prelude::*;

use crate::effects::ActiveEffect;

/// Every effect a creature is wearing.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedEffects(pub Vec<ActiveEffect>);

/// Every effect a creature is wearing.
#[must_use]
pub fn worn(world: &World, creature: Entity) -> Vec<ActiveEffect> {
    world
        .get::<AppliedEffects>(creature)
        .map_or_else(Vec::new, |worn| worn.0.clone())
}

/// Puts an effect on a creature.
///
/// Reference: `Creature::add_effect` - an effect the creature does not have is added, and one it does have has its
/// numbers changed instead, which is what [`ActiveEffect::new`] already decides.
pub fn add(world: &mut World, creature: Entity, effect: ActiveEffect) {
    let Some(mut worn) = world.get_mut::<AppliedEffects>(creature) else {
        world
            .entity_mut(creature)
            .insert(AppliedEffects(vec![effect]));
        return;
    };
    if let Some(existing) = worn.0.iter_mut().find(|worn| worn.id == effect.id) {
        *existing = effect;
    } else {
        worn.0.push(effect);
    }
}

/// Takes an effect off a creature, answering whether it had one.
///
/// Reference: `Creature::remove_effect( efftype_id )` (`creature.cpp:1985-2010`), which removes every effect of that type
/// and touches nothing else.
pub fn remove(world: &mut World, creature: Entity, id: &str) -> bool {
    let Some(mut worn) = world.get_mut::<AppliedEffects>(creature) else {
        return false;
    };
    let before = worn.0.len();
    worn.0.retain(|worn| worn.id != id);
    worn.0.len() != before
}

/// Whether a creature is wearing an effect.
#[must_use]
pub fn has(world: &World, creature: Entity, id: &str) -> bool {
    world
        .get::<AppliedEffects>(creature)
        .is_some_and(|worn| worn.0.iter().any(|worn| worn.id == id))
}

/// How strong an effect is on a creature, or nothing when it is not wearing it.
#[must_use]
pub fn intensity_of(world: &World, creature: Entity, id: &str) -> Option<i32> {
    world
        .get::<AppliedEffects>(creature)
        .and_then(|worn| worn.0.iter().find(|worn| worn.id == id))
        .map(|worn| worn.intensity)
}

/// Spends one turn of every effect on a creature, and takes off the ones that have run out.
///
/// Reference: `Creature::process_effects`, which decrements each effect's duration and removes the expired ones; a
/// permanent effect is one with no duration to spend. Answers how many effects came off, which is what makes the rule
/// testable without reading the component back.
pub fn spend_a_turn(world: &mut World, creature: Entity) -> usize {
    let Some(mut worn) = world.get_mut::<AppliedEffects>(creature) else {
        return 0;
    };
    let before = worn.0.len();
    for effect in &mut worn.0 {
        if !effect.permanent {
            effect.duration -= 1;
        }
    }
    worn.0
        .retain(|effect| effect.permanent || effect.duration > 0);
    before - worn.0.len()
}
