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
    world.get::<Traits>(creature).map_or_else(Vec::new, |traits| {
        traits.names.iter().cloned().collect()
    })
}

/// A trait set built from ids.
fn traits_of(names: &[&str]) -> Traits {
    Traits {
        names: names.iter().map(|name| (*name).to_owned()).collect(),
    }
}
