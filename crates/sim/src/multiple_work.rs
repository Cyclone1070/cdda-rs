//! Where a **multiple** activity does its work.
//!
//! Reference: `get_zone_for_act` (`activity_item_handling.cpp:2642-2688`), the first step of `generic_multi_activity_handler`: each job is done inside a **zone** of a particular
//! type, and the handler collects the zones within view distance before it chooses which one to walk to.
//!
//! The table is the whole of the port so far. The handler's next steps - collecting the zones, sorting them by distance, routing to one, and doing the work - all need the zone
//! store, which the port does not have yet; they are the named gap, not an omission.

use bevy_ecs::prelude::*;

/// Every zone type the data loaded.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneTypes(pub Vec<cdda_content::zone_types::ZoneType>);

/// Marks the one entity that holds the world's zone types.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheZoneTypes;

/// Installs the zone types the data loaded, replacing any that were there.
pub fn set_zone_types(world: &mut World, zone_types: Vec<cdda_content::zone_types::ZoneType>) {
    let entity = crate::singletons::singleton::<ZoneTypes, TheZoneTypes>(world);
    if let Some(mut held) = world.get_mut::<ZoneTypes>(entity) {
        held.0 = zone_types;
    }
}

/// One zone type, by id.
#[must_use]
pub fn zone_type_named(world: &World, id: &str) -> Option<cdda_content::zone_types::ZoneType> {
    let holder = crate::singletons::holder::<TheZoneTypes>(world)?;
    world
        .get::<ZoneTypes>(holder)
        .and_then(|types| types.0.iter().find(|kind| kind.id == id).cloned())
}

/// Which zone type an activity works in, and `None` for an activity that works in no zone.
///
/// Reference: `get_zone_for_act` (`activity_item_handling.cpp:2642-2688`): eleven activities map to a zone type, and everything else answers the **undefined** zone
/// (`zone_type_`), which the port spells `None`.
///
/// **The ids are the data's**, not invented: `sim_suite::multiple_work` asserts that every non-`None` answer names a zone type the data defines, which is the property that makes
/// this table safe to grow.
#[must_use]
pub fn zone_for_activity(activity: &str) -> Option<&'static str> {
    match activity {
        "ACT_VEHICLE_DECONSTRUCTION" => Some("VEHICLE_DECONSTRUCT"),
        "ACT_VEHICLE_REPAIR" => Some("VEHICLE_REPAIR"),
        "ACT_MULTIPLE_CHOP_TREES" => Some("CHOP_TREES"),
        "ACT_MULTIPLE_CONSTRUCTION" => Some("CONSTRUCTION_BLUEPRINT"),
        "ACT_MULTIPLE_FARM" => Some("FARM_PLOT"),
        "ACT_MULTIPLE_BUTCHER" => Some("LOOT_CORPSE"),
        "ACT_MULTIPLE_CHOP_PLANKS" => Some("LOOT_WOOD"),
        "ACT_MULTIPLE_FISH" => Some("FISHING_SPOT"),
        "ACT_MULTIPLE_MINE" => Some("MINING"),
        "ACT_MULTIPLE_MOP" => Some("MOPPING"),
        "ACT_MULTIPLE_DIS" => Some("DISASSEMBLE"),
        // Reading, crafting, tidying and the fetch errand work where they are told to, not in a zone: the reference answers the undefined zone for them too.
        _ => None,
    }
}
