//! The radiation a square carries, as a component on the square.
//!
//! Reference: `submap::get_radiation` and `set_radiation` (`src/submap.h:166-176`) and
//! `map::adjust_radiation` (`src/map.cpp:4893-4904`), which reads the square's value, adds a delta
//! and writes it back. The reference stores it in a per-square array; this port puts it on the thing
//! standing there, because how much radiation a place carries is a property of the place.
//!
//! A square that returns to zero carries nothing: the component is taken off rather than left at
//! zero, so "this square is hot" and "this square is not" are the same question.

use crate::Position;
use bevy_ecs::prelude::*;

/// How much radiation one square carries, in the reference's units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Radiation {
    /// The dose, which is never negative in the reference's own use.
    pub dose: i32,
}

/// The radiation on a square, zero when it carries none.
#[must_use]
pub fn dose_at(world: &World, square: (i32, i32, i32)) -> i32 {
    entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<Radiation>(*entity).map(|held| held.dose))
        .unwrap_or(0)
}

/// Sets a square's radiation to a dose, as `map::set_radiation` does (`mapgen.cpp:5676-5679`): an absolute write, not an adjustment, and a dose of zero takes the component
/// off through [`adjust`].
pub fn set(world: &mut World, square: (i32, i32, i32), dose: i32) {
    adjust(world, square, dose - dose_at(world, square));
}

/// Adds to a square's radiation, as `map::adjust_radiation` does.
///
/// A square left at zero loses the component, so an untouched map holds nothing and a square that has
/// been cleaned up costs nothing either.
pub fn adjust(world: &mut World, square: (i32, i32, i32), delta: i32) {
    let total = dose_at(world, square) + delta;
    let standing = entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Radiation>(*entity).is_some());
    match (standing, total) {
        (Some(entity), 0) => {
            world.despawn(entity);
        }
        (Some(entity), total) => {
            if let Some(mut held) = world.get_mut::<Radiation>(entity) {
                held.dose = total;
            }
        }
        (None, 0) => {}
        (None, total) => {
            let entity = world
                .spawn((
                    Position {
                        x: square.0,
                        y: square.1,
                        z: square.2,
                    },
                    Radiation { dose: total },
                ))
                .id();
            crate::singletons::index_mut(world).insert(entity, square);
        }
    }
}

/// How many squares carry radiation, which is what lets a gate say it compared something.
#[must_use]
pub fn squares_with_radiation(world: &World) -> usize {
    let mut query = world.try_query::<&Radiation>();
    let Some(query) = query.as_mut() else {
        return 0;
    };
    query.iter(world).count()
}

/// The entities standing at a square, in the order the index holds them.
fn entities_at(world: &World, square: (i32, i32, i32)) -> &[Entity] {
    match crate::singletons::index(world) {
        Some(index) => index.at_level(square.0, square.1, square.2),
        None => &[],
    }
}
