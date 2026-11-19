//! **Which end of a vehicle its tow cable leaves from**, and the link between two vehicles.
//!
//! Reference: `vehicle::set_tow_directions` (`vehicle.cpp:7231-7251`) and `towing_data::set_towing` (`vehicle.cpp:7253-7263`). The reference stores the link as **two raw pointers** -
//! `towing_data::towing` and `towed_by` - which is the one thing this port must never import. Here the link is an entity id, and a vehicle with no link simply has no [`Towing`]
//! component: absence is not a null pointer, it is not being towed.
//!
//! The side is pure geometry over the vehicle's own mounts, so it is a free function with no world in it: the rule can be checked against the reference's branches without building a
//! vehicle at all.

use bevy_ecs::prelude::*;

/// **Which end of a vehicle a tow cable leaves from** - the reference's `towing_point_side` (`vehicle.h:192-198`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TowPoint {
    /// `TOW_FRONT`.
    Front,
    /// `TOW_SIDE`.
    Side,
    /// `TOW_BACK`.
    Back,
}

/// **Which third of the vehicle the tow part sits in** - the reference's `vehicle::set_tow_directions` (`vehicle.cpp:7231-7251`).
///
/// The rule reads the vehicle's own bounds: its length is `mount_max.x - mount_min.x + 1`, the tow part's mount is normalised against `mount_min.x`, and - **only when the vehicle is
/// three or more mounts long** - the normalised position is compared with a third and two thirds of the length. A shorter vehicle always answers **front**, because the reference's own
/// comment says a small vehicle is no danger if it flips around.
#[must_use]
pub fn tow_direction(mount_x: i32, mount_min_x: i32, mount_max_x: i32) -> TowPoint {
    let length = mount_max_x - mount_min_x + 1;
    if length < 3 {
        return TowPoint::Front;
    }
    let trisect = length / 3;
    let normalised = (mount_x - mount_min_x).abs();
    if normalised <= trisect {
        TowPoint::Back
    } else if normalised <= trisect * 2 {
        TowPoint::Side
    } else {
        TowPoint::Front
    }
}

/// **A linked pair of vehicles**: who pulls whom, and where the pulling ends - the reference's `towing_data` (`vehicle.h:201-225`) with ids where it has pointers.
///
/// A vehicle carries this component only while it is part of a pair, so "is this vehicle towing?" is a component query and "nothing is towing it" is the absence of one, never a null.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Towing {
    /// The vehicle this one pulls, if it is the tower.
    pub towing: Option<Entity>,
    /// The vehicle that pulls this one, if it is being towed.
    pub towed_by: Option<Entity>,
    /// Which end of **this** vehicle the cable leaves from.
    pub direction: TowPoint,
}

/// **Links two vehicles**, and answers whether it could - the reference's `towing_data::set_towing` (`vehicle.cpp:7253-7263`), which refuses a missing vehicle and otherwise writes both
/// ends of the link and recomputes both directions.
///
/// The reference does not refuse a vehicle towing itself, because a raw pointer to yourself is representable and simply useless; the port does, because an entity linked to itself is a
/// loop the schedule would have to break later. The rule is named here rather than hidden: it is a departure taken deliberately against an input the reference's own callers never make.
pub fn set_towing(world: &mut World, pulling: Entity, pulled: Entity) -> bool {
    if pulling == pulled {
        return false;
    }
    if world.get::<crate::vehicles::Vehicle>(pulling).is_none()
        || world.get::<crate::vehicles::Vehicle>(pulled).is_none()
    {
        return false;
    }
    let front_side = tow_side_of(world, pulling);
    let rear_side = tow_side_of(world, pulled);
    let mut pull_state = world.get_mut::<Towing>(pulling).map_or(
        Towing {
            towing: None,
            towed_by: None,
            direction: front_side,
        },
        |held| *held,
    );
    pull_state.towing = Some(pulled);
    pull_state.direction = front_side;
    let mut drawn_state = world.get_mut::<Towing>(pulled).map_or(
        Towing {
            towing: None,
            towed_by: None,
            direction: rear_side,
        },
        |held| *held,
    );
    drawn_state.towed_by = Some(pulling);
    drawn_state.direction = rear_side;
    world.entity_mut(pulling).insert(pull_state);
    world.entity_mut(pulled).insert(drawn_state);
    true
}

/// **Breaks a pair** - the reference's `towing_data::clear_towing`, which zeroes both pointers on the vehicle it is called for. The port clears **both ends**, because a half-cleared
/// pair would leave one vehicle pointing at a stranger; and a vehicle with nothing left to record loses the component altogether.
pub fn clear_towing(world: &mut World, vehicle: Entity) {
    let Some(state) = world.get::<Towing>(vehicle).copied() else {
        return;
    };
    if let Some(mut theirs) = state
        .towing
        .and_then(|other| world.get_mut::<Towing>(other))
    {
        theirs.towed_by = None;
    }
    if let Some(mut theirs) = state
        .towed_by
        .and_then(|other| world.get_mut::<Towing>(other))
    {
        theirs.towing = None;
    }
    world.entity_mut(vehicle).remove::<Towing>();
}

/// The end of one vehicle its cable leaves from, read from its own parts.
fn tow_side_of(world: &World, vehicle: Entity) -> TowPoint {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return TowPoint::Front;
    };
    let xs: Vec<i32> = built.parts.iter().map(|part| part.x).collect();
    let Some(min_x) = xs.iter().copied().min() else {
        return TowPoint::Front;
    };
    let Some(max_x) = xs.iter().copied().max() else {
        return TowPoint::Front;
    };
    // **The tow part is the one carrying the `TOW` flag** - the reference's `get_tow_part()`. A vehicle with no such part answers front, which is what the reference answers for a
    // vehicle it cannot place a cable on.
    let tow_mount = built
        .parts
        .iter()
        .enumerate()
        .find(|(index, _)| {
            crate::vehicles::part_spec(world, vehicle, *index)
                .is_some_and(|spec| spec.flags.contains("TOW"))
        })
        .map_or(min_x, |(_, part)| part.x);
    tow_direction(tow_mount, min_x, max_x)
}
