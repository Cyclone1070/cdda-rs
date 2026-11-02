//! How wet a creature is.
//!
//! Reference: `Character::mod_wetness` (`character.cpp`) raises and lowers a wetness value that lives on the character's body parts, and `wet_character`
//! (`npctalk.cpp:5464-5474`) is the dialogue's way of adding to it. The port has no limb model - a creature is one thing - so wetness is **one number per
//! creature** rather than a number per part. That is a simplification of the store, not of the rule: adding forty wetness adds forty.

use bevy_ecs::prelude::{Component, World};

/// How wet a creature is, in the reference's own units: zero dry, a hundred soaked.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Wetness(pub i32);

/// Raises a creature's wetness, keeping it between dry and soaked.
///
/// Reference: `Character::mod_wetness` clamps to the same range: nothing is less than dry, and nothing is wetter than soaked.
pub fn add(world: &mut World, creature: bevy_ecs::prelude::Entity, amount: i32) {
    let soaked = 100;
    if let Some(mut wet) = world.get_mut::<Wetness>(creature) {
        wet.0 = (wet.0 + amount).clamp(0, soaked);
        return;
    }
    world.entity_mut(creature).insert(Wetness(amount.clamp(0, soaked)));
}

/// How wet a creature is, answering dry for one that has never been wet.
#[must_use]
pub fn of(world: &World, creature: bevy_ecs::prelude::Entity) -> i32 {
    world.get::<Wetness>(creature).map_or(0, |wet| wet.0)
}
