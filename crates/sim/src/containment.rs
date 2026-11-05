//! Things inside other things.
//!
//! Reference: `item::contents` and `item::in_container` (`item.cpp`). The reference keeps an item's contents **inside** the item, which is a tree; the port keeps one
//! store, so a thing that is inside another is a **link**: the inner item carries [`ContainedIn`] pointing at the container, and a container's contents are a query.
//! Nothing about being inside a thing is duplicated on the container - the link is the only truth about where the inner thing is.
//!
//! A contained thing has **no square**. It is not on the floor, not in a pocket, not on a creature: it is in the container, and the container is the thing that is
//! somewhere. [`put_inside`] removes the square for exactly that reason, and [`take_out`] puts it back.

use bevy_ecs::prelude::*;

use crate::GroundItem;

/// A thing inside another thing.
///
/// Reference: the parent/child relation `item::contents` gives an item, kept here as a link so the inner thing stays an entity rather than becoming a field.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainedIn(pub Entity);

/// Puts a thing inside a container, taking it off the floor.
///
/// Reference: `item::put_in( container, count )` (`item.cpp`) - the thing stops being where it was and becomes part of the container. A container cannot be put
/// inside itself or inside something it already holds, which would make a cycle no walk could end.
pub fn put_inside(world: &mut World, thing: Entity, container: Entity) -> bool {
    // `holds` walks **up** from the container: a thing may not be put inside something it already contains, or the chain would become a cycle.
    if thing == container || holds(world, container, thing) {
        return false;
    }
    if world.get::<GroundItem>(thing).is_none() {
        return false;
    }
    // What is inside a container is not on the floor, so it has no square of its own.
    let square = world.get::<crate::Position>(thing).copied();
    world.entity_mut(thing).remove::<crate::Position>();
    if let Some(at) = square {
        crate::singletons::index_mut(world).remove(thing, (at.x, at.y, at.z));
    }
    world.entity_mut(thing).remove::<crate::creature::Carried>();
    world.entity_mut(thing).insert(ContainedIn(container));
    true
}

/// Takes a thing out of whatever holds it, putting it back on a square.
///
/// Reference: `item::remove_item`/`item::spill_contents` (`item.cpp`): the thing leaves the container and lands where the container is.
pub fn take_out(world: &mut World, thing: Entity, square: (i32, i32, i32)) -> bool {
    if world.get::<ContainedIn>(thing).is_none() {
        return false;
    }
    world.entity_mut(thing).remove::<ContainedIn>();
    world.entity_mut(thing).insert(crate::Position {
        x: square.0,
        y: square.1,
        z: square.2,
    });
    crate::singletons::index_mut(world).insert(thing, square);
    true
}

/// Everything directly inside a container, in the order the store holds it.
#[must_use]
pub fn contents_of(world: &World, container: Entity) -> Vec<Entity> {
    let Some(mut query) = world.try_query::<(Entity, &ContainedIn)>() else {
        return Vec::new();
    };
    let mut found: Vec<Entity> = query
        .iter(world)
        .filter(|(_, inside)| inside.0 == container)
        .map(|(entity, _)| entity)
        .collect();
    found.sort_unstable();
    found
}

/// Whether `container` is somewhere above `thing` in the chain of contents.
fn holds(world: &World, container: Entity, thing: Entity) -> bool {
    let mut current = Some(container);
    let mut steps = 0;
    while let Some(at) = current {
        if at == thing {
            return true;
        }
        steps += 1;
        if steps > 64 {
            // A chain this deep is a cycle somebody built; refusing is better than looping.
            return true;
        }
        current = world.get::<ContainedIn>(at).map(|inside| inside.0);
    }
    false
}

/// What a thing weighs, and whether its contents count.
///
/// Reference: `item::weight( include_contents, integral )` (`item.cpp:7229-7305`): the item's own weight times its charges, plus - when contents are counted - the
/// weight of everything inside it. **A container's contents are carried weight once**, which is why the contents are not also counted from the square they are not
/// standing on: a contained thing has no square of its own.
#[must_use]
pub fn weight_of(
    world: &World,
    thing: Entity,
    names: &crate::item_registry::ItemNames,
    include_contents: bool,
) -> i64 {
    let Some(held) = world.get::<GroundItem>(thing) else {
        return 0;
    };
    let mut total = names.weight_of(held.kind) * i64::from(held.charges.max(1));
    if include_contents {
        for inside in contents_of(world, thing) {
            total += weight_of(world, inside, names, true);
        }
    }
    total
}

/// Empties a container onto a square and answers how many things came out.
///
/// Reference: `item_contents::spill_contents( pos )` (`item_contents.cpp:1228-1237`), which walks the pockets and spills each one that is not marked `_no_unload`. The port walks the container's
/// **contents query** instead, which is the same set by construction - a thing is inside because it holds the link - so **spilling is taking each link out**, and "the container is empty afterwards"
/// is a consequence rather than a second step that could forget one.
///
/// **Named, not invented:** the `NO_UNLOAD` pocket exemption. The port's pocket facts carry capacity, weight and shape but not that flag, so every pocket spills today; the reference keeps the contents
/// of a `NO_UNLOAD` pocket in place.
#[must_use]
pub fn spill_contents(world: &mut World, container: Entity, square: (i32, i32, i32)) -> usize {
    let mut spilled = 0;
    for thing in contents_of(world, container) {
        if take_out(world, thing, square) {
            spilled += 1;
        }
    }
    spilled
}
