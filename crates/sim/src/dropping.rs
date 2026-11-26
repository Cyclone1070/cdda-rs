//! **Putting down what a creature is holding.** Reference: `Character::remove_weapon` (`character.cpp`) and the two dialogue effects that use it - the player dropping their
//! own weapon and an NPC being made to drop theirs.
//!
//! Dropping is the inverse of [`crate::creature::pick_up`], and the port's item model makes it three component changes: the thing stops being `Carried` and `WieldedBy`, it
//! gains a `Position`, and it goes back into the index. The index is the only place that says what is on a square, so a thing put down without it would be on no surface at all.

use bevy_ecs::prelude::{Entity, World};

/// **Puts what a creature is holding on the square it stands on**, answering whether there was anything to put down.
///
/// Reference: `Character::remove_weapon` (`character.cpp`) returns the item it took out of the character's hands; the callers then place it - `talk_function::drop_weapon`
/// puts it at the person's square and `talk_function::player_weapon_drop` drops it through the player's own drop path.
///
/// The port takes no item: it finds the one marked [`crate::creature::WieldedBy`] the creature, which is the same thing the reference's `weapon` member means.
pub fn put_down_what_is_held(world: &mut World, creature: Entity) -> Option<Entity> {
    let at = world.get::<crate::Position>(creature).copied()?;
    let held = {
        let mut query = world.query::<(Entity, &crate::creature::WieldedBy)>();
        query
            .iter(world)
            .find(|(_, wielded)| wielded.0 == creature)
            .map(|(entity, _)| entity)
    }?;
    world
        .entity_mut(held)
        .remove::<crate::creature::WieldedBy>();
    world.entity_mut(held).remove::<crate::creature::Carried>();
    world.entity_mut(held).remove::<crate::creature::InPocket>();
    world.entity_mut(held).insert(at);
    crate::singletons::index_mut(world).insert(held, (at.x, at.y, at.z));
    Some(held)
}
