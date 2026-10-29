//! The radiation a square carries.
//!
//! Reference: `submap::get_radiation` and `set_radiation` (`src/submap.h:166-176`) and `map::adjust_radiation` (`src/map.cpp:4893-4904`), which adds a delta to the
//! square's value. The store is a plain per-square `int` with no clamp - `rad` is `cata::mdarray<int, point_sm_ms>` (`src/submap.h:69`) - so this keeps the same
//! shape: a map from square to total, and a processor that adds.
//!
//! Only three things in the data write radiation: `fd_nuke_gas` at its three higher intensities, and whatever consumes the value later. Nothing reads it yet, which is
//! why this is a store rather than a system: the reading side is the character's own dose, and that needs a character.

use std::collections::BTreeMap;

/// How much radiation every square that has any carries.
///
/// A component on the one entity [`TheRadiation`] marks: a store that belongs to the world is a
/// thing in the world, and the architecture has one place for things.
#[derive(Debug, Clone, Default, bevy_ecs::prelude::Component)]
pub struct Radiation {
    squares: BTreeMap<(i32, i32, i32), i32>,
}

impl Radiation {
    /// An empty store, which is what an untouched map has.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The radiation on a square, in the reference's units, zero when it has none.
    #[must_use]
    pub fn get(&self, square: (i32, i32, i32)) -> i32 {
        self.squares.get(&square).copied().unwrap_or(0)
    }

    /// Adds to a square's radiation.
    ///
    /// Reference: `map::adjust_radiation` (`src/map.cpp:4893-4904`), which reads the submap's value, adds the delta and writes it back. A square left at zero is
    /// dropped from the store, so an untouched map holds nothing and a square that has been cleaned up costs nothing either.
    pub fn adjust(&mut self, square: (i32, i32, i32), delta: i32) {
        let total = self.get(square) + delta;
        if total == 0 {
            self.squares.remove(&square);
        } else {
            self.squares.insert(square, total);
        }
    }

    /// How many squares hold radiation, which is what lets a gate say it compared something.
    #[must_use]
    pub fn squares_with_radiation(&self) -> usize {
        self.squares.len()
    }
}
