//! What the ground answers about fire, as a component on the square.
//!
//! The reference keeps these per square in the map's caches and hands the fire rules a map
//! (`map_field.cpp` reads terrain and furniture flags through `map`). This port puts the answers on
//! the square itself, so a rule asks the store rather than a table the caller had to build.
//!
//! The facts are written once per turn by [`set_fire_facts`], from whatever the world's own data says
//! about the terrain and furniture standing there; a square nobody told holds the neutral answer.

use crate::{Position, TerrainFacts};
use bevy_ecs::prelude::*;

/// The fire-relevant answers about one square.
#[derive(Debug, Clone, Copy, Default, PartialEq, Component)]
pub struct FireFacts {
    /// The flags, in the shape the fire rules read them.
    pub facts: TerrainFacts,
}

/// Writes what a square answers about fire, adding the component if the square has none.
pub fn set_fire_facts(world: &mut World, square: (i32, i32, i32), facts: TerrainFacts) {
    let standing = entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<FireFacts>(*entity).is_some());
    if let Some(entity) = standing {
        if let Some(mut held) = world.get_mut::<FireFacts>(entity) {
            held.facts = facts;
        }
        return;
    }
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            FireFacts { facts },
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
}

/// What a square answers about fire, or the neutral answer when nobody described it.
#[must_use]
pub fn fire_facts_at(world: &World, square: (i32, i32, i32)) -> TerrainFacts {
    entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<FireFacts>(*entity).map(|held| held.facts))
        .unwrap_or_default()
}

/// The fire on a square, as its intensity and age, if one is burning there.
#[must_use]
pub fn fire_at(world: &World, square: (i32, i32, i32)) -> Option<(i32, i64)> {
    entities_at(world, square).iter().find_map(|entity| {
        world.get::<crate::Field>(*entity).and_then(|field| {
            if field.id == crate::FIRE {
                Some((field.entry.intensity(), field.entry.age()))
            } else {
                None
            }
        })
    })
}

/// How many squares carry facts, which a caller can check rather than assume.
#[must_use]
pub fn fire_facts_squares(world: &World) -> usize {
    let mut query = world.try_query::<&FireFacts>();
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
