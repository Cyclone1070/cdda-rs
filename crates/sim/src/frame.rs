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
use cdda_core_types::vision::{View, lit_level_with};
use std::collections::BTreeMap;

use crate::{Light, Map, Prototypes, field_opacity};

/// One square of a drawn frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameCell {
    /// The terrain's glyph, as the data gives it.
    pub glyph: char,
    /// How the square is drawn: bright, lit, low, a glow with nothing in it, or blank.
    pub lit: LitLevel,
    /// How clearly the character sees it, zero to one.
    pub seen: f32,
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
    let terrain = world
        .get_resource::<Prototypes>()
        .map(|prototypes| prototypes.terrain.clone())
        .unwrap_or_default();
    // Furniture is a second layer of the transparency rule: either layer being opaque stops the ray (`lightmap.cpp:146-149`). Nothing here draws furniture glyphs yet, so the
    // layer only ever makes a square darker than its terrain.
    let furniture = world
        .get_resource::<Prototypes>()
        .map(|prototypes| prototypes.furniture.clone())
        .unwrap_or_default();
    // The frame draws one level, and furniture is not drawn yet, so the two layers' numbers are merged here by taking the more absorbing of the two for each code - the
    // product the reference forms per square, applied per code because a code is all this frame needs to know.
    let mut absorbing_layers = terrain.clone();
    absorbing_layers.merge_absorbing_from(&furniture);

    let transparency: BTreeMap<(i32, i32), f32> = {
        let Some(map) = world.get_resource::<Map>() else {
            return Frame {
                origin,
                radius,
                cells: Vec::new(),
            };
        };
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
                let code = map.0.terrain_at(square.0, square.1).unwrap_or(0);
                let furniture_code = map.0.furniture_at(square.0, square.1).unwrap_or(0);
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
    // The level is taken out of the world for the cast and put back: the cast needs it mutably, and nothing else may borrow it meanwhile.
    let grid: SeenGrid = match world.remove_resource::<Map>() {
        Some(mut map) => {
            let grid = map.0.seen_grid_of(origin, radius, &transparency);
            world.insert_resource(map);
            grid
        }
        None => return Frame {
            origin,
            radius,
            cells: Vec::new(),
        },
    };
    let light = world.get_resource::<Light>().cloned().unwrap_or_default();

    let side = usize::try_from(radius * 2 + 1).unwrap_or(0);
    let mut cells = Vec::with_capacity(side * side);
    let Some(map) = world.get_resource::<Map>() else {
        return Frame {
            origin,
            radius,
            cells,
        };
    };
    for row in 0..side {
        for column in 0..side {
            let square = (
                origin.0 + i32::try_from(column).unwrap_or(0) - radius,
                origin.1 + i32::try_from(row).unwrap_or(0) - radius,
            );
            let code = map.0.terrain_at(square.0, square.1).unwrap_or(0);
            // The rule functions take the whole light map; a frame's is the squares the cast reached, which is what `Light` holds.
            cells.push(FrameCell {
                glyph: terrain.symbol(code),
                lit: lit_level_with(&map.0, &view, &light.squares_for_the_rules(), &grid, square),
                seen: grid.at(square),
            });
        }
    }
    Frame {
        origin,
        radius,
        cells,
    }
}
