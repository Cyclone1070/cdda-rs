//! Who owns what: the name a faction leaves on a thing, and the one rule that writes it.
//!
//! The reference keeps an owner as a `faction_id` on an item and on a vehicle (`item::set_owner`, `vehicle::set_owner`), and a mapgen's `faction_owner` member stamps it over a
//! rectangle (`map::apply_faction_ownership`, `mapgen.cpp:7355-7371`). Ownership is a **name**, not a pointer: the faction system proper (relations, camps, missions) is a later
//! phase, and nothing here needs it - what a later rule asks is "is this yours", which is a string comparison.

use bevy_ecs::prelude::*;

/// The faction a thing belongs to, by id.
///
/// Reference: `item::owner` (`item.h`) and `vehicle::owner`, both a `faction_id`. A thing with no owner is not "owned by nobody" in the data's sense - it simply has no
/// component, which is the port's rule for every absence.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Owner(pub String);

/// Says who owns a thing, answering whether it already had an owner.
pub fn set_owner(world: &mut World, entity: Entity, faction: &str) -> bool {
    let had = world.get::<Owner>(entity).is_some();
    world.entity_mut(entity).insert(Owner(faction.to_owned()));
    had
}

/// Who owns a thing, or `None` when nobody does.
#[must_use]
pub fn owner_of(world: &World, entity: Entity) -> Option<String> {
    world.get::<Owner>(entity).map(|owner| owner.0.clone())
}

/// Stamps a faction's ownership on everything standing on one square.
///
/// Reference: `map::apply_faction_ownership` (`mapgen.cpp:7355-7371`): every **item** on the square is stamped, and the **vehicle** there is stamped only when it has no owner
/// yet - a car that already belongs to somebody keeps them.
pub fn own_the_square(world: &mut World, square: (i32, i32, i32), faction: &str) {
    for item in crate::items_at(world, (square.0, square.1)) {
        set_owner(world, item, faction);
    }
    let vehicle = crate::square::entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<crate::vehicles::Vehicle>(*entity).is_some());
    if let Some(vehicle) = vehicle
        && world.get::<Owner>(vehicle).is_none()
    {
        set_owner(world, vehicle, faction);
    }
}

/// Stamps a faction's ownership on every square of a rectangle.
///
/// Reference: `map::apply_faction_ownership( p1, p2, id )` walks `points_in_rectangle` - the whole rectangle, not a rolled corner of it.
pub fn own_the_rectangle(
    world: &mut World,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    z: i32,
    faction: &str,
) {
    for x in x0.min(x1)..=x0.max(x1) {
        for y in y0.min(y1)..=y0.max(y1) {
            own_the_square(world, (x, y, z), faction);
        }
    }
}

/// How many things a faction owns, which is what a census or a test asks.
#[must_use]
pub fn count_owned_by(world: &mut World, faction: &str) -> usize {
    let mut query = world.query::<&Owner>();
    query.iter(world).filter(|owner| owner.0 == faction).count()
}
