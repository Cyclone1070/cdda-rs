//! Whether a creature is on the player's side, and for how much longer.
//!
//! Reference: `monster::friendly` (`monster.h`), a count of turns the monster stays friendly, set from `f_spawn_monster`'s `friendly` member and read by
//! `monster::attitude`. The port keeps the number, because the rules that write it are counts: `f_spawn_monster` marks a spawn friendly, and a charm spell
//! **adds** `duration / 100` to it (`magic_spell_effect.cpp:1546`) - so a component with a count is what lets the second rule exist at all.
//!
//! A count of zero means "not friendly", which is why a creature that was never marked has no component: the store answers "no" by not being there, rather than by a
//! zero that some rule has to remember to check.

use bevy_ecs::component::Component;

/// How many turns a creature stays on the player's side.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Friendly {
    /// The count the reference adds to. `f_spawn_monster` starts it at what the data asked; a charm spell adds to it.
    pub turns: i32,
}

/// Makes a creature friendly for a number of turns, replacing anything it had before.
pub fn make_friendly(world: &mut bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity, turns: i32) {
    world
        .entity_mut(creature)
        .insert(Friendly { turns: turns.max(0) });
}

/// Adds to how long a creature stays friendly, which is what a charm spell does.
///
/// Reference: `mon->friendly += sp.duration( caster ) / 100` (`magic_spell_effect.cpp:1546`) - an **addition**, so charming an already-charmed monster extends it
/// rather than resetting it.
pub fn charm(world: &mut bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity, turns: i32) {
    let current = world
        .get::<Friendly>(creature)
        .copied()
        .unwrap_or(Friendly { turns: 0 });
    world
        .entity_mut(creature)
        .insert(Friendly { turns: current.turns.saturating_add(turns) });
}

/// Whether a creature is on the player's side.
#[must_use]
pub fn is_friendly(world: &bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity) -> bool {
    world.get::<Friendly>(creature).is_some_and(|held| held.turns != 0)
}

/// How long a creature will stay friendly, which is zero for one that never was.
#[must_use]
pub fn turns_of(world: &bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity) -> i32 {
    world.get::<Friendly>(creature).map_or(0, |held| held.turns)
}
