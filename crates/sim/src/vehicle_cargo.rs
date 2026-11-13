//! **What a vehicle part carries.**
//!
//! Reference: `vehicle_stack` (`vehicle.h:178-190`), which is an `item_stack` bound to one `vehicle_part`, and the three volume questions it answers - `max_volume`
//! (`vehicle.cpp:216-224`), `stored_volume` (`vehicle.cpp:226-236`) and `item_stack::free_volume` (`item_stack.cpp:163-166`). `vehicle::add_item` (`vehicle.cpp:6278-6320`)
//! is the rule that asks them before an item may go in, and it is the rule [`stow_item`] ports: a broken part takes nothing, and a part takes nothing that does not fit.
//!
//! **The item is an entity here, as everywhere else in the port.** Its data is a [`GroundItem`] - the name is the one the store already had for an item - and its place is an
//! [`InVehiclePart`] component naming the vehicle and the part, so nothing holds a second list of what a part contains and the query is the only index.

use bevy_ecs::prelude::*;

use crate::vehicles::{part_spec, Vehicle};
use crate::{GroundItem, ItemTypeId};

/// The most a part may hold, whatever its size says.
///
/// Reference: `std::min( vpi.size, 10000_liter )` in `vehicle_stack::max_volume` (`vehicle.cpp:220-223`), a cap the comment says exists to keep a huge cargo space from
/// overflowing the arithmetic. **No part in the current data reaches it** - the largest cargo part is 991,000 ml - so the cap is carried for the rule's sake, not because a
/// dump exercises it.
pub const MAX_CARGO_VOLUME_ML: i64 = 10_000_000;

/// The most items one part may hold.
///
/// Reference: `MAX_ITEM_IN_VEHICLE_STORAGE`, which is `MAX_ITEM_IN_SQUARE` (`game_constants.h:31-33`), asked by `item_stack::amount_can_fit` (`item_stack.cpp:106-107`).
/// **Named, not exercised:** the gate fills a basket by volume long before it reaches 4,096 items, so the count branch has no test yet.
pub const MAX_ITEMS_IN_PART: usize = 4096;

/// An item held **in** a vehicle part.
///
/// The reference keeps a `cata::colony<item>` inside each `vehicle_part`; the port keeps every item as an entity and gives it this component instead, naming the vehicle and
/// the part it rides in. The part number is the index into [`Vehicle::parts`](crate::vehicles::Vehicle), the same number every other part rule takes.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InVehiclePart {
    /// The vehicle whose part holds it.
    pub vehicle: Entity,
    /// Which of that vehicle's parts, by index.
    pub part: usize,
}

/// What the part holds, as entities - the reference's `vehicle_stack`'s `items`.
#[must_use]
pub fn items_in_part(world: &World, vehicle: Entity, part: usize) -> Vec<Entity> {
    let Some(mut query) = world.try_query::<(Entity, &InVehiclePart)>() else {
        return Vec::new();
    };
    query
        .iter(world)
        .filter(|(_, held)| held.vehicle == vehicle && held.part == part)
        .map(|(entity, _)| entity)
        .collect()
}

/// How much the part can hold, in millilitres - the reference's `vehicle_stack::max_volume` (`vehicle.cpp:216-224`).
///
/// Zero unless the part is cargo and is not broken, and never more than [`MAX_CARGO_VOLUME_ML`]. A part that is not cargo can still be asked, and answers zero - which is why
/// a seat refuses to hold anything.
#[must_use]
pub fn cargo_capacity(world: &World, vehicle: Entity, part: usize) -> i64 {
    let Some(spec) = part_spec(world, vehicle, part) else {
        return 0;
    };
    let broken = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(part))
        .is_none_or(|held| held.broken);
    if broken || !spec.flags.contains("CARGO") {
        return 0;
    }
    spec.size.min(MAX_CARGO_VOLUME_ML)
}

/// How much the part already holds, in millilitres - the reference's `vehicle_stack::stored_volume` (`vehicle.cpp:226-236`).
///
/// An item takes its kind's volume times its charge count, which is how the rest of the store measures a stack.
///
/// **Named:** the reference answers a part's **full** capacity when a piece of furniture is tied into it (`tied_down_furniture`), because the furniture is in the way. The
/// port has no furniture tied into a part yet.
#[must_use]
pub fn stored_volume(world: &World, vehicle: Entity, part: usize) -> i64 {
    items_in_part(world, vehicle, part)
        .into_iter()
        .filter_map(|item| world.get::<GroundItem>(item))
        .map(|item| crate::item_registry::volume_in(world, item.kind) * i64::from(item.charges.max(1)))
        .sum()
}

/// How much room the part has left, in millilitres - the reference's `item_stack::free_volume` (`item_stack.cpp:163-166`).
///
/// **It is not clamped**, exactly as the reference is not: a part that holds more than it can (which only an older, larger capacity could have allowed) reports a negative
/// number rather than pretending to be empty.
#[must_use]
pub fn free_volume(world: &World, vehicle: Entity, part: usize) -> i64 {
    cargo_capacity(world, vehicle, part) - stored_volume(world, vehicle, part)
}

/// Puts an item in a vehicle part, when the part takes it - the reference's `vehicle::add_item` (`vehicle.cpp:6278-6320`).
///
/// Refused when the part is broken, when it is not cargo, when it is full, or when the item is bigger than what is left. An item of **zero** volume fits anything, which is
/// the reference's `charges_per_volume` answering `INFINITE_CHARGES` for a zero-volume item (`item.cpp:1394-1411`).
///
/// **Named, not guessed:** the reference refuses ammunition that does not match a gun part's calibre, merges a charge stack into one already in the part, spills a bucket's
/// contents, and stops at 4,096 items per part. The port has no gun-in-part loading, no stack merging and no count branch test yet.
pub fn stow_item(
    world: &mut World,
    vehicle: Entity,
    part: usize,
    kind: ItemTypeId,
    charges: i32,
) -> Option<Entity> {
    let broken = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(part))
        .is_none_or(|held| held.broken);
    if broken {
        return None;
    }
    if items_in_part(world, vehicle, part).len() >= MAX_ITEMS_IN_PART {
        return None;
    }
    let volume = crate::item_registry::volume_in(world, kind) * i64::from(charges.max(1));
    if volume > 0 && volume > free_volume(world, vehicle, part) {
        return None;
    }
    Some(
        world
            .spawn((
                GroundItem {
                    kind,
                    charges,
                    age: 0,
                    burnt: 0,
                    damage: 0,
                    burn: None,
                },
                InVehiclePart { vehicle, part },
            ))
            .id(),
    )
}

/// Takes an item out of the part that holds it - the reference's `vehicle::remove_item` (`vehicle.cpp:6321-6340`) and the `vehicle_stack::erase` that calls it
/// (`vehicle.cpp:200-203`).
///
/// The part is **not asked whether it is broken**: the reference erases from the part's colony either way, which is how a wreck is salvaged. The answer says whether the entity
/// was held in a part at all, so a caller cannot destroy something it did not own - the reference's caller copies the item out before erasing it.
pub fn remove_item(world: &mut World, item: Entity) -> bool {
    if world.get::<InVehiclePart>(item).is_none() {
        return false;
    }
    world.despawn(item)
}
