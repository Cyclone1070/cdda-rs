//! The traits a creature carries.
//!
//! Reference: `Character::has_trait` reads `my_traits`, a set of `trait_id`s (`character.cpp`). The port keeps the same answer
//! as a component on the creature: one set of ids, no parallel container, and every question asks the store.
//!
//! Traits are pure data - the mutations they imply are the reference's `mutation_branch`, which the port has not ported. This
//! module therefore only answers "does it have this one", which is what the dialogue conditions ask.

use bevy_ecs::prelude::*;
use std::collections::BTreeSet;

/// The traits a creature carries, by id.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Traits {
    names: BTreeSet<String>,
}

/// Gives a creature exactly these traits, replacing whatever it had.
pub fn set_traits(world: &mut World, creature: Entity, names: &[&str]) {
    let traits = traits_of(names);
    if let Some(mut held) = world.get_mut::<Traits>(creature) {
        *held = traits;
        return;
    }
    world.entity_mut(creature).insert(traits);
}

/// Adds one trait, which is what a mutation does.
/// Records which mutation **variant** a creature has for a trait.
///
/// Reference: `Character::set_mutation( trait, variant )` (`mutation.cpp:1074-1090`): the variant picks one of a trait's cosmetic variations - a different name and
/// description, no rule attached. The port keeps the choice so the data's intent is not lost, in its own component, and no rule reads it yet. **The component exists so
/// that the day a rule does read it, the answer is already stored rather than dropped at load time.**
pub fn set_variant(world: &mut World, creature: Entity, trait_name: &str, variant: &str) {
    if let Some(mut held) = world.get_mut::<TraitVariants>(creature) {
        held.0.insert(trait_name.to_owned(), variant.to_owned());
    } else {
        let mut held = TraitVariants::default();
        held.0.insert(trait_name.to_owned(), variant.to_owned());
        world.entity_mut(creature).insert(held);
    }
}

/// The variant chosen for each trait, when the data chose one.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Default, PartialEq)]
pub struct TraitVariants(pub std::collections::BTreeMap<String, String>);

/// The variant a creature has for a trait, when one was chosen.
#[must_use]
pub fn variant_of(world: &World, creature: Entity, trait_name: &str) -> Option<String> {
    world
        .get::<TraitVariants>(creature)
        .and_then(|held| held.0.get(trait_name).cloned())
}

/// Adds a trait to a creature, doing nothing when it already has it.
///
/// Reference: `Character::set_mutation` adds the mutation to the set it holds; a trait set twice is still one trait.
pub fn add_trait(world: &mut World, creature: Entity, name: &str) {
    if let Some(mut held) = world.get_mut::<Traits>(creature) {
        held.names.insert(name.to_owned());
        return;
    }
    world.entity_mut(creature).insert(traits_of(&[name]));
}

/// Removes one trait, answering whether it was there.
pub fn remove_trait(world: &mut World, creature: Entity, name: &str) -> bool {
    let Some(mut held) = world.get_mut::<Traits>(creature) else {
        return false;
    };
    held.names.remove(name)
}

/// Whether a creature carries a trait.
#[must_use]
pub fn has_trait(world: &World, creature: Entity, name: &str) -> bool {
    world
        .get::<Traits>(creature)
        .is_some_and(|traits| traits.names.contains(name))
}

/// Every trait a creature carries, in id order.
#[must_use]
pub fn traits_held(world: &World, creature: Entity) -> Vec<String> {
    world
        .get::<Traits>(creature)
        .map_or_else(Vec::new, |traits| traits.names.iter().cloned().collect())
}

/// A trait set built from ids.
fn traits_of(names: &[&str]) -> Traits {
    Traits {
        names: names.iter().map(|name| (*name).to_owned()).collect(),
    }
}
