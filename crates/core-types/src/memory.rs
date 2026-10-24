//! Map memory: the terrain a character has seen, and how well lit it was.
//!
//! Reference: `map_memory` (`src/map_memory.cpp`) and `map::remember` (`src/game.cpp`), which copy
//! what is currently shown into a per-level store that is never cleared. A square seen once stays on
//! the character's map: it lowers to dim, and if it later goes dark it keeps the terrain it had.
//!
//! The tile is the reference's `memorized_tile` (`src/map_memory.h:22-54`) field for field: the terrain **and** a decoration, each with its own subtile and rotation, plus a
//! symbol. Four writes fill it and each preserves the rest of the tile, because the reference reads the tile out, changes one field and writes it back
//! (`src/map_memory.cpp:218-275`). The clear is the interesting one: [`MapMemory::clear_decoration`] takes a **prefix**, clears only a decoration whose id starts with it, and
//! clears the **symbol** as well when it does - the symbol was the decoration's, so a square whose graffiti is gone does not keep the graffiti's glyph.
//!
//! **One deliberate seam.** The port stores the terrain twice: as the code the drawing path indexes with, and as the id the reference stores. They are the same fact, and the
//! id is what a recording can be compared with, so the recording is checked against the id and the drawing path keeps its code.

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
///
/// Reference: `memorized_tile` (`src/map_memory.h:22-54`). The four small numbers are `int8_t` there, which is why they are `i8` here: a subtile is one of four and a
/// rotation one of four, and the reference's own type says how small the range is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memorized {
    /// The terrain, as the code the drawing path indexes with.
    pub terrain: u32,
    /// The terrain, as the reference's `ter_str_id` - what a recording is compared with.
    pub terrain_id: String,
    /// Which of the terrain's four subtiles was drawn.
    pub terrain_subtile: i8,
    /// Which of its four rotations was drawn.
    pub terrain_rotation: i8,
    /// The decoration: a piece of furniture, a vehicle part, graffiti. Empty when there is none.
    pub decoration: String,
    /// Which of the decoration's subtiles was drawn.
    pub decoration_subtile: i8,
    /// Which of its rotations was drawn.
    pub decoration_rotation: i8,
    /// The glyph the square was drawn with, as a codepoint. Zero when nothing was drawn.
    pub symbol: u32,
    /// How well lit it was.
    pub light: RememberedLight,
}

impl Default for Memorized {
    fn default() -> Self {
        // The reference's `memorized_tile` is a struct of zeroes and empty strings, and a square that was never written answers exactly that
        // (`mm_submap::default_tile`, `src/map_memory.cpp:37`).
        Self {
            terrain: 0,
            terrain_id: String::new(),
            terrain_subtile: 0,
            terrain_rotation: 0,
            decoration: String::new(),
            decoration_subtile: 0,
            decoration_rotation: 0,
            symbol: 0,
            light: RememberedLight::Dim,
        }
    }
}

/// Everything the character remembers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapMemory {
    squares: BTreeMap<(i32, i32), Memorized>,
}

impl MapMemory {
    /// An empty memory.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What is remembered about a square, if anything.
    #[must_use]
    pub fn at(&self, square: (i32, i32)) -> Option<&Memorized> {
        self.squares.get(&square)
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
    pub fn remember(&mut self, square: (i32, i32), remembered: Memorized) {
        self.squares.insert(square, remembered);
    }

    /// The tile on a square, ready to be written back: the default tile when nothing is remembered.
    fn tile(&self, square: (i32, i32)) -> Memorized {
        self.squares.get(&square).cloned().unwrap_or_default()
    }

    /// Records the terrain and how it was drawn, keeping the decoration and the symbol.
    ///
    /// Reference: `map_memory::set_tile_terrain` (`src/map_memory.cpp:218-232`).
    pub fn set_terrain(
        &mut self,
        square: (i32, i32),
        terrain: u32,
        id: &str,
        subtile: i8,
        rotation: i8,
    ) {
        let mut tile = self.tile(square);
        tile.terrain = terrain;
        id.clone_into(&mut tile.terrain_id);
        tile.terrain_subtile = subtile;
        tile.terrain_rotation = rotation;
        self.squares.insert(square, tile);
    }

    /// Records the decoration and how it was drawn.
    ///
    /// Reference: `map_memory::set_tile_decoration` (`src/map_memory.cpp:233-247`).
    pub fn set_decoration(&mut self, square: (i32, i32), id: &str, subtile: i8, rotation: i8) {
        let mut tile = self.tile(square);
        id.clone_into(&mut tile.decoration);
        tile.decoration_subtile = subtile;
        tile.decoration_rotation = rotation;
        self.squares.insert(square, tile);
    }

    /// Records the glyph the square was drawn with.
    ///
    /// Reference: `map_memory::set_tile_symbol` (`src/map_memory.cpp:248-259`).
    pub fn set_symbol(&mut self, square: (i32, i32), symbol: u32) {
        let mut tile = self.tile(square);
        tile.symbol = symbol;
        self.squares.insert(square, tile);
    }

    /// Forgets a decoration whose id starts with `prefix`, and the symbol that was drawn for it.
    ///
    /// Reference: `map_memory::clear_tile_decoration` (`src/map_memory.cpp:260-276`). The test is `string_starts_with( mt.get_dec_id(), prefix )`, so the **empty prefix
    /// matches every decoration** - which is how the reference forgets every decoration in a region at once. A clear that does not match changes nothing at all, and a clear
    /// that does match also zeroes the symbol, because the glyph belonged to the decoration.
    pub fn clear_decoration(&mut self, square: (i32, i32), prefix: &str) {
        let mut tile = self.tile(square);
        if !tile.decoration.starts_with(prefix) {
            return;
        }
        tile.decoration.clear();
        tile.decoration_subtile = 0;
        tile.decoration_rotation = 0;
        tile.symbol = 0;
        self.squares.insert(square, tile);
    }
}

/// Records what the character can see this turn into the memory.
///
/// Returns how many squares were added or updated. A square that is in sight is written whether it
/// is lit or not - the character sees *something* there and the terrain is what they add to the map -
/// but only squares that are actually shown are written, so a square behind a wall never enters the
/// memory by this route.
///
/// **What this does not write, and the reference does:** the subtile and rotation, which need the tile's
/// drawing data, and the decoration, which comes from the furniture and vehicle layers. Both default to
/// what the reference's `default_tile` answers.
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
                Memorized {
                    terrain,
                    light: light_level,
                    ..Memorized::default()
                },
            );
            written += 1;
        }
    }
    written
}
