//! The frame: what the port would draw for one view.
//!
//! Reference: the draw path's per-cell work (`map::draw`, and the curses and tiles front ends behind it), which asks three things about a square - what is there, how brightly
//! it is lit, and how clearly the character sees it - and prints the glyph only when the level is not blank.
//!
//! The three answers already existed. What did not was a place that asks them **once per view** rather than once per question: a frame is one sight cast, one light map, and a
//! walk over the squares, which is the shape the reference's frame has and the shape the semantic frame gate will compare.

use bevy_ecs::prelude::*;
use cdda_core_types::level::SeenGrid;
use cdda_core_types::light::LitLevel;
use cdda_core_types::vision::{View, lit_level_with_codes};
use std::collections::BTreeMap;

use crate::memory::Memory;
use crate::{Light, field_opacity};

/// One square of a drawn frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameCell {
    /// The terrain's glyph, as the data gives it.
    pub glyph: char,
    /// How the square is drawn: bright, lit, low, a glow with nothing in it, or blank.
    pub lit: LitLevel,
    /// How clearly the character sees it, zero to one.
    pub seen: f32,
    /// Whether the glyph came out of the character's memory rather than off the map.
    ///
    /// Reference: `draw_background` (`src/map.cpp:6906-6917`), which the draw loop calls for a square whose `visibility_type` is `HIDDEN` or `DARK`. Such a square prints
    /// the symbol it was **memorized** with, and a blank space when it was never seen.
    pub from_memory: bool,
}

/// A drawn frame: the squares of the view, row by row.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    origin: (i32, i32),
    radius: i32,
    cells: Vec<FrameCell>,
}

impl Frame {
    /// The cell at a world square, when the frame covers it.
    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> Option<&FrameCell> {
        let side = usize::try_from(self.radius * 2 + 1).ok()?;
        let column = usize::try_from(x - self.origin.0 + self.radius).ok()?;
        let row = usize::try_from(y - self.origin.1 + self.radius).ok()?;
        if column >= side || row >= side {
            return None;
        }
        self.cells.get(row * side + column)
    }

    /// The frame as rows of glyphs, one line per row of squares.
    #[must_use]
    pub fn text(&self) -> String {
        self.render(|cell| cell.glyph)
    }

    /// The frame as rows of levels: `B` bright, `L` lit, `l` low, `*` a glow with nothing in it, `.` blank.
    ///
    /// This is the string a semantic frame gate compares: a picture of the world is hard to diff and a picture of *what the character can tell about it* is not.
    #[must_use]
    pub fn levels(&self) -> String {
        self.render(|cell| match cell.lit {
            LitLevel::Bright => 'B',
            LitLevel::Lit => 'L',
            LitLevel::Low => 'l',
            LitLevel::BrightOnly => '*',
            LitLevel::Dark | LitLevel::Memorized | LitLevel::Blank => '.',
        })
    }

    fn render(&self, of: impl Fn(&FrameCell) -> char) -> String {
        let Ok(side) = usize::try_from(self.radius * 2 + 1) else {
            return String::new();
        };
        let mut out = String::new();
        for row in 0..side {
            for column in 0..side {
                out.push(self.cells.get(row * side + column).map_or(' ', &of));
            }
            out.push('\n');
        }
        out
    }
}


/// One square of a frame, drawn the way the reference's loop draws it.
///
/// Reference: `map::draw`'s inner loop (`src/map.cpp:6930-6960`): the square's `lit_level`, the `visibility_type` it becomes, and the two branches - a background for
/// `HIDDEN` and `DARK` (the memorized symbol, or a space) and the tile itself otherwise, which is also when the tile is memorized (`src/map.cpp:7216`).
#[allow(clippy::too_many_arguments)]
fn draw_cells(
    code_at: &dyn Fn(i32, i32) -> Option<u32>,
    view: &View,
    light: &Light,
    grid: &SeenGrid,
    memory: Option<&mut Memory>,
    terrain: &crate::terrain_table::TerrainTable,
    origin: (i32, i32),
    radius: i32,
    side: usize,
) -> Vec<FrameCell> {
    let light_map = light.squares_for_the_rules();
    let mut memory = memory;
    let mut cells = Vec::with_capacity(side * side);
    for row in 0..side {
        for column in 0..side {
            let square = (
                origin.0 + i32::try_from(column).unwrap_or(0) - radius,
                origin.1 + i32::try_from(row).unwrap_or(0) - radius,
            );
            let code = code_at(square.0, square.1).unwrap_or(0);
            let lit = lit_level_with_codes(code_at, view, &light_map, grid, square);
            // **Two branches, the reference's own.** `get_visibility` turns `DARK`, `BLANK` and `MEMORIZED` into something the loop draws as a background
            // (`src/map.cpp:6836-6865`), and everything else as the tile itself.
            let hidden = matches!(lit, LitLevel::Dark | LitLevel::Blank | LitLevel::Memorized);
            let remembered = if hidden {
                memory
                    .as_ref()
                    .and_then(|memory| memory.0.at(square))
                    .and_then(|tile| char::from_u32(tile.symbol))
                    .filter(|symbol| *symbol != '\0')
            } else {
                None
            };
            let glyph = if let Some(remembered) = remembered {
                remembered
            } else if hidden {
                ' '
            } else {
                let symbol = terrain.symbol(code);
                if let Some(memory) = memory.as_mut() {
                    // `map::draw` memorizes inside `draw_maptile` for every square it prints (`src/map.cpp:7216`), so a square is remembered because it was drawn. The
                    // terrain id is not available here - the frame reads codes - so the tile carries the code and the symbol, which is what a remembered square prints.
                    memory.0.set_terrain(square, code, "", 0, 0);
                    memory.0.set_symbol(square, u32::from(symbol));
                }
                symbol
            };
            cells.push(FrameCell {
                glyph,
                lit,
                seen: grid.at(square),
                from_memory: remembered.is_some(),
            });
        }
    }
    cells
}

/// Draws the frame around a character.
///
/// One sight cast, one light map, one walk: the reference rebuilds a level's caches once per frame and then asks about each square
/// (`map::build_map_cache`, then the draw loop). The `ground_light_level` and `vision_threshold` are the character's, which the port has no creature layer for yet, so the
/// caller supplies them from [`cdda_core_types::light::vision_threshold`].
///
/// # Panics
///
/// Panics if the radius is negative.
#[must_use]
pub fn frame(
    world: &mut World,
    origin: (i32, i32),
    radius: i32,
    ground_light_level: f32,
    vision_threshold: f32,
) -> Frame {
    assert!(radius >= 0, "a negative view distance is a caller bug");
    let terrain = crate::singletons::prototypes(world)
        .map(|prototypes| prototypes.terrain.clone())
        .unwrap_or_default();
    // Furniture is a second layer of the transparency rule: either layer being opaque stops the ray (`lightmap.cpp:146-149`). Nothing here draws furniture glyphs yet, so the
    // layer only ever makes a square darker than its terrain.
    let furniture = crate::singletons::prototypes(world)
        .map(|prototypes| prototypes.furniture.clone())
        .unwrap_or_default();
    // The frame draws one level, and furniture is not drawn yet, so the two layers' numbers are merged here by taking the more absorbing of the two for each code - the
    // product the reference forms per square, applied per code because a code is all this frame needs to know.
    let mut absorbing_layers = terrain.clone();
    absorbing_layers.merge_absorbing_from(&furniture);

    let transparency: BTreeMap<(i32, i32), f32> = {
        if crate::square::squares(world).is_empty() {
            return Frame {
                origin,
                radius,
                cells: Vec::new(),
            };
        }
        let side = usize::try_from(radius * 2 + 1).unwrap_or(0);
        // **The fields on every square**, which the sight cast reads and the frame's table was leaving out: smoke between the character and a square attenuates the view of
        // that square exactly as the reference's `transparency_cache` does (`calc_transp`, `lightmap.cpp:140-167`). Read once, before the level is borrowed.
        let fields = field_opacity(world);
        let mut table = BTreeMap::new();
        for row in 0..side {
            for column in 0..side {
                let square = (
                    origin.0 + i32::try_from(column).unwrap_or(0) - radius,
                    origin.1 + i32::try_from(row).unwrap_or(0) - radius,
                );
                let code = crate::square::terrain_at(world, (square.0, square.1, 0)).unwrap_or(0);
                let furniture_code =
                    crate::square::furniture_at(world, (square.0, square.1, 0)).unwrap_or(0);
                let mut transparency = terrain
                    .transparency(code)
                    .min(furniture.transparency(furniture_code));
                // **Fields multiply**, as `calc_transp` does (`lightmap.cpp:155-165`): smoke at its second intensity is ten times as absorbing as air, and at its third it
                // is nothing at all.
                transparency *= fields.multiplier_at(square.0, square.1);
                table.insert(square, transparency);
            }
        }
        table
    };

    // The two rules read the table, and the glyphs below read it too, so each closure owns a copy rather than fighting over one.
    let opaque = terrain.clone();
    let absorbing = absorbing_layers;
    let view = View {
        origin,
        radius,
        blocks_light: Box::new(move |code| opaque.transparency(code) <= 0.0),
        // The two layers multiply (`calc_transp`, `lightmap.cpp:146-149`), and the table already reports zero for an opaque code, so the product is the terrain's own number
        // unless the furniture makes it worse.
        transparency_of: Box::new(move |code| absorbing.transparency(code)),
        ground_light_level,
        vision_threshold,
    };
    // The cast is a function of the table, so nothing has to be taken out of the world for it. The
    // count is kept because "a frame is one cast" is a promise, and a promise needs a receipt.
    let grid: SeenGrid = cdda_core_types::level::seen_grid(origin, radius, &transparency);
    crate::singletons::count_sight_cast(world);
    let light = crate::singletons::light(world);

    let side = usize::try_from(radius * 2 + 1).unwrap_or(0);
    // **The memory is taken off its entity while the frame is drawn**, for the same reason the level is: the loop both reads it and writes to it, and a component borrow
    // held across the walk cannot be handed back to the caller. A world with no memory entity still draws; it just never remembers anything.
    let memory_entity = world
        .query_filtered::<Entity, With<crate::TheMemory>>()
        .iter(world)
        .next();
    let mut memory = memory_entity.and_then(|entity| world.entity_mut(entity).take::<Memory>());
    let code_at = |x: i32, y: i32| crate::square::terrain_at(world, (x, y, 0));
    let cells = draw_cells(
        &code_at,
        &view,
        &light,
        &grid,
        memory.as_mut(),
        &terrain,
        origin,
        radius,
        side,
    );
    if let (Some(entity), Some(memory)) = (memory_entity, memory) {
        world.entity_mut(entity).insert(memory);
    }
    Frame {
        origin,
        radius,
        cells,
    }
}
