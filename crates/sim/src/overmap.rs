//! The overmap's terrain, as far as a rule needs to know it.
//!
//! Reference: `overmap_buffer.ter( omt_pos )` (`overmapbuffer.cpp`) answers an `oter_id` for a square of the overmap, and the `u_at_om_location`
//! guard (`condition.cpp:1023-1046`) compares it - with its direction and connections stripped - against a string the data writes.
//!
//! World generation is a declared gap. What this module holds is the **shape** of the answer: a square of the overmap has one terrain id, world
//! generation fills it, and every rule that asks reads it from there. Nothing else in the port pretends to know it, which is why it is a component
//! with a setter rather than a computed value.

use bevy_ecs::prelude::*;

/// Squares across one overmap tile.
///
/// Reference: the overmap tile is two submaps of twelve squares (`coordinate_constants`: `SEE = 2`, submap 12x12), so it is 24 squares
/// across and `project_to<coords::omt>` divides a square's coordinate by 24.
pub const SQUARES_PER_OMT: i32 = 24;

/// One terrain id per square of the overmap.
///
/// Keyed by the tile's coordinates rather than by an entity, because the overmap has no other content yet: when it grows features of its
/// own, each tile becomes an entity with this as one of its components, and this map is deleted. Keeping it a map now is what lets a
/// caller answer the question without inventing a world.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct OvermapTerrains(pub std::collections::BTreeMap<(i32, i32), String>);

/// Marks the overmap, the way every other singleton is marked.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TheOvermap;

/// Which overmap tile a square belongs to.
#[must_use]
pub const fn tile_of(x: i32, y: i32) -> (i32, i32) {
    (x.div_euclid(SQUARES_PER_OMT), y.div_euclid(SQUARES_PER_OMT))
}

/// Sets the terrain of one overmap tile, which is what world generation will do.
pub fn set_terrain(world: &mut World, tile: (i32, i32), terrain: &str) {
    let entity = crate::singletons::singleton::<OvermapTerrains, TheOvermap>(world);
    if let Some(mut terrains) = world.get_mut::<OvermapTerrains>(entity) {
        terrains.0.insert(tile, terrain.to_owned());
    }
}

/// Every tile the overmap holds with the terrain wanted, in a settled order.
#[must_use]
pub fn tiles_with_terrain(world: &World, wanted: &str) -> Vec<(i32, i32)> {
    let Some(terrains) = crate::singletons::holder::<TheOvermap>(world)
        .and_then(|holder| world.get::<OvermapTerrains>(holder))
    else {
        return Vec::new();
    };
    terrains
        .0
        .iter()
        .filter(|(_, terrain)| terrain.as_str() == wanted)
        .map(|(tile, _)| *tile)
        .collect()
}

/// The nearest tile within a radius whose terrain is the one wanted.
///
/// Reference: `mission_util::find_or_create_om_terrain` (`mission_util.cpp:321`), the search half: it walks outward from the origin and takes the first
/// tile that matches. The port **searches only**: it never generates an overmap tile to satisfy a search, because generating world is world generation's
/// job and a dialogue effect that quietly builds a forest is not a search. A tile that is not there is `None`, and the caller refuses.
#[must_use]
pub fn nearest_with_terrain(
    world: &World,
    origin: (i32, i32),
    wanted: &str,
    radius: i32,
    min_distance: i32,
) -> Option<(i32, i32)> {
    // Only a tile the overmap actually holds can match, so the candidates come from the store rather than from a ring of empty squares: a search that
    // finds nothing costs the size of the overmap, not the square of its radius.
    let placed = tiles_with_terrain(world, wanted);
    placed
        .into_iter()
        .filter_map(|tile| {
            let step = (tile.0 - origin.0).abs().max((tile.1 - origin.1).abs());
            // `min_distance` skips the tiles nearest the searcher, which is how the data asks for somewhere *away* from the player
            // (`overmapbuffer::find_closest`, `overmapbuffer.cpp:1229`).
            (step <= radius && step >= min_distance).then_some((tile, step))
        })
        // Nearest first, then the lower x and y, so the same overmap always answers the same tile.
        .min_by_key(|(tile, step)| (*step, *tile))
        .map(|(tile, _)| tile)
}

/// The terrain of one overmap tile, or nothing when nothing has placed one.
#[must_use]
pub fn terrain_at(world: &World, tile: (i32, i32)) -> Option<&str> {
    let terrains = world.get::<OvermapTerrains>(crate::singletons::holder::<TheOvermap>(world)?)?;
    terrains.0.get(&tile).map(String::as_str)
}

/// A terrain id with its direction and connections taken off.
///
/// Reference: `oter_no_dir_or_connections` (`overmap.cpp`), which the `u_at_om_location` guard compares against. The data spells rotations as
/// `field_north`, `house_east` and connections as `road_nesw`, so the base id is the part before the first spelt-out suffix - which is what a
/// guard asking for `"field"` means.
#[must_use]
pub fn base_terrain(id: &str) -> &str {
    const SUFFIXES: [&str; 12] = [
        "_north", "_south", "_east", "_west", "_northeast", "_southeast", "_southwest", "_northwest", "_nesw", "_nsw", "_senw", "_ew",
    ];
    for suffix in SUFFIXES {
        if let Some(base) = id.strip_suffix(suffix) {
            return base;
        }
    }
    id
}
