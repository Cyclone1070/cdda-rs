//! Map memory: the terrain a character has seen, and how well lit it was.
//!
//! Reference: `map_memory` (`src/map_memory.cpp`) and `map::remember` (`src/game.cpp`), which copy
//! what is currently shown into a per-level store that is never cleared. A square seen once stays on
//! the character's map: it lowers to dim, and if it later goes dark it keeps the terrain it had.
//!
//! The port keeps the terrain id and a brightness category per square. The reference also remembers
//! furniture, items on the floor and the submap's own decoration; those arrive with the systems that
//! produce them, and the ledger names the gap.

use crate::light::AMBIENT_LOW;
use crate::shadowcasting::FourQuadrants;
use crate::vision::apparent_light_at;
use std::collections::BTreeMap;

/// How well a remembered square is lit, which is what the draw path uses to pick a shade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RememberedLight {
    /// Seen, but with less light than the low ambient value: drawn dark.
    Dim,
    /// Lit to at least the low ambient value: drawn normally.
    Lit,
}

/// One square the character remembers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remembered {
    /// The terrain it had when it was last seen.
    pub terrain: u32,
    /// How well lit it was.
    pub light: RememberedLight,
}

/// Everything the character remembers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapMemory {
    squares: BTreeMap<(i32, i32), Remembered>,
}

impl MapMemory {
    /// An empty memory.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What is remembered about a square, if anything.
    #[must_use]
    pub fn at(&self, square: (i32, i32)) -> Option<Remembered> {
        self.squares.get(&square).copied()
    }

    /// How many squares are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.squares.len()
    }

    /// Whether nothing has been seen yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.squares.is_empty()
    }

    /// Adds one square to the memory, replacing what was there.
    pub fn remember(&mut self, square: (i32, i32), remembered: Remembered) {
        self.squares.insert(square, remembered);
    }
}

/// Records what the character can see this turn into the memory.
///
/// Returns how many squares were added or updated. A square that is in sight is written whether it
/// is lit or not - the character sees *something* there and the terrain is what they add to the map -
/// but only squares that are actually shown are written, so a square behind a wall never enters the
/// memory by this route.
pub fn remember_seen(
    memory: &mut MapMemory,
    level: &mut crate::level::Level,
    origin: (i32, i32),
    radius: i32,
    blocks_light: impl Fn(u32) -> bool,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    terrain_of: impl Fn(i32, i32) -> Option<u32>,
) -> usize {
    let mut written = 0;
    let rows = level.visibility(origin, radius, blocks_light);
    let side = usize::try_from(radius * 2 + 1).unwrap_or(0);
    for row in 0..side {
        for column in 0..side {
            let Some(seen) = rows.get(row).and_then(|row| row.get(column)).copied() else {
                continue;
            };
            if !seen {
                continue;
            }
            let Ok(dx) = i32::try_from(column) else {
                continue;
            };
            let Ok(dy) = i32::try_from(row) else {
                continue;
            };
            let square = (origin.0 + dx - radius, origin.1 + dy - radius);
            let Some(terrain) = terrain_of(square.0, square.1) else {
                continue;
            };
            let light_level = if apparent_light_at(light, square) >= AMBIENT_LOW {
                RememberedLight::Lit
            } else {
                RememberedLight::Dim
            };
            memory.remember(
                square,
                Remembered {
                    terrain,
                    light: light_level,
                },
            );
            written += 1;
        }
    }
    written
}
