//! Putting a piece of the map back the way it was when the data asked for it.
//!
//! Reference: `f_revert_location` (`npctalk.cpp:4884-4918`) takes a **copy** of the four submaps around a place (`submap::get_revert_submap`,
//! `submap.cpp:352-362`) and schedules a `REVERT_SUBMAP` event that copies the terrain, furniture, traps and items back
//! (`submap::revert_submap`, `submap.cpp:322-347`). The copy is what makes the rule possible at all: a mapgen update can change a house while the
//! player is inside it and put it back an hour later.
//!
//! The port keeps the copy the way it keeps everything - as a component **on the square itself**. That is the whole design: a square that has a
//! [`Snapshot`] remembers what stood there, and restoring it reads the component rather than a parallel table of coordinates that would have to be
//! kept in step with the store.

use bevy_ecs::prelude::*;

use crate::square::{Square, furniture_at, set_furniture, set_terrain, terrain_at};

/// What stood on a square when the map was told to remember it.
///
/// Reference: `submap`'s `m->ter` and `m->frn` arrays, copied wholesale by `get_revert_submap`. The port keeps the two the map's own entities carry;
/// a trap and a thing lying on the floor are separate rules the port has yet to give a change of their own, and they are named in `docs/parity.md` rather
/// than quietly left out.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The terrain code that stood there, when anything stood there.
    pub terrain: Option<u32>,
    /// The furniture code that stood there, when there was furniture.
    pub furniture: Option<u32>,
}

/// How far a revert reaches, in squares: the reference copies **four submaps** of twelve by twelve, which is a twenty-four square block centred on the place.
pub const REVERT_RADIUS: i32 = 12;

/// Remembers what stands on every square within [`REVERT_RADIUS`] of a place.
///
/// Reference: `f_revert_location` (`npctalk.cpp:4898-4917`) - the copy is taken **when the effect runs**, not when it is scheduled, which is why a mapgen
/// update that happens in between is undone and one that happens after the revert is not.
///
/// Answers how many squares were remembered.
pub fn take(world: &mut World, center: (i32, i32, i32)) -> usize {
    let squares: Vec<(Entity, (i32, i32, i32))> = {
        let mut query = world.query::<(Entity, &crate::Position)>();
        query
            .iter(world)
            .filter(|(entity, position)| {
                let at = (position.x, position.y, position.z);
                world.get::<Square>(*entity).is_some()
                    && at.2 == center.2
                    && (at.0 - center.0).abs() <= REVERT_RADIUS
                    && (at.1 - center.1).abs() <= REVERT_RADIUS
            })
            .map(|(entity, position)| (entity, (position.x, position.y, position.z)))
            .collect()
    };
    for (entity, at) in &squares {
        let snapshot = Snapshot {
            terrain: terrain_at(world, *at),
            furniture: furniture_at(world, *at),
        };
        world.entity_mut(*entity).insert(snapshot);
    }
    squares.len()
}

/// Puts every remembered square back the way it was.
///
/// Reference: `timed_event.cpp:336-340` with `submap::revert_submap` (`submap.cpp:322-347`): each remembered tile's terrain and furniture are copied back, and a
/// tile that had no furniture loses the one it has. Answers how many squares were put back.
pub fn restore(world: &mut World) -> usize {
    restore_where(world, None)
}

/// Puts back **only the squares around one place**, which is what a `REVERT_SUBMAP` event does.
///
/// Reference: `timed_event.cpp:336-340` reverts the submap the event names, not the whole map: two reverts pending in different places are two different events, and
/// firing one must not fire the other.
pub fn restore_at(world: &mut World, center: (i32, i32, i32)) -> usize {
    restore_where(world, Some(center))
}

fn restore_where(world: &mut World, around: Option<(i32, i32, i32)>) -> usize {
    let remembered: Vec<(Entity, (i32, i32, i32), Snapshot)> = {
        let mut query = world.query::<(Entity, &crate::Position, &Snapshot)>();
        query
            .iter(world)
            .filter(|(_, position, _)| {
                around.is_none_or(|center| {
                    position.z == center.2
                        && (position.x - center.0).abs() <= REVERT_RADIUS
                        && (position.y - center.1).abs() <= REVERT_RADIUS
                })
            })
            .map(|(entity, position, snapshot)| {
                (entity, (position.x, position.y, position.z), *snapshot)
            })
            .collect()
    };
    for (entity, at, snapshot) in &remembered {
        if let Some(code) = snapshot.terrain {
            set_terrain(world, *at, code);
        }
        // A code of zero is how [`set_furniture`] says "nothing stands here", which is what a square that had no furniture gets back.
        set_furniture(world, *at, snapshot.furniture.unwrap_or(0));
        world.entity_mut(*entity).remove::<Snapshot>();
    }
    remembered.len()
}

/// Whether anything is waiting to be put back.
#[must_use]
pub fn has_snapshot(world: &World) -> bool {
    let mut query = world.try_query::<&Snapshot>();
    query
        .as_mut()
        .is_some_and(|query| query.iter(world).next().is_some())
}
