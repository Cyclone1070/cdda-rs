//! Whether a square is under the sky.
//!
//! Reference: `map::is_outside( const tripoint & )` (`map.cpp`), which reads the outside cache. That cache is built from the terrain and furniture `INDOORS` flags and
//! then spread out from walls and roofs. The port builds the same cache when it casts light, but a dialogue effect asking "may I put a monster here" needs the
//! answer for one square and not a whole cast, so this asks the flags directly - the same flags the cache starts from.

use bevy_ecs::prelude::*;

/// Whether a square is open to the sky.
///
/// A square the level does not hold is **not** outside: there is nowhere to stand, so no monster may be put there at all. A square inside answers `false` when the
/// terrain or the furniture on it carries the indoors flag - which is the reference's own starting point before it spreads the answer out from walls.
#[must_use]
pub fn is_outside(world: &World, square: (i32, i32, i32)) -> bool {
    let Some(code) = crate::square::terrain_at(world, square) else {
        return false;
    };
    let Some(prototypes) = crate::singletons::prototypes(world) else {
        return true;
    };
    if prototypes.terrain.is_indoors(code) {
        return false;
    }
    !crate::square::furniture_at(world, square)
        .is_some_and(|furniture| prototypes.furniture.is_indoors(furniture))
}
