//! The sizes of the world: a submap, the map it sits in, and the bubble that gets looked at.
//!
//! Reference: `src/map_scale_constants.h`. Every number here is read from the reference's own constants by `--dump-geometry` and pinned by a test, so a port that
//! disagrees is a red test rather than a difference nobody notices.
//!
//! Three sizes, and they are easy to confuse:
//!
//! | Name | Squares | What it is |
//! |---|---|---|
//! a **submap** | 144 | `SEEX` by `SEEY`, twelve by twelve; the unit a save file is made of |
//! the **map** | 17,424 | `MAPSIZE` squared submaps, 132 by 132; what is loaded around the player |
//! the **view** | 14,641 | `2 * MAX_VIEW_DISTANCE + 1` across, 121 by 121; what the state dump fingerprints |
//!
//! The vertical stack is 21 levels - ten below zero, zero, ten above - and it is the **outer** dimension of the submap array, so one column of the world is
//! contiguous in memory.

/// The width of a submap, and of the nonant grid inside it.
pub const SEEX: i32 = 12;
/// The height of a submap. The reference writes `SEEY = SEEX`, and so does this.
pub const SEEY: i32 = SEEX;
/// How many submaps across the map is. Odd on purpose: the player sits in the middle of one.
pub const MAPSIZE: i32 = 11;
/// How many submaps from the middle to the edge.
pub const HALF_MAPSIZE: i32 = MAPSIZE / 2;
/// The map's width in squares.
pub const MAPSIZE_X: i32 = SEEX * MAPSIZE;
/// The map's height in squares.
pub const MAPSIZE_Y: i32 = SEEY * MAPSIZE;
/// The squares in one submap.
pub const SUBMAP_ELEMENTS: usize = (SEEX * SEEY) as usize;
/// The squares in the map.
pub const MAP_TILES: usize = (MAPSIZE_X * MAPSIZE_Y) as usize;
/// How far anything can be seen, in squares: `MAPSIZE` eleven gives sixty.
pub const MAX_VIEW_DISTANCE: i32 = SEEX * HALF_MAPSIZE;
/// The squares in the window the state dump fingerprints: the disc's bounding box.
pub const VIEW_TILES: usize = ((MAX_VIEW_DISTANCE * 2 + 1) * (MAX_VIEW_DISTANCE * 2 + 1)) as usize;
/// How many levels below zero there are.
pub const OVERMAP_DEPTH: i32 = 10;
/// How many levels above zero there are.
pub const OVERMAP_HEIGHT: i32 = 10;
/// The whole vertical stack: below, zero, above.
pub const OVERMAP_LAYERS: i32 = 1 + OVERMAP_DEPTH + OVERMAP_HEIGHT;

/// Where a square sits inside its submap.
///
/// Reference: `maptile_soa` holds `ter[x][y]` (`submap.h:86-92`), so the flat index is `x * SEEY + y`. The other order looks just as reasonable and transposes
/// every submap, which nothing notices until a wall gets walked.
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub const fn submap_index(x: i32, y: i32) -> usize {
    // The caller owns the bounds: a square outside the submap is not this function's to reject, and the reference's own `ter[x][y]` would read memory.
    debug_assert!(x >= 0 && x < SEEX && y >= 0 && y < SEEY);
    (x * SEEY + y) as usize
}

/// Where a submap sits in the map's array of them.
///
/// Reference: `map::get_nonant` (`map.cpp:9715-9732`). With z-levels on, the index is `(z + OVERMAP_DEPTH) + (x + y * MAPSIZE) * OVERMAP_LAYERS`: the **level**
/// is the outer dimension and the two horizontal ones are a single index inside it. A position outside the map returns zero in the reference, and this returns
/// zero too rather than panicking - the reference's own choice, kept so that a bug reads the same way in both.
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub const fn nonant(x: i32, y: i32, z: i32, zlevels: bool) -> usize {
    if x < 0 || x >= MAPSIZE || y < 0 || y >= MAPSIZE || z < -OVERMAP_DEPTH || z > OVERMAP_HEIGHT {
        return 0;
    }
    if zlevels {
        let layer = z + OVERMAP_DEPTH;
        (layer + (x + y * MAPSIZE) * OVERMAP_LAYERS) as usize
    } else {
        (x + y * MAPSIZE) as usize
    }
}
