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

/// How far a job looks for work: the reference's `MAX_VIEW_DISTANCE`, which is `SEEX * HALF_MAPSIZE` = `12 * 5` = **60** squares (`map_scale_constants.h:11-27`).
pub const MAX_VIEW_DISTANCE: i32 = 60;

/// The square a job should work at next: the nearest square of the zones its activity wants, within view distance.
///
/// Reference: the first half of `generic_multi_activity_handler` (`activity_item_handling.cpp:3334-3400`): it collects the zones of the activity's type
/// (`generic_multi_activity_locations`), sorts the spots **by distance** (`get_sorted_tiles_by_distance`, `:3349`) and takes them in that order. The port does the same two steps
/// and answers the nearest square, with a tie broken by the square itself so the answer does not depend on the order the store happens to hold.
///
/// **The faction is the caller's**: the reference asks the character's own faction (`_fac_id`, `:162-166`), and the port has no faction per character yet, so it is passed in and
/// named as a gap rather than guessed at.
///
/// An activity with no zone (`zone_for_activity` answers `None`) has no work square, which is not the same as having none **found** - see [`the_job_has_nothing_to_do`].
#[must_use]
pub fn next_work_square(
    world: &mut bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
    faction: &str,
) -> Option<(i32, i32, i32)> {
    let activity = world
        .get::<crate::creature::Doing>(npc)
        .map(|doing| doing.activity.clone())?;
    let kind = zone_for_activity(&activity)?;
    let here = world
        .get::<crate::Position>(npc)
        .map(|place| (place.x, place.y, place.z))?;
    let squares = crate::zones::zones_near(world, kind, faction, here, MAX_VIEW_DISTANCE);
    squares
        .into_iter()
        .min_by_key(|square| (crate::zones::square_dist(*square, here), *square))
}

/// Whether a multiple job has looked and found nothing to do, and so ends.
///
/// Reference: the end of `generic_multi_activity_handler` (`activity_item_handling.cpp:3502-3505`): when the work-spot set is **empty** the person's job is over and the game says
/// *"%s failed to perform the %s activity because no suitable locations were found."* The port ends the job and writes the same sentence.
///
/// **Only a job that works in a zone can end this way**: reading, crafting and the errands have no zone, and an activity with no zone spot is not a job with no work.
#[must_use]
pub fn the_job_has_nothing_to_do(
    world: &mut bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
    faction: &str,
) -> bool {
    let Some(activity) = world
        .get::<crate::creature::Doing>(npc)
        .map(|doing| doing.activity.clone())
    else {
        return false;
    };
    if zone_for_activity(&activity).is_none() {
        return false;
    }
    if next_work_square(world, npc, faction).is_some() {
        return false;
    }
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(
        world,
        &format!(
            "{name} failed to perform the {activity} activity because no suitable locations were found."
        ),
    );
    world.entity_mut(npc).remove::<crate::creature::Doing>();
    true
}
