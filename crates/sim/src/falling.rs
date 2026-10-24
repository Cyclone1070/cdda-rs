//! What falls when the ground goes out from under it.
//!
//! Reference: `map::process_falling` (`src/map.cpp:3018-3035`) and `map::drop_everything` (`src/map.cpp:2693-2705`). A square whose support was destroyed in the last turn is
//! queued; the turn then asks each queued square whether it still has a floor, and if it does not, everything on it goes down: furniture, items, the vehicle, fields - in that
//! order, which matters when a thing that falls lands on a thing that fell first.
//!
//! Two of those four are here. **Items** fall to the lowest level that has a floor, not one level down - a rock above a three-storey shaft lands at the bottom - and
//! **fields** fall exactly **one** level and only if the type's data says they are cosmetic, because the reference's comment is that an active field "drops itself" through its
//! own processor. A field that lands on a hole is refused: `map::add_field` will not put a non-gaseous field on a square with no floor.
//!
//! Two are **not** here, and both are declared rather than quietly missing: **furniture** falls with a bash at the square it lands on (which drags in the bash port's own
//! declared gaps, and which `corpus/falling/fall.json` records so the next slice can be held to it), and **creatures** fall through `gravity_check`, which needs creatures.

use bevy_ecs::prelude::*;
use std::collections::BTreeSet;

use cdda_core_types::field::FieldId;

use crate::{Field, GroundItem, Position};

/// How far below street level the world goes (`OVERMAP_DEPTH`), and how far above (`OVERMAP_HEIGHT`).
///
/// The reference's `has_floor_or_water` answers **true** - there is a floor - outside these, so a thing falling in a bottomless shaft stops at the world's floor rather than
/// falling forever (`src/map.cpp:2651-2660`).
pub const OVERMAP_DEPTH: i32 = 10;
/// See [`OVERMAP_DEPTH`].
pub const OVERMAP_HEIGHT: i32 = 10;

/// The terrain questions a fall asks.
pub trait Floors {
    /// Whether the square has the `NO_FLOOR` flag: open air, a hole, the sky.
    fn no_floor(&self, at: (i32, i32, i32)) -> bool;
    /// Whether the square is inside the loaded world.
    fn in_bounds(&self, at: (i32, i32, i32)) -> bool;
}

/// Whether a square holds a thing up: `map::has_floor_or_water` (`src/map.cpp:2651-2660`).
///
/// The name is the reference's, and it answers "will something stop here", not "is there a floor tile": water counts, the bottom of the world counts, and anything outside the
/// loaded map counts. Only a square that is loaded **and** flagged `NO_FLOOR` lets a thing through.
#[must_use]
pub fn has_floor_or_water(floors: &impl Floors, at: (i32, i32, i32)) -> bool {
    if at.2 < -OVERMAP_DEPTH + 1 || at.2 > OVERMAP_HEIGHT {
        return true;
    }
    if !floors.in_bounds(at) {
        return true;
    }
    !floors.no_floor(at)
}

/// The square things fall to from `at`, or `None` when `at` already holds them up.
///
/// `map::drop_items`'s loop (`src/map.cpp:2863-2871`): step down while the square below has no floor. A thing above a three-storey shaft falls all three, which is why this
/// is a loop rather than one subtraction.
#[must_use]
pub fn landing(floors: &impl Floors, at: (i32, i32, i32)) -> Option<(i32, i32, i32)> {
    if has_floor_or_water(floors, at) {
        return None;
    }
    let mut below = at;
    while !has_floor_or_water(floors, below) {
        below.2 -= 1;
    }
    (below != at).then_some(below)
}

/// Which fields the data says fall, and which of them may land on a hole.
#[derive(Debug, Clone, Default, PartialEq, Eq, bevy_ecs::prelude::Resource)]
pub struct FieldRules {
    /// Types with `accelerated_decay`: the cosmetic fields, which are the only ones that drop (`src/map.cpp:2989`).
    pub falls: BTreeSet<FieldId>,
    /// Types whose phase is `gas`, the only phase `map::add_field` will place on a square with no floor (`src/map.cpp:6531-6534`).
    pub gas: BTreeSet<FieldId>,
}

/// Drops everything on one square that can fall: items to the floor below, cosmetic fields one level down.
///
/// Returns how many items and how many fields moved, which is what a caller has to report if it wants to say anything about a fall at all.
pub fn drop_everything(
    world: &mut World,
    floors: &impl Floors,
    rules: &FieldRules,
    at: (i32, i32, i32),
) -> (usize, usize) {
    // `if( !has_floor_or_water( p ) )` - the whole of `drop_everything` is behind this test, so a square with a floor does nothing at all.
    let Some(below) = landing_and_floor(floors, at) else {
        return (0, 0);
    };
    let items = drop_items(world, below, at);
    let fields = drop_fields(world, floors, rules, at);
    (items, fields)
}

/// Where a square's contents go, and whether that square holds them up. `None` when the square itself has a floor.
fn landing_and_floor(floors: &impl Floors, at: (i32, i32, i32)) -> Option<(i32, i32, i32)> {
    landing(floors, at)
}

/// Moves every item on a square to the square below, keeping their order.
fn drop_items(world: &mut World, below: (i32, i32, i32), at: (i32, i32, i32)) -> usize {
    let mut moved = 0;
    let mut query = world.query::<(Entity, &mut Position, &GroundItem)>();
    // **Order is the point.** The reference appends the falling items to the foot of the stack they land on, one at a time and in the order they lay there, so a port that
    // moved them in any other order would stack them differently - and the top of a stack is what a player picks up.
    let mut falling: Vec<Entity> = query
        .iter_mut(world)
        .filter(|(_, at_square, _)| (at_square.x, at_square.y, at_square.z) == at)
        .map(|(entity, _, _)| entity)
        .collect();
    falling.sort_by_key(|entity| entity.index());
    for entity in falling {
        if let Some(mut position) = world.get_mut::<Position>(entity) {
            position.x = below.0;
            position.y = below.1;
            position.z = below.2;
            moved += 1;
        }
    }
    moved
}

/// Moves the cosmetic fields on a square down exactly one level, and removes the ones that cannot land.
fn drop_fields(
    world: &mut World,
    floors: &impl Floors,
    rules: &FieldRules,
    at: (i32, i32, i32),
) -> usize {
    let below = (at.0, at.1, at.2 - 1);
    let mut moved = 0;
    let mut query = world.query::<(Entity, &mut Position, &Field)>();
    let mut falling: Vec<(Entity, FieldId)> = query
        .iter_mut(world)
        .filter(|(_, at_square, _)| (at_square.x, at_square.y, at_square.z) == at)
        .map(|(entity, _, field)| (entity, field.id))
        .filter(|(_, id)| rules.falls.contains(id))
        .collect();
    falling.sort_by_key(|(entity, _)| entity.index());
    for (entity, id) in falling {
        // `map::add_field` refuses a non-gaseous field on a square with no floor, and the reference's `drop_fields` does not check first - so a liquid that falls into a hole
        // is destroyed, not held in mid-air.
        if floors.no_floor(below) && !rules.gas.contains(&id) {
            world.despawn(entity);
            continue;
        }
        if let Some(mut position) = world.get_mut::<Position>(entity) {
            position.x = below.0;
            position.y = below.1;
            position.z = below.2;
            moved += 1;
        }
    }
    moved
}
