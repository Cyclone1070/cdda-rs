//! Whether a creature is on the player's side.
//!
//! Reference: `monster::friendly` (`monster.h`), a count of turns the monster stays friendly, set from `f_spawn_monster`'s `friendly` member and read by
//! `monster::attitude`. The port keeps the **fact**, not the count: one marker component on the creature, so anything that asks "is this one ours" reads the
//! store instead of a field beside it. When the turn count earns its keep - a summon that turns on its master - it becomes a component with a number, and the
//! sites that read the marker keep working.

use bevy_ecs::component::Component;

/// The player's side, on a creature that was spawned as an ally.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Friendly;

/// Whether a creature is on the player's side.
#[must_use]
pub fn is_friendly(world: &bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity) -> bool {
    world.get::<Friendly>(creature).is_some()
}
