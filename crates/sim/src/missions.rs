//! The missions a character has taken, and the one the player is.
//!
//! Reference: `f_u_has_mission` (`condition.cpp:709-721`) walks `get_avatar().get_active_missions()` looking for a mission whose
//! `mission_id` matches, and answers true when one does. The port keeps the same answer as a component - a set of mission type
//! ids on the character - and the same question, asked of the **avatar** rather than of whoever is speaking.
//!
//! That distinction matters: an NPC's dialogue can ask `u_has_mission`, and the reference answers about the player. The port has
//! no separate avatar identity yet, so it looks for an entity carrying [`TheAvatar`] and falls back to the speaker when nobody has
//! claimed it - a divergence recorded in `docs/parity.md` rather than hidden.

use bevy_ecs::prelude::*;
use std::collections::BTreeSet;

/// The mission type ids a character has taken.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Missions {
    active: BTreeSet<String>,
}

/// Marks the character the player is.
///
/// Reference: `get_avatar()` (`game.cpp`), which answers the one character the player drives. The port states it as a marker so a
/// question can ask "the player's missions" without assuming the speaker is the player.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheAvatar;

/// Takes a mission, which is what accepting one does.
pub fn accept(world: &mut World, character: Entity, mission: &str) {
    if let Some(mut held) = world.get_mut::<Missions>(character) {
        held.active.insert(mission.to_owned());
        return;
    }
    world
        .entity_mut(character)
        .insert(Missions { active: BTreeSet::from([mission.to_owned()]) });
}

/// Drops a mission, answering whether it was there.
pub fn finish(world: &mut World, character: Entity, mission: &str) -> bool {
    let Some(mut held) = world.get_mut::<Missions>(character) else {
        return false;
    };
    held.active.remove(mission)
}

/// Whether a character has taken a mission.
#[must_use]
pub fn has_mission(world: &World, character: Entity, mission: &str) -> bool {
    world
        .get::<Missions>(character)
        .is_some_and(|held| held.active.contains(mission))
}

/// Every mission a character has taken, in id order.
#[must_use]
pub fn active_missions(world: &World, character: Entity) -> Vec<String> {
    world.get::<Missions>(character).map_or_else(Vec::new, |held| {
        held.active.iter().cloned().collect()
    })
}

/// The player, or nothing when no entity has claimed to be one.
#[must_use]
pub fn avatar(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheAvatar>>()?;
    query.iter(world).next()
}

/// Makes a character the player's.
pub fn set_avatar(world: &mut World, character: Entity) {
    if let Ok(mut entity) = world.get_entity_mut(character) {
        entity.insert(TheAvatar);
    }
}

/// Whose missions a dialogue question is about.
///
/// Reference: the avatar's, always. The port prefers the avatar and falls back to the speaker when nobody has claimed the role, so
/// a test can ask the question without building a whole player.
#[must_use]
pub fn question_holder(world: &World, speaker: Entity) -> Entity {
    avatar(world).unwrap_or(speaker)
}
