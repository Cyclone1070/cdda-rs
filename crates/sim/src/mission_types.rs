//! The mission definitions the game's data holds, by id.
//!
//! The data side is [`cdda_content::mission_types`]; this is the store, so a rule that has a mission's id can ask what the mission is, what it leads to and where it may come
//! from without going back to the JSON.
//!
//! Reference: `mission_type::get` / `mission_type::get_all` (`src/mission.h`, `mission.cpp`), the registry the reference keeps in a `generic_factory`.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Entity, World};

/// Every mission the data defines.
#[derive(Component, Debug, Clone, Default)]
pub struct MissionTypes {
    /// The definitions, sorted by id.
    pub kinds: Vec<cdda_content::mission_types::MissionType>,
    /// Where each id sits in `kinds`.
    pub by_id: std::collections::BTreeMap<String, usize>,
}

/// Marks the entity holding [`MissionTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheMissionTypes;

/// The entity holding [`MissionTypes`], if the game has loaded missions.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheMissionTypes>(world)
}

/// Loads the mission definitions into the store, replacing what was there.
pub fn set_mission_types(world: &mut World, missions: cdda_content::mission_types::Missions) {
    let entity = crate::singletons::singleton::<MissionTypes, TheMissionTypes>(world);
    if let Some(mut table) = world.get_mut::<MissionTypes>(entity) {
        table.by_id = missions.by_id;
        table.kinds = missions.kinds;
    }
}

/// The mission with that id, if the game knows one.
#[must_use]
pub fn kind_named(world: &World, id: &str) -> Option<cdda_content::mission_types::MissionType> {
    let entity = holder(world)?;
    let table = world.get::<MissionTypes>(entity)?;
    let at = table.by_id.get(id)?;
    table.kinds.get(*at).cloned()
}

/// The mission that follows a mission, if the data says one does.
#[must_use]
pub fn follow_up(world: &World, id: &str) -> Option<String> {
    kind_named(world, id).and_then(|kind| kind.follow_up)
}

/// Every mission id the data defines, sorted.
#[must_use]
pub fn ids(world: &World) -> Vec<String> {
    let Some(entity) = holder(world) else {
        return Vec::new();
    };
    world
        .get::<MissionTypes>(entity)
        .map_or_else(Vec::new, |table| {
            table.kinds.iter().map(|kind| kind.id.clone()).collect()
        })
}
