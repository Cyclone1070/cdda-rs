//! The monster groups the world can draw from.
//!
//! Reference: `MonsterGroupManager` (`mongroup.cpp`), which keeps every `MonsterGroup` loaded from the data and answers `GetMonsterFromGroup`. The port's
//! `cdda_content::monster_groups` reads and picks; this holds them in the store so a rule can ask for a monster without being handed the table.

use bevy_ecs::prelude::*;

/// Every monster group, as the data defines them.
#[derive(Component, Debug, Clone, Default)]
pub struct MonsterGroups(pub Vec<cdda_content::monster_groups::MonsterGroup>);

/// Marks the entity holding the groups.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheMonsterGroups;

/// Installs the groups a spawn may draw from. A second call replaces them, which is what loading a world twice does.
pub fn set_groups(world: &mut World, groups: Vec<cdda_content::monster_groups::MonsterGroup>) {
    let holder = crate::singletons::singleton::<MonsterGroups, TheMonsterGroups>(world);
    world.entity_mut(holder).insert(MonsterGroups(groups));
}

/// Picks a monster out of a group, resolving nested groups by weight.
///
/// Reference: `MonsterGroupManager::GetMonsterFromGroup` (`mongroup.cpp`), whose pick walks the group's weights and repeats inside a nested group. The port
/// answers a **name**, because that is what the rest of the store holds: a monster type is found by its id rather than by a pointer to its prototype.
#[must_use]
pub fn pick(world: &World, group: &str, dice: &mut cdda_core_types::rng::Rolls) -> Option<String> {
    let groups = crate::singletons::holder::<TheMonsterGroups>(world)
        .and_then(|holder| world.get::<MonsterGroups>(holder))?;
    cdda_content::monster_groups::pick_monster(group, &groups.0, dice).map(str::to_owned)
}
