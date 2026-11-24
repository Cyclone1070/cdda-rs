//! The vehicle groups the map's placements draw from.
//!
//! Reference: `VehicleGroup::pick` (`vehicle_group.cpp`), reached from `jmapgen_vehicle::apply` (`mapgen.cpp:3180-3200`) by way of `map::add_vehicle`. The store keeps the
//! groups the way it keeps the monster groups: one singleton entity, because a group is data the world holds rather than a thing in the world.

use crate::singletons::{holder, singleton};
use bevy_ecs::prelude::*;

/// The marker that says a single entity holds the vehicle groups.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheVehicleGroups;

/// Every vehicle group the content loaded.
#[derive(Debug, Clone, Default, Component)]
pub struct VehicleGroupStore(pub Vec<cdda_content::vehicle_groups::VehicleGroup>);

/// Installs the groups, replacing whatever the world held.
pub fn set_groups(world: &mut World, groups: Vec<cdda_content::vehicle_groups::VehicleGroup>) {
    let holder = singleton::<VehicleGroupStore, TheVehicleGroups>(world);
    world.entity_mut(holder).insert(VehicleGroupStore(groups));
}

/// Picks a vehicle out of a group by weight.
#[must_use]
pub fn pick(world: &World, group: &str, dice: &mut cdda_core_types::rng::Rolls) -> Option<String> {
    let store = holder::<TheVehicleGroups>(world)
        .and_then(|holder| world.get::<VehicleGroupStore>(holder))?;
    cdda_content::vehicle_groups::pick(group, &store.0, dice)
}

/// Whether an id names a group rather than a prototype.
#[must_use]
pub fn is_group(world: &World, id: &str) -> bool {
    holder::<TheVehicleGroups>(world)
        .and_then(|holder| world.get::<VehicleGroupStore>(holder))
        .is_some_and(|store| cdda_content::vehicle_groups::is_group(id, &store.0))
}
