//! Whether a field may spread into a square.
//!
//! Reference: `map::propagate_field` (`map_field.cpp:2264-2270`) asks two questions of a square it is about to enter - is it
//! `impassable`, and if so does the terrain carry `TFLAG_PERMEABLE`. A wall stops smoke and lets nothing through; a screen door
//! carries `PERMEABLE` and the smoke passes.
//!
//! The answer lives on the square, like the fire facts do, because the rule needs it per square and the store knows terrain by
//! entity. A square nobody told is open, which is the neutral answer: no component, nothing to stop anything.

use bevy_ecs::prelude::*;

use crate::Position;

/// What a square says about a field trying to enter it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Component)]
pub struct FieldPassage {
    /// The terrain or furniture cannot be walked into.
    pub impassable: bool,
    /// The terrain carries `PERMEABLE`, so gases pass even though nothing can walk.
    pub permeable: bool,
}

/// Writes what a square answers about a field entering it.
pub fn set_passage(world: &mut World, square: (i32, i32, i32), passage: FieldPassage) {
    let standing = {
        let mut query = world.query::<(Entity, &Position, &FieldPassage)>();
        query
            .iter(world)
            .find(|(_, at, _)| (at.x, at.y, at.z) == square)
            .map(|(entity, _, _)| entity)
    };
    if let Some(entity) = standing {
        if let Some(mut held) = world.get_mut::<FieldPassage>(entity) {
            *held = passage;
        }
        return;
    }
    world.spawn((
        Position {
            x: square.0,
            y: square.1,
            z: square.2,
        },
        passage,
    ));
}

/// Whether a field may enter a square.
///
/// The reference's answer is `!impassable || permeable` (`map_field.cpp:2266`), and a square that has said nothing is open.
#[must_use]
pub fn may_enter(world: &World, square: (i32, i32, i32)) -> bool {
    let Some(mut query) = world.try_query::<(&Position, &FieldPassage)>() else {
        return true;
    };
    query
        .iter(world)
        .find(|(at, _)| (at.x, at.y, at.z) == square)
        .is_none_or(|(_, passage)| !passage.impassable || passage.permeable)
}

/// Every square near a centre that a field cannot enter.
///
/// The spread needs its answers before it starts changing the store, because placing a field is a write and asking the store a
/// question at the same time is not something one `&mut World` allows. The radius that matters is the amount being spread,
/// since a square costs at least one unit and nothing can travel further than the units available.
#[must_use]
pub fn blocked_within(
    world: &World,
    centre: (i32, i32, i32),
    radius: i32,
) -> std::collections::BTreeSet<(i32, i32, i32)> {
    let mut blocked = std::collections::BTreeSet::new();
    let Some(mut query) = world.try_query::<(&Position, &FieldPassage)>() else {
        return blocked;
    };
    for (at, passage) in query.iter(world) {
        let square = (at.x, at.y, at.z);
        if passage.impassable && !passage.permeable && (square.0 - centre.0).abs() <= radius && (square.1 - centre.1).abs() <= radius {
            blocked.insert(square);
        }
    }
    blocked
}
