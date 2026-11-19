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

/// **The distance at which a slack cable starts pulling** - `vehicle::no_towing_slack` (`vehicle.cpp:7354-7385`), whose answer is `rl_dist( tower_tow_point, towed_tow_point ) >= 8`.
pub const TOW_SLACK: i32 = 8;

/// **The distance at which the cable snaps** - `vehicle::tow_cable_too_far` (`vehicle.cpp:7323-7352`), `>= 25`.
pub const TOW_SNAP: i32 = 25;

/// **The distance at which a towed vehicle is nudged forward on a slack-ish cable** - `do_towing_move` (`vehicle.cpp:7119-7121`): under six squares the towed hull is pushed rather
/// than pulled, so a reversing tower does not drive into it.
pub const TOW_PUSH: i32 = 6;

/// **The distance at which a towed vehicle is dragged along** - `do_towing_move` (`vehicle.cpp:7122-7124`): at seven or more squares a towed vehicle that is no faster than its tower is
/// pulled.
pub const TOW_STRAIN: i32 = 7;

/// **The distance at which the towed vehicle is snapped up to the cable's speed** - `do_towing_move` (`vehicle.cpp:7125-7129`): at twelve or more squares its velocity is set to the
/// tower's times 1.8, negated if the cable leaves its back.
pub const TOW_STRETCH: i32 = 12;

/// **Whether a cable is pulling at all**: `no_towing_slack` - `distance >= 8`.
#[must_use]
pub fn no_towing_slack(distance: i32) -> bool {
    distance >= TOW_SLACK
}

/// **Whether a cable has been pulled too far and will snap**: `tow_cable_too_far` - `distance >= 25`.
#[must_use]
pub fn tow_cable_too_far(distance: i32) -> bool {
    distance >= TOW_SNAP
}

/// **Whether the towed hull is being pushed rather than pulled**: `do_towing_move` sets `accel_y` at **under six** squares, which is what keeps a reversing tower off the vehicle behind
/// it. The reference's own comparison is `rl_dist( ... ) < 6`, not `<=`.
#[must_use]
pub fn towing_pushes(distance: i32) -> bool {
    distance < TOW_PUSH
}

/// **Whether the towed hull is being dragged**: at **seven or more** squares a towed vehicle no faster than its tower is pulled along (`>= 7`).
#[must_use]
pub fn towing_strains(distance: i32) -> bool {
    distance >= TOW_STRAIN
}

/// **The velocity a towed vehicle is snapped to when the cable is fully stretched** - `do_towing_move` (`vehicle.cpp:7125-7129`): the tower's velocity **times 1.8**, negated when the
/// cable leaves the towed vehicle's back. The reference multiplies an `int` by a `double` and stores it back into an `int`, so the product truncates toward zero.
#[must_use]
pub fn towed_velocity_at_stretch(tower_velocity: i32, reverse: bool) -> i32 {
    let product = f64::from(tower_velocity) * 1.8;
    #[allow(clippy::cast_possible_truncation)]
    let stretched = product as i32;
    if reverse { -stretched } else { stretched }
}

/// **What a tower's movement does to the vehicle behind it**: how hard to accelerate, and what velocity the towed hull is given - `vehicle::do_towing_move`'s arithmetic
/// (`vehicle.cpp:7119-7129`), with its three distances resolved in the reference's own order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TowingPull {
    /// How hard the towed hull is pushed along its own facing: `accel_y`, which `selfdrive` takes.
    pub accel_y: i32,
    /// The velocity the towed hull is **set to**, not added to.
    pub velocity: i32,
}

/// **The pull a tower applies to its towed vehicle**, in the reference's own order:
///
/// 1. under six squares the towed hull is **pushed** - `accel_y = reverse ? -1 : 1` - because a reversing tower must not drive into it;
/// 2. at seven or more squares a towed hull **no faster than the tower** is **dragged** - `accel_y = reverse ? 1 : -1` - and this assignment comes **after** the first, so a hull can be
///    pushed at one distance and dragged at another but never both at once;
/// 3. at twelve or more squares the towed hull's velocity is **set** to the tower's times 1.8 (negated in reverse), otherwise to the tower's velocity itself (negated in reverse).
///
/// The velocity is always **assigned**, never added: a towed vehicle does not keep its own speed, which is what makes a cable a constraint rather than a suggestion.
#[must_use]
pub fn towing_pull(
    distance: i32,
    tower_velocity: i32,
    behind_velocity: i32,
    reverse: bool,
) -> TowingPull {
    let mut accel_y = 0;
    if towing_pushes(distance) {
        accel_y = if reverse { -1 } else { 1 };
    }
    if behind_velocity <= tower_velocity && towing_strains(distance) {
        accel_y = if reverse { 1 } else { -1 };
    }
    let velocity = if distance >= TOW_STRETCH {
        towed_velocity_at_stretch(tower_velocity, reverse)
    } else if reverse {
        -tower_velocity
    } else {
        tower_velocity
    };
    TowingPull { accel_y, velocity }
}
