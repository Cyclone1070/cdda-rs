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
    // **A spot that cannot be worked is skipped**, which is the reference's `can_do_activity_there` gate (`activity_item_handling.cpp:3367-3420`): the handler walks the spots in
    // distance order and passes over every one the activity's own check fails, so the person's nearest *workable* square is the one it walks to.
    squares
        .into_iter()
        .filter(|square| can_work_at(world, npc, &activity, *square))
        .min_by_key(|square| (crate::zones::square_dist(*square, here), *square))
}

/// Whether an activity can be done at a square at all.
///
/// Reference: the per-activity branch of `can_do_activity_there` (`activity_item_handling.cpp:1080-1300`), whose answer for each spot is one of "ok" or a failure that **skips
/// that spot**. The butchering branch asks two things: the spot must hold a corpse (`:1210-1215`, `:1263-1264`), and the worker must carry a blade that reaches `BUTCHER` one
/// (`:1254`). Both are answered here, which is why the function takes the **person** as well as the spot - a gate that only knew the square could not ask the second.
///
/// **Mining, fishing and chopping answer both halves.** Each branch asks what the ground is *and* what the worker is holding, and both are ported: the terrain by its flag
/// (`MINEABLE`, `FISHABLE`, `TREE` or the two ids), the tool by a digging tool for mining - a `DIG_TOOL` flag that is not a jackhammer, or a jackhammer with a charge, which is the
/// reference's two arms exactly - and by `carried_has_quality` at the default level of zero for the rod and the axe.
///
/// **Mopping answers its item half too**: a liquid item lying on the square, and a `MOP` flag in the worker's hands - the reference's `terrain_moppable` also counts moppable
/// fields and vehicle blood, and the port places neither as a square thing yet.
///
/// **The activities that need furniture, an item or a vehicle answer `true`**: disassembly and construction want items, and the vehicle branches want a vehicle. Answering `true` keeps those jobs walking to their zones as they did before this rule existed; answering `false` would end every one of them on its first
/// turn, which is a worse lie than the one it would be replacing.
#[must_use]
pub fn can_work_at(
    world: &mut bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
    activity: &str,
    square: (i32, i32, i32),
) -> bool {
    match activity {
        "ACT_MULTIPLE_BUTCHER" => {
            let corpse = {
                let mut query = world.query::<(
                    bevy_ecs::prelude::Entity,
                    &crate::Position,
                    &crate::CorpseOf,
                )>();
                let mut found = None;
                for (entity, place, _) in query.iter(world) {
                    if (place.x, place.y, place.z) == square {
                        found = Some(entity);
                        break;
                    }
                }
                let Some(corpse) = found else {
                    return false;
                };
                corpse
            };
            // **A body somebody is already butchering is not a spot for anybody else.** Reference: `:1213` skips a corpse that carries `activity_var` - the marker the reference
            // puts on an item somebody has taken for work. The port keeps no marker: the activity's own `subject` says who is working on what, so the answer is read from the store
            // and cannot go stale when a cut is cancelled.
            // **And the worker needs a blade.** Reference: `:1254` answers `NEEDS_BUTCHERING` only when `has_quality( qual_BUTCHER, 1 )` holds, and a spot that fails is skipped -
            // so a person with no knife walks past every corpse rather than starting a cut that could never finish. The question is `has_quality`, not `max_quality`: a knife
            // lying beside the body is reachable for the cut, but the gate is asked before anybody walks there, and the reference asks it about what is carried.
            !somebody_is_working_on(world, corpse)
                && crate::creature::carried_has_quality(world, npc, "BUTCHER", 1)
        }
        // **Mining wants mineable ground.** Reference: `:1160-1163` fails a square without `TFLAG_MINEABLE`. The branch's other half - a digging tool in the worker's hands - is a
        // question the port cannot ask yet, so the terrain is the whole of the answer here rather than half of it.
        "ACT_MULTIPLE_MINE" => {
            ground_has_flag(world, square, "MINEABLE") && carried_digging_tool(world, npc)
        }
        // **Fishing wants fishable water.** Reference: `:1188-1191`. The rod is the unanswerable half.
        "ACT_MULTIPLE_FISH" => {
            ground_has_flag(world, square, "FISHABLE")
                && crate::creature::carried_has_quality(world, npc, "FISHING_ROD", 0)
        }
        // **Chopping wants a tree, a trunk or a stump.** Reference: `:1198-1209` - the two ids are compared exactly and everything else needs the `TFLAG_TREE` flag. The axe is
        // the branch's other half.
        "ACT_MULTIPLE_CHOP_TREES" => {
            let Some(code) = crate::square::terrain_at(world, square) else {
                return false;
            };
            let has_tree = crate::terrain_types::has_flag(world, code, "TREE")
                || matches!(
                    crate::terrain_types::name_of(world, code).as_deref(),
                    Some("t_trunk" | "t_stump")
                );
            has_tree && crate::creature::carried_has_quality(world, npc, "AXE", 0)
        }
        // **Mopping wants a spill and a mop.** Reference: `:1175-1186` asks `terrain_moppable( src_loc )` and then `cache_has_item_with( json_flag_MOP )`. The ground test
        // (`map.cpp:3560-3601`) answers true for a **liquid item lying on the square**, for a moppable field, and for vehicle blood; the port has the item half and the flag, and
        // fields as square things and vehicles are declared unported rather than guessed at.
        "ACT_MULTIPLE_MOP" => {
            ground_has_liquid_item(world, square)
                && crate::creature::carried_has_flag(world, npc, "MOP")
        }
        _ => true,
    }
}

/// Whether the worker carries a digging tool that actually works.
///
/// Reference: `:1165-1171` is two arms - `itm.has_flag( DIG_TOOL ) && !itm.type->can_use( "JACKHAMMER" )`, or `itm.type->can_use( "JACKHAMMER" ) && itm.ammo_sufficient( &you )`.
/// The exclusion is what makes the rule more than a flag: a jackhammer **has** `DIG_TOOL` in the data, and a flat one with no charge in it is not a digging tool. The charge test is
/// `ammo_sufficient` at its default quantity of one (`item.cpp:11569-11582`), which for a powered tool is "at least one charge".
fn carried_digging_tool(
    world: &mut bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
) -> bool {
    crate::creature::carried_items(world, npc)
        .into_iter()
        .any(|(kind, charges)| {
            if crate::item_registry::can_use(world, kind, "JACKHAMMER") {
                charges >= 1
            } else {
                crate::item_registry::kind_has_flag(world, kind, "DIG_TOOL")
            }
        })
}

/// Whether a liquid item lies on a square.
///
/// Reference: the item half of `map::terrain_moppable` (`map.cpp:3563-3572`), which walks `i_at( p )` looking for something `made_of( phase_id::LIQUID )` - a spilled drink is
/// mopped up by a mop, and the square it is on is the spot the activity walks to.
fn ground_has_liquid_item(world: &mut bevy_ecs::prelude::World, square: (i32, i32, i32)) -> bool {
    let spilled: Vec<crate::ItemTypeId> = {
        let mut query = world.query::<(&crate::GroundItem, &crate::Position)>();
        query
            .iter(world)
            .filter(|(_, at)| (at.x, at.y, at.z) == square)
            .map(|(item, _)| item.kind)
            .collect()
    };
    spilled
        .into_iter()
        .any(|kind| crate::item_registry::is_liquid(world, kind))
}

/// Whether the ground at a square carries a terrain flag.
///
/// Reference: `map::has_flag( ter_furn_flag, p )` (`map.cpp`), which the activity gates read for the one flag their rule is about. Furniture is not placed as a square thing in the
/// port yet, so the answer is the terrain's alone and the doc says so rather than guessing what a furniture flag would add.
fn ground_has_flag(world: &bevy_ecs::prelude::World, square: (i32, i32, i32), flag: &str) -> bool {
    crate::square::terrain_at(world, square)
        .is_some_and(|code| crate::terrain_types::has_flag(world, code, flag))
}

/// Whether anybody's activity has this thing as its subject.
///
/// The reference keeps the same fact on the item as the `activity_var` string (`activity_item_handling.cpp:1213`), which has to be cleared wherever the work ends. The port derives
/// it from `Doing::subject`, which is where the work already lives: one rule, one place to look, and nothing to clear.
#[must_use]
fn somebody_is_working_on(
    world: &mut bevy_ecs::prelude::World,
    thing: bevy_ecs::prelude::Entity,
) -> bool {
    let mut query = world.query::<&crate::creature::Doing>();
    query.iter(world).any(|doing| doing.subject == Some(thing))
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

/// What spending a turn on a job did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOfWork {
    /// The creature is not doing anything that works in a zone, so this rule has no opinion.
    NotAJob,
    /// The job looked, found no work in reach, and ended.
    Ended,
    /// The creature stands on its work square, and the work there is one the port has not ported yet.
    Ready,
    /// The creature stood on its work square, found the thing to work on, and **started the work**.
    Started,
    /// The creature walked one step towards the work.
    Walking,
    /// There is no way through to the work: the reference's `reason.no_path` (`activity_item_handling.cpp:3380-3385`), which skips the spot rather than ending the job.
    NoPath,
}

/// Spends one turn of a job: look for work, end if there is none, walk towards it if there is, and stand ready when the creature has arrived.
///
/// Reference: `generic_multi_activity_handler` (`activity_item_handling.cpp:3334-3420`), which runs on every turn of the activity. Its steps, in order: collect the work spots
/// (the zones of the activity's kind), sort them by distance, and for the nearest one that is **not** where the creature stands, route to it, set the destination and spend the
/// creature's moves (`route_adjacent` + `set_moves( 0 )`, `:3380-3388`). A spot the creature is **on** is where the work happens.
///
/// The port's version walks the step itself through [`crate::routing::walk_towards`], which asks the routing for one step towards the target - the reference's own shape, since its
/// NPCs re-route each turn rather than keeping a path.
///
/// **Named gaps**: the work itself when the creature has arrived (`Ready`), the per-activity `can_do_activity_there` checks, the tool-fetch errand, and the reference's
/// "skip this spot and try the next" when one is unreachable - the port answers `NoPath` after the nearest spot rather than walking the rest of the list.
pub fn work_a_turn(
    world: &mut bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
    faction: &str,
    bounds: (i32, i32),
) -> TurnOfWork {
    if the_job_has_nothing_to_do(world, npc, faction) {
        return TurnOfWork::Ended;
    }
    let Some(activity) = world
        .get::<crate::creature::Doing>(npc)
        .map(|doing| doing.activity.clone())
    else {
        return TurnOfWork::NotAJob;
    };
    if zone_for_activity(&activity).is_none() {
        return TurnOfWork::NotAJob;
    }
    let Some(work) = next_work_square(world, npc, faction) else {
        return TurnOfWork::Ended;
    };
    let Some(place) = world.get::<crate::Position>(npc).copied() else {
        return TurnOfWork::NotAJob;
    };
    if (place.x, place.y, place.z) == work {
        return match start_the_work_here(world, npc, (place.x, place.y, place.z)) {
            Some(()) => TurnOfWork::Started,
            None => TurnOfWork::Ready,
        };
    }
    // **The job decides the direction; the turn takes the step.** The reference's activity ends its turn with `set_destination` and lets the walk happen in the same creature
    // phase; the port writes a `WantsToStep`, which the walk stage of the turn consumes. Stepping here would move the creature before the turn's own step rules had a say.
    match crate::routing::step_towards(world, npc, work, bounds) {
        Some((dx, dy)) => {
            world
                .entity_mut(npc)
                .insert(crate::walk::WantsToStep { dx, dy });
            TurnOfWork::Walking
        }
        None => TurnOfWork::NoPath,
    }
}

/// When a person is standing on their work square, finds the thing to work on and **starts that work**: one activity's worth of the reference's
/// `can_do_activity_there` + activity assignment (`activity_item_handling.cpp`, the butchering branch).
///
/// Only butchering is ported, because it is the one activity whose finish the port already has (`creature::finish_butchering`, with `butcher_time_to_cut` and the harvest table
/// behind it). The others - mining, mopping, chopping, farming, fishing, disassembly - answer `None`, which is [`TurnOfWork::Ready`]: standing where the work is, with nothing
/// ported to do. That is a named gap and reads as one.
///
/// Reference: `butcher_time_to_cut( state )` gives the activity's length; the state is the corpse's size (from the monster table), the action the corpse carries (full butchering
/// unless a `Butchery` says otherwise), the best cutting tool in reach, and the butcher's **practice** in the three butchery proficiencies - the same terms the finish reads.
/// **Helpers are zero**, because the port has no "helping" state on a creature - a gap, not a chosen simplification.
fn start_the_work_here(
    world: &mut bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
    square: (i32, i32, i32),
) -> Option<()> {
    let corpse = {
        let mut query = world.query::<(
            bevy_ecs::prelude::Entity,
            &crate::Position,
            &crate::CorpseOf,
        )>();
        let mut found = None;
        for (entity, place, _) in query.iter(world) {
            if (place.x, place.y, place.z) == square {
                found = Some(entity);
                break;
            }
        }
        found
    };
    let corpse = corpse?;
    let action = world
        .get::<crate::Butchery>(corpse)
        .map_or(crate::ButcherAction::Full, |work| work.action);
    let quality = if action == crate::ButcherAction::Dissect {
        "CUT_FINE"
    } else {
        "BUTCHER"
    };
    let tool_factor = crate::creature::best_quality(world, npc, quality);
    let size = world
        .get::<crate::CorpseOf>(corpse)
        .map_or(crate::CreatureSize::Medium, |body| {
            crate::monster_registry::size_of(world, body.kind)
        });
    let moves = crate::butcher_time_to_cut(crate::ButcherState {
        size,
        action,
        tool_factor,
        field_dressed: false,
        field_dress_failed: false,
        quartered: false,
        instant_bleed: false,
        butchering_basic: crate::creature::practised_fraction(world, npc, "prof_butchering_basic"),
        butchering_advanced: crate::creature::practised_fraction(
            world,
            npc,
            "prof_butchering_advanced",
        ),
        skinning_basic: crate::creature::practised_fraction(world, npc, "prof_skinning_basic"),
        // **No helpers yet**: the reference counts the people adjacent to the corpse who are helping (`butchery.cpp`'s helpers term). The port has no "helping" state on a
        // creature, so the term is zero and the cut is the butcher's alone.
        helpers: 0,
    });
    world.entity_mut(npc).insert(crate::creature::Doing {
        activity: "butcher".to_owned(),
        moves_left: moves,
        basis: crate::creature::Basis::Time,
        moves_total: moves,
        subject: Some(corpse),
    });
    Some(())
}
