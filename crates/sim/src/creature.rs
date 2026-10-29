//! The living things in the world.
//!
//! Reference: `Creature` (`src/creature.h:47`) and its subclasses - `monster`, `npc`, `avatar` - are a
//! class hierarchy with virtual methods, and the simulation keeps them in parallel containers beside
//! the map. The port keeps none of that structure: a creature is an entity like a square or an item,
//! and what a creature *has* becomes components it carries.
//!
//! What it has, and nothing more than the reference's own rules read:
//!
//! * [`Position`] - the square it stands on, shared with everything else that has a place.
//! * [`Creature`] - the name it is known by and how big it is, which the reference reads for damage
//!   scaling and for what it can wear.
//! * [`Moves`] - the action points it has banked. The reference holds these on the creature
//!   (`Creature::moves`); here they are a component, so a system that spends them asks the store.

use crate::CreatureSize;
use crate::Position;
use bevy_ecs::prelude::*;

/// A living thing: what it is called, and how big it is.
///
/// Reference: `Creature::disp_name`, and `Creature::get_size`, which `monster` and `npc` each answer
/// from their own type. The size is a component's field rather than a subclass, so a dog that grows is
/// a change to one value, not a different entity.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Creature {
    /// The name a message about it uses.
    pub name: String,
    /// How big it is, which decides how much damage it takes and what fits it.
    pub size: CreatureSize,
}

/// Action points banked this turn.
///
/// Reference: `Creature::moves` (`src/creature.h`), which the reference adds to and spends; a creature
/// with enough moves takes a step, and one without waits. Kept as a component so that "who can act" is
/// a query rather than a loop over a list of creatures.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Moves {
    /// The points left to spend.
    pub left: i32,
}

/// Puts a creature in the world, standing on `square`.
///
/// The square is written twice - as the entity's [`Position`] and into the index - because the index is
/// derived: [`crate::PositionIndex::rebuild`] would produce the same thing from the store, and the
/// architecture test checks exactly that.
pub fn spawn_creature(world: &mut World, name: &str, square: (i32, i32, i32)) -> Entity {
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            Creature {
                name: name.to_owned(),
                size: CreatureSize::Medium,
            },
            Moves::default(),
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
    entity
}

/// Every creature standing on `square`, in the order the world holds them.
///
/// A square can hold more than one: two dogs in a doorway is an ordinary state, and the reference's
/// `creature_at` returns a list for that reason.
#[must_use]
pub fn creature_at(world: &World, square: (i32, i32, i32)) -> Vec<Entity> {
    let Some(index) = crate::singletons::index(world) else {
        return Vec::new();
    };
    index
        .at_level(square.0, square.1, square.2)
        .iter()
        .copied()
        .filter(|entity| world.get::<Creature>(*entity).is_some())
        .collect()
}

/// Adds action points to a creature.
pub fn give_moves(world: &mut World, entity: Entity, points: i32) {
    if let Some(mut moves) = world.get_mut::<Moves>(entity) {
        moves.left += points;
    }
}
