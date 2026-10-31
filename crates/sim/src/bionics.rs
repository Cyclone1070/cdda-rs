//! Bionics: the machinery a creature carries inside it.
//!
//! Reference: `Character::has_bionic` and `num_bionics` (`character.cpp`). A bionic is an id on a list; the port keeps the list as a component on
//! the creature, so "does it have this bionic" is a question about one entity's data rather than a lookup in a side table.

use bevy_ecs::prelude::*;

/// The bionics installed in one creature.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct Bionics(pub Vec<String>);

/// Installs a bionic, keeping the list free of duplicates.
pub fn install(world: &mut World, creature: Entity, bionic: &str) {
    let mut held = world.get::<Bionics>(creature).cloned().unwrap_or_default();
    if held.0.iter().any(|known| known == bionic) {
        return;
    }
    held.0.push(bionic.to_owned());
    world.entity_mut(creature).insert(held);
}

/// Whether a creature has a bionic, or - with `ANY` - any bionic at all.
///
/// Reference: `f_has_bionics` (`condition.cpp:946-949`): `ANY` holds when the creature has any bionic or a full power store, and any other name
/// asks for that bionic in particular.
#[must_use]
pub fn has(world: &World, creature: Entity, asked: &str) -> bool {
    let held = world.get::<Bionics>(creature);
    if asked == "ANY" {
        // Reference: `condition.cpp:946-949` also accepts a full power store. The port has no power store yet, so `ANY` asks only about the list -
        // a divergence recorded in `docs/parity.md`.
        return held.is_some_and(|held| !held.0.is_empty());
    }
    held.is_some_and(|held| held.0.iter().any(|known| known == asked))
}

/// Every bionic a creature has installed.
#[must_use]
pub fn bionics_of(world: &World, creature: Entity) -> Vec<String> {
    world
        .get::<Bionics>(creature)
        .map_or_else(Vec::new, |held| held.0.clone())
}
