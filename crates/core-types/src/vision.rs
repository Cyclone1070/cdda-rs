//! What the character can actually see: the cast, limited by light.
//!
//! Reference: the draw path reads `map::seen` and `lm.apparent_light_at` together
//! (`src/game.cpp`, the map drawing loop), so a square in sight but in pitch darkness is not shown
//! at all. The two halves live in different places in the reference and are joined when the frame is
//! drawn; this module joins them the same way, as a query rather than a cache.
//!
//! The port keeps the visible-*and*-lit rule only. The reference's finer behaviour - remembering a
//! square's terrain after it goes dark, drawing it dimmed below `LIGHT_AMBIENT_LOW` - needs map
//! memory, which is not ported.

use crate::level::Level;
use crate::shadowcasting::FourQuadrants;
use std::collections::BTreeMap;

/// The light the map holds for a square: the brightest of the four quadrants that cover it.
///
/// This is **not** the reference's `apparent_light`, which is this value multiplied by the square's visibility (`lightmap.cpp:795-798`). The multiplication happens in
/// [`shown_at`], where the cast is known; a caller that forgets it shows the player what is behind a wall.
///
/// Reference: `level_cache::apparent_light_at` (`src/lightmap.cpp`), which adds the ambient light to
/// the brightest quadrant and clamps at the low and high ambient values.
#[must_use]
pub fn apparent_light_at(light: &BTreeMap<(i32, i32), FourQuadrants>, square: (i32, i32)) -> f32 {
    light.get(&square).map_or(0.0, FourQuadrants::max)
}

/// The light a square appears to hold, counting only the quarters the character can see it from.
///
/// Reference: `level_cache::apparent_light_helper` (`src/lightmap.cpp:725-800`), the branch that matters. For an **opaque** square the reference does not take the brightest of
/// the four quarters, because three of them may face into the wall: it walks the eight neighbours, keeps the ones that are non-opaque and visible, and counts the light only in
/// the two quarters each of them faces. A non-opaque square takes the brightest quarter, "for a non-opaque tile light from all directions is equivalent"
/// (`lightmap.cpp:792-796`).
///
/// The port's visibility is a yes or a no rather than the reference's fraction, so a visible square counts as fully visible; that is the same trade [`shown_at`] makes.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn apparent_light_with(
    level: &Level,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    grid: &crate::level::SeenGrid,
    square: (i32, i32),
) -> f32 {
    apparent_light_with_codes(&|x, y| level.terrain_at(x, y), view, light, grid, square)
}

/// [`apparent_light_with`] for a caller whose squares are components rather than a level.
///
/// The level was only ever asked for two things here - is this square ground at all, and is that
/// ground opaque - so the question becomes a function the caller supplies.
#[must_use]
pub fn apparent_light_with_codes(
    code_at: &dyn Fn(i32, i32) -> Option<u32>,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    grid: &crate::level::SeenGrid,
    square: (i32, i32),
) -> f32 {
    let opaque = code_at(square.0, square.1).is_some_and(|code| (view.blocks_light)(code));
    apparent_light_from(
        code_at,
        view,
        light,
        grid.at(square),
        square,
        opaque,
        |neighbour| grid.at(neighbour) > 0.0,
    )
}

/// [`apparent_light_with`] for a caller that has no cast in hand.
///
/// # Panics
///
/// Panics if the radius is negative, as [`Level::seen_at`] does.
#[must_use]
pub fn apparent_light_at_seen(
    level: &mut Level,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    square: (i32, i32),
) -> f32 {
    let grid = level.seen_grid(view.origin, view.radius, |code| {
        (view.transparency_of)(code)
    });
    let opaque = level
        .terrain_at(square.0, square.1)
        .is_some_and(|code| (view.blocks_light)(code));
    let code_at = |x: i32, y: i32| level.terrain_at(x, y);
    apparent_light_from(
        &code_at,
        view,
        light,
        grid.at(square),
        square,
        opaque,
        |neighbour| grid.at(neighbour) > 0.0,
    )
}

/// The body both entry points share: the light a square appears to hold, given how clearly it is seen.
#[allow(clippy::too_many_arguments)]
fn apparent_light_from(
    code_at: &dyn Fn(i32, i32) -> Option<u32>,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    seen: f32,
    square: (i32, i32),
    opaque: bool,
    seen_at_neighbour: impl Fn((i32, i32)) -> bool,
) -> f32 {
    use crate::shadowcasting::{FourQuadrants, Quadrant};
    let Some(cell) = light.get(&square).copied() else {
        return 0.0;
    };
    // **Visibility multiplies the light** (`lightmap.cpp:795-798`), and the multiplier is the sight cast's own fraction: a square seen through smoke gets a fraction of the light
    // the map says it holds, and a square out of the cast gets none of it.
    if seen <= 0.0 {
        return 0.0;
    }
    if !opaque {
        // The simple branch: for a non-opaque tile light from all directions is equivalent (`lightmap.cpp:792-796`).
        return seen * cell.max();
    }

    // The reference's own table (`lightmap.cpp:756-767`): which two quarters each neighbour faces. Eight entries, in this order.
    let neighbours: [((i32, i32), [Quadrant; 2]); 8] = [
        ((0, 1), [Quadrant::SouthEast, Quadrant::SouthWest]),
        ((0, -1), [Quadrant::NorthEast, Quadrant::NorthWest]),
        ((1, 0), [Quadrant::SouthEast, Quadrant::NorthEast]),
        ((1, 1), [Quadrant::SouthEast, Quadrant::SouthEast]),
        ((1, -1), [Quadrant::NorthEast, Quadrant::NorthEast]),
        ((-1, 0), [Quadrant::SouthWest, Quadrant::NorthWest]),
        ((-1, 1), [Quadrant::SouthWest, Quadrant::SouthWest]),
        ((-1, -1), [Quadrant::NorthWest, Quadrant::NorthWest]),
    ];

    let mut seen_from = FourQuadrants::uniform(0.0);
    for (offset, quarters) in neighbours {
        let neighbour = (square.0 + offset.0, square.1 + offset.1);
        // Out of bounds, or a wall: the reference skips both (`lightmap.cpp:770-776`).
        let Some(code) = code_at(neighbour.0, neighbour.1) else {
            continue;
        };
        if (view.blocks_light)(code) {
            continue;
        }
        // The neighbour has to be seen too, and the cast in hand is the same one the reference consults (`map_cache.seen_cache[neighbour]`): out of the frame it reads zero, which
        // skips the neighbour exactly as an unseen one is skipped.
        if !seen_at_neighbour(neighbour) {
            continue;
        }
        for quarter in quarters {
            seen_from.set(quarter, 1.0);
        }
    }

    let mut apparent = 0.0f32;
    for quarter in [
        Quadrant::NorthEast,
        Quadrant::SouthEast,
        Quadrant::SouthWest,
        Quadrant::NorthWest,
    ] {
        apparent = apparent.max(seen_from.get(quarter) * cell.get(quarter));
    }
    apparent * seen
}

/// What the character is looking with and from: the origin, the range, what stops the sight, and the two numbers that decide whether the light is enough.
///
/// Reference: the facts `map::pl_sees` reads from the avatar (`src/map.cpp:854-871`) that the port takes as arguments instead - there is no creature layer yet.
pub struct View {
    /// Where the character is.
    pub origin: (i32, i32),
    /// How far they see.
    pub radius: i32,
    /// Which terrain codes stop the sight.
    ///
    /// Owned rather than a `fn` pointer: the sim's rule reads a loaded table, and a table has no `fn` pointer to give.
    pub blocks_light: Box<dyn Fn(u32) -> bool>,
    /// How much a code's square absorbs: the same number `calc_transp` puts in the reference's transparency cache, where `TRANSPARENCY_OPEN_AIR` is the floor and zero means
    /// opaque. Sight attenuates by it (`cast_seen_light`), so the answer is a fraction rather than a yes or a no.
    pub transparency_of: Box<dyn Fn(u32) -> f32>,
    /// The light around the character, which decides whether a hazy square glows.
    pub ground_light_level: f32,
    /// The dimmest light they can see by ([`crate::light::vision_threshold`]).
    pub vision_threshold: f32,
}

/// Whether a square is shown to the character: in the cast, and bright enough to see by.
///
/// Reference: `map::pl_sees` (`src/map.cpp:854-871`) joined with the draw path's `lit_level` test - the two halves of the rule the frame actually uses. `pl_sees` asks for
/// the square to be in the cast and `apparent_light >= vision_threshold`; the draw path then skips a square whose level is `Blank`, which is the same test plus the hazy
/// case. The port asks it once, here, and answers with the level rather than with a bool in two places.
///
/// `ground_light_level` is the light around the character, which decides whether a hazy square glows; `vision_threshold` comes from [`crate::light::vision_threshold`].
///
/// # Panics
///
/// Panics if the radius is negative, as [`Level::seen_at`] does.
pub fn shown_at(
    level: &mut Level,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    square: (i32, i32),
) -> bool {
    lit_level_at_square(level, view, light, square) != crate::light::LitLevel::Blank
}

/// The level a square is drawn at, which is the whole answer a frame needs about its light.
///
/// Reference: `map::apparent_light_at` (`src/lightmap.cpp:801-851`) as the draw path calls it - the same chain [`lit_level_at`] holds, with the facts taken from the world
/// rather than handed over. Splitting it out means the two callers a frame has - "is this drawn at all" and "at which level" - ask one question, so they cannot disagree.
#[must_use]
pub fn lit_level_with(
    level: &Level,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    grid: &crate::level::SeenGrid,
    square: (i32, i32),
) -> crate::light::LitLevel {
    lit_level_with_codes(&|x, y| level.terrain_at(x, y), view, light, grid, square)
}

/// [`lit_level_with`] for a caller whose squares are components rather than a level.
#[must_use]
pub fn lit_level_with_codes(
    code_at: &dyn Fn(i32, i32) -> Option<u32>,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    grid: &crate::level::SeenGrid,
    square: (i32, i32),
) -> crate::light::LitLevel {
    // `obstructed` is the sight cast's own fraction at or below a tenth (`lightmap.cpp:733`), not "was anything in the way": a wall you look at is seen clearly, and a square seen
    // only through smoke or fog is not.
    let seen = grid.at(square);
    lit_level_at(
        apparent_light_with_codes(code_at, view, light, grid, square),
        seen <= 0.1,
        view.ground_light_level,
        view.vision_threshold,
        false,
    )
}

/// [`lit_level_with`] for a caller that has no cast in hand.
///
/// # Panics
///
/// Panics if the radius is negative, as [`Level::seen_at`] does.
#[must_use]
pub fn lit_level_at_square(
    level: &mut Level,
    view: &View,
    light: &BTreeMap<(i32, i32), FourQuadrants>,
    square: (i32, i32),
) -> crate::light::LitLevel {
    // **Visibility multiplies the light.** `apparent_light = vis * lm.max()` (`lightmap.cpp:795-798`), and `vis` is the seen-cache value, so a square out of the cast has an
    // apparent light of zero however bright the light map says it is. [`apparent_light_at_seen`] does that multiplication and, on an opaque square, keeps only the quarters the
    // character can see it from. Believing the light map is already apparent light shows the player what is behind a wall, which is what the first test here caught.
    let grid = level.seen_grid(view.origin, view.radius, |code| {
        (view.transparency_of)(code)
    });
    lit_level_with(level, view, light, &grid, square)
}

/// What a square looks like to the character: the reference's `lit_level`.
///
/// Reference: `map::apparent_light_at` (`src/lightmap.cpp:801-851`), with the parts that need an avatar - clairvoyance, the camera cache, the recoil check, the impaired
/// range - left to the caller, which supplies the three facts they decide: whether the light got through only as haze (`obstructed`), whether anything is on the square
/// (`sees_items`), and the character's vision threshold.
///
/// `obstructed` is the reference's `a.obstructed`, which is true when the square's own `seen_cache` value is at or below solid: the light arrives as a glow rather than as
/// detail. `apparent_light` is the brightest quadrant of the light map.
#[must_use]
pub fn lit_level_at(
    apparent_light: f32,
    obstructed: bool,
    ground_light_level: f32,
    vision_threshold: f32,
    sees_items: bool,
) -> crate::light::LitLevel {
    use crate::light::{AMBIENT_LIT, LitLevel, SOURCE_BRIGHT};
    if obstructed {
        // Hazy: enough light to glow, not enough to make anything out.
        if apparent_light > AMBIENT_LIT && apparent_light > ground_light_level {
            return LitLevel::BrightOnly;
        }
        return LitLevel::Blank;
    }
    if apparent_light > SOURCE_BRIGHT || sees_items {
        return LitLevel::Bright;
    }
    if apparent_light > AMBIENT_LIT {
        return LitLevel::Lit;
    }
    if apparent_light >= vision_threshold {
        return LitLevel::Low;
    }
    LitLevel::Blank
}

/// Whether a square can be seen from another on the same level.
///
/// Reference: `map::sees`'s same-level branch (`src/map.cpp:7329-7362`). Sight is a Bresenham walk
/// from the viewer to the target that stops **before** testing the square it is looking at, so a
/// wall is seen as a wall and only what is behind it stays hidden.
///
/// The three gates come in the reference's order: the range, the bounds of the level, and then the
/// walk. `range` is negative for "no limit", which is the reference's own convention. A line that
/// leaves the level is blocked from the square where it leaves.
#[must_use]
pub fn sees_on_one_level(
    from: (i32, i32),
    to: (i32, i32),
    range: i32,
    transparent: impl Fn(i32, i32) -> bool,
    in_bounds: impl Fn(i32, i32) -> bool,
) -> bool {
    if range >= 0 && range < rl_dist(from, to) {
        return false;
    }
    if !in_bounds(to.0, to.1) {
        return false;
    }
    for step in crate::line::line_to(
        crate::line::Point::new(from.0, from.1),
        crate::line::Point::new(to.0, to.1),
        0,
    ) {
        // The square being looked at is seen even when it is what stops the line: a wall is visible as a wall.
        if step.x == to.0 && step.y == to.1 {
            return true;
        }
        if !in_bounds(step.x, step.y) || !transparent(step.x, step.y) {
            return false;
        }
    }
    // An empty walk means the viewer is looking at their own square, which they can see.
    true
}

/// The distance the reference's range check uses.
///
/// Reference: `rl_dist` (`src/line.cpp`), which is Chebyshev distance unless the "circular distances"
/// option is on. The option is off by default and the port has no options yet, so this is the
/// square-ring distance and the option is a declared gap.
fn rl_dist(from: (i32, i32), to: (i32, i32)) -> i32 {
    (from.0 - to.0).abs().max((from.1 - to.1).abs())
}
