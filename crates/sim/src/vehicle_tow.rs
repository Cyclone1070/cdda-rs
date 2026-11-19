//! **Which end of a vehicle its tow cable leaves from**, and the link between two vehicles.
//!
//! Reference: `vehicle::set_tow_directions` (`vehicle.cpp:7231-7251`) and `towing_data::set_towing` (`vehicle.cpp:7253-7263`). The reference stores the link as **two raw pointers** -
//! `towing_data::towing` and `towed_by` - which is the one thing this port must never import. Here the link is an entity id, and a vehicle with no link simply has no [`Towing`]
//! component: absence is not a null pointer, it is not being towed.
//!
//! The side is pure geometry over the vehicle's own mounts, so it is a free function with no world in it: the rule can be checked against the reference's branches without building a
//! vehicle at all.

use bevy_ecs::prelude::*;

use crate::vehicle_angle::{angle_from_target, turn_from_angle};

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

/// **What lets a vehicle steer and accelerate itself** - `vehicle::selfdrive`'s opening guard (`vehicle_move.cpp:1322-1329`): an untowed vehicle with no magic, no harnessed animal and no
/// autopilot may not.
///
/// The port has only the towing term today; the other three are named inputs rather than invented ones, so a caller must say which it claims.
// **Four independent facts about one vehicle**, which a bit set would hide: the reference asks `is_towed()`, `magic`, `get_harnessed_animal()` and `has_part()` separately, and a caller
// that has to name them is a caller that has thought about which it means.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelfDrive {
    /// The vehicle is being towed - `is_towed()`.
    pub towed: bool,
    /// It drives by magic - `vehicle::magic`.
    pub magic: bool,
    /// An animal is harnessed to it - `get_harnessed_animal( here )`.
    pub harnessed: bool,
    /// It carries an `AUTOPILOT` part - `has_part( "AUTOPILOT" )`.
    pub autopilot: bool,
}

impl SelfDrive {
    /// Whether anything at all may drive this vehicle itself.
    #[must_use]
    pub fn allowed(&self) -> bool {
        self.towed || self.magic || self.harnessed || self.autopilot
    }
}

/// **Drives a vehicle that is being towed** - the towed half of `vehicle::selfdrive` (`vehicle_move.cpp:1322-1355`): a non-zero turn turns the wheels by
/// `turn × steer_increment`, and a non-zero acceleration presses the accelerator by `-acceleration`, because the reference's `accel_y` is **minus** the direction it passes to `thrust`.
///
/// **Named, not guessed:** the reference's `selfdrive` also refuses to steer when `steering_effectiveness` is zero, rounds a side-hitched hull's move through `line_to` and
/// `move_vehicle` (the skid branch in `do_towing_move`, not here), and shakes a skidding hull loose with `handling_difficulty × rng( 1, 10 ) < 15`. The port has no steering-effectiveness
/// model, no skid release and no magic or animal drive, so those branches are absent rather than approximated.
pub fn selfdrive(world: &mut World, vehicle: Entity, turn: i32, acceleration: i32) -> bool {
    let capabilities = world
        .get::<Towing>(vehicle)
        .map_or(SelfDrive::default(), |held| SelfDrive {
            towed: held.towed_by.is_some(),
            ..SelfDrive::default()
        });
    if !capabilities.allowed() {
        return false;
    }
    if turn != 0 {
        let degrees = turn * crate::vehicle_move::STEER_INCREMENT_DEGREES;
        crate::vehicle_move::turn(world, vehicle, degrees);
    }
    if acceleration != 0 {
        crate::vehicle_move::thrust_with_rolls(
            world,
            vehicle,
            -acceleration,
            &mut cdda_core_types::rng::Rolls::new(0),
        );
    }
    true
}

/// **Where a side-hitched towed hull is dragged to** - `do_towing_move`'s `default` branch (`vehicle.cpp:7141-7152`), which is the branch for a cable that leaves the hull's **side**:
/// the hull skids, and it is moved one square toward the tower each turn.
///
/// The reference computes it as `tower_tow_point - nearby_destination`, where `nearby_destination` is the **first step** of a line from the towed hull's tow point to the tower's, then
/// clamps each component into `[-1, 1]`. Over a distance that is one square toward the tower; at the same square it is zero, and the hull stays put.
#[must_use]
pub fn side_hitch_step(tower: (i32, i32), nearby: (i32, i32)) -> (i32, i32) {
    let dx = (tower.0 - nearby.0).clamp(-1, 1);
    let dy = (tower.1 - nearby.1).clamp(-1, 1);
    (dx, dy)
}

/// **The first square of the path from one point to another**, which is what the side-hitch step subtracts from - the reference's `line_to( ... )[1]`, guarded by
/// `lineto.size() >= 2` and falling back to the tower's own square when the line has no second cell.
#[must_use]
pub fn nearby_destination(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    let line = cdda_core_types::line::line_to(
        cdda_core_types::line::Point::new(from.0, from.1),
        cdda_core_types::line::Point::new(to.0, to.1),
        0,
    );
    line.get(1).map_or(to, |step| (step.x, step.y))
}

/// **Drags a side-hitched hull one square toward its tower on the map**, which is `do_towing_move`'s `default` branch applied (`vehicle.cpp:7141-7152`): the hull is marked
/// **skidding** and moved by [`side_hitch_step`] through the port's own vehicle displacement, so its old squares leave the index and its new ones enter it.
///
/// Returns whether the hull moved; a hull already on the tower's square is left alone rather than moved by nothing.
pub fn drag_side_hitched(world: &mut World, tower: (i32, i32), hull: Entity) -> bool {
    let Some(at) = world.get::<crate::Position>(hull).map(|pos| (pos.x, pos.y)) else {
        return false;
    };
    let step = side_hitch_step(tower, nearby_destination(at, tower));
    if step == (0, 0) {
        return false;
    }
    // **The skid is set before the move**, as the reference sets it before `move_vehicle`: the move reads the flag to decide that the facing follows `turn_dir`.
    if let Some(mut held) = world.get_mut::<crate::vehicles::Vehicle>(hull) {
        held.skidding = true;
    }
    crate::vehicles::move_vehicle(world, hull, step)
}

/// **The turn each cable applies to the hull behind it, once per turn**, which is `map::move_vehicle`'s towing block (`map.cpp:998-1004`) applied to every tower:
///
/// - a cable with no slack (`no_towing_slack`) and a tower that is not moving does nothing, which is the reference's `velocity <= 0` guard;
/// - the **angle** is measured from the towed hull's facing to the tower's square ([`angle_from_target`]), the **turn** is [`turn_from_angle`] with `reverse` set when the cable leaves
///   the towed hull's back, and the **pull** is [`towing_pull`];
/// - a front- or back-hitched hull is driven by [`selfdrive`] with the tower's own velocity assigned first, exactly as `do_towing_move` assigns `towed_veh->velocity` and then calls it;
/// - a side-hitched hull is dragged one square toward the tower by [`drag_side_hitched`] and marked skidding.
///
/// **Named, not guessed:** the reference measures between the two vehicles' **tow parts** (`abs_part_pos( get_tow_part() )`). The port has no per-part square yet, so it measures
/// between the two hulls' origins, which is the same square for a hull whose cable leaves its centre and a named approximation for every other. The reference also cancels the tow
/// when either vehicle's tow part is missing and snaps a cable at twenty-five squares; the snapping is a caller's job here because it needs the map to remove the cable item.
///
/// Returns how many cables pulled something.
pub fn tow_vehicles(world: &mut World) -> usize {
    let mut links = Vec::new();
    let mut query = world.query::<(Entity, &Towing)>();
    for (entity, held) in query.iter(world) {
        if let Some(pulled) = held.towing {
            links.push((entity, pulled, held.direction));
        }
    }
    let mut pulls = 0;
    for (tower, drawn, direction) in links {
        let Some(tower_pos) = world
            .get::<crate::Position>(tower)
            .map(|pos| (pos.x, pos.y))
        else {
            continue;
        };
        let Some(drawn_pos) = world
            .get::<crate::Position>(drawn)
            .map(|pos| (pos.x, pos.y))
        else {
            continue;
        };
        let distance = cdda_core_types::light::rl_dist(drawn_pos, tower_pos, false);
        // **A cable pulled past twenty-five squares snaps** - `tow_cable_too_far` (`vehicle.cpp:7323-7352`) asked at the end of `map::move_vehicle` (`map.cpp:1000-1003`), which cuts
        // the pair and says so.
        if tow_cable_too_far(distance) {
            invalidate_towing(world, tower);
            continue;
        }
        if !no_towing_slack(distance) {
            continue;
        }
        let tower_velocity = world
            .get::<crate::vehicles::Vehicle>(tower)
            .map_or(0, |built| built.velocity);
        if tower_velocity <= 0 {
            continue;
        }
        let reverse = direction == TowPoint::Back;
        if direction == TowPoint::Side {
            if drag_side_hitched(world, tower_pos, drawn) {
                pulls += 1;
            }
            continue;
        }
        let facing = world
            .get::<crate::vehicles::Vehicle>(drawn)
            .map_or(0.0, |built| built.rotation);
        let rel = (
            f64::from(tower_pos.0 - drawn_pos.0),
            f64::from(tower_pos.1 - drawn_pos.1),
        );
        let angle = angle_from_target(facing_vector(facing), rel);
        let turn = turn_from_angle(angle, false, reverse);
        let drawn_velocity = world
            .get::<crate::vehicles::Vehicle>(drawn)
            .map_or(0, |built| built.velocity);
        let pull = towing_pull(distance, tower_velocity, drawn_velocity, reverse);
        if let Some(mut held) = world.get_mut::<crate::vehicles::Vehicle>(drawn) {
            held.velocity = pull.velocity;
        }
        selfdrive(world, drawn, turn, pull.accel_y);
        pulls += 1;
    }
    pulls
}

/// **A hull's facing as a unit vector**, in the port's own convention: `rotation` is degrees clockwise from **east**, the same way the hull's own drawing and `move` are measured.
fn facing_vector(rotation: f64) -> (f64, f64) {
    let radians = rotation.to_radians();
    (radians.cos(), -radians.sin())
}

/// **Cuts a cable** - `vehicle::invalidate_towing` (`vehicle.cpp:7265-7320`), with `first_vehicle` set: if this vehicle is towing, or being towed, the **other end is cut first**, then this
/// end, so neither vehicle is left pointing at the other.
///
/// Reference's own order: `other_veh->invalidate_towing( here )` runs before this vehicle's tow part is removed and its data cleared, which is what makes the pair break cleanly rather
/// than one side at a time.
///
/// **Named, not guessed**: the reference also removes the cable's **part** and drops its item on the ground, with the link state recorded on the item so that re-attaching remembers which
/// end it came from. The port has no tow-cable part and no linked items yet, so the cable's item is not dropped - a cable in the port is a link between two vehicles, and cutting it
/// clears both ends.
///
/// Returns whether there was a cable to cut; a vehicle with no link answers `false` and is untouched.
pub fn invalidate_towing(world: &mut World, vehicle: Entity) -> bool {
    let Some(held) = world.get::<Towing>(vehicle).copied() else {
        return false;
    };
    if held.towing.is_none() && held.towed_by.is_none() {
        return false;
    }
    // **The other end first**, exactly as the reference cuts it, and without recursion into a cable that no longer exists: clearing this vehicle's component below already
    // takes the far side's pointer with it, so one clear is enough.
    if let Some(other) = held
        .towing
        .or(held.towed_by)
        .filter(|other| world.get::<Towing>(*other).is_some())
    {
        clear_towing(world, other);
    }
    clear_towing(world, vehicle);
    true
}
