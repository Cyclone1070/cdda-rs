//! Which overmap tiles belong to a city.
//!
//! Reference: `overmap::city_tiles` (`overmap.cpp:4228-4236`), the set the overmap generator fills as it lays a city out, and which `is_in_city` answers from by simple
//! membership. The port keeps it as one component on one entity - the same shape as every other store - so a question about a city is a set lookup and not a walk over
//! the map. **No city exists in a world the generator has not run**: the set is empty, and membership answers `false` the way the reference's legacy path does.

use bevy_ecs::prelude::{Component, Entity, With, World};

/// The overmap tiles a city covers, as (x, y) in overmap tiles.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct CityTiles(pub std::collections::BTreeSet<(i32, i32)>);

/// Marks the one entity that holds the world's city tiles.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheCityTiles;

/// Replaces the city's tiles, as the generator hands them over.
pub fn set_tiles(world: &mut World, tiles: impl IntoIterator<Item = (i32, i32)>) {
    let holder = crate::singletons::singleton::<CityTiles, TheCityTiles>(world);
    if let Some(mut held) = world.get_mut::<CityTiles>(holder) {
        held.0 = tiles.into_iter().collect();
    }
}

/// Whether an overmap tile belongs to a city.
///
/// Reference: `overmap::is_in_city` (`overmap.cpp:4228-4236`): membership of the tile set, and `false` when no city has been laid out.
#[must_use]
pub fn is_in_city(world: &World, tile: (i32, i32)) -> bool {
    let Some(holder) = holder(world) else {
        return false;
    };
    world
        .get::<CityTiles>(holder)
        .is_some_and(|held| held.0.contains(&tile))
}

/// The entity holding the city tiles, when one exists.
fn holder(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheCityTiles>>()?;
    query.iter(world).next()
}
