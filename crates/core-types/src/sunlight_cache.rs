//! Daylight falling from the sky down through the levels, one level at a time.
//!
//! Reference: `map::build_sunlight_cache( pzlev )` (`src/lightmap.cpp:306-437`). Sunlight is parallel, so there are no rays here: the reference walks the z-levels from
//! the top down, and each square takes its light from the square directly above it or, failing that, from one of the four cardinals - which is how daylight spills
//! sideways under a ceiling.
//!
//! Two flags carry the state between levels and keep the cheap cases cheap: while nothing has blocked the sun a whole level is filled in one `fill_n`, and once nothing
//! gets through a level is filled with the indoors value and skipped. The per-square work happens only in between.

use crate::light::{AMBIENT_DIM, AMBIENT_LOW, SOURCE_BRIGHT, TRANSPARENCY_OPEN_AIR};
use crate::shadowcasting::{FourQuadrants, Quadrant};

/// What one level looks like to the sunlight pass.
///
/// Reference: the three caches `build_sunlight_cache` reads for a level - `transparency_cache`, `floor_cache` and `outside_cache` (`lightmap.cpp:343-395`). Grouped
/// because a level is meaningless to this pass with any of them missing.
#[derive(Debug, Clone)]
pub struct SunLayer {
    /// The side of the level, in squares.
    pub side: usize,
    /// How much light each square lets through.
    pub transparency: Vec<f32>,
    /// Whether each square has a floor and is therefore shielded from above.
    pub floor: Vec<bool>,
    /// Whether each square is under open sky.
    pub outside: Vec<bool>,
}

/// What the sunlight pass needs to know about the level **above** the one being lit.
///
/// Reference: the three caches `build_sunlight_cache` reads from the previous level - `transparency_cache`, `floor_cache` and the light map itself (`lightmap.cpp:367-371`).
/// They belong together because they are all read at the *previous* level's square, which is the divergence that matters: the roof that stops the sun is a property of
/// the level above, not of the level being lit.
#[derive(Debug, Clone)]
pub struct AboveLayer {
    /// How much light each square of the level above lets through.
    pub transparency: Vec<f32>,
    /// Whether each square of the level above has a floor, which is a ceiling to the light below it.
    pub floor: Vec<bool>,
    /// The light already computed for each square of the level above.
    pub light: Vec<f32>,
}

/// The indoors light level for a level, which depends on the sky rather than on the depth.
///
/// Reference: `lightmap.cpp:338-340`. Above ground under a bright sky, indoors is dim-but-lit (`LIGHT_AMBIENT_DIM * 0.8`); otherwise - at night, or anywhere below ground -
/// it is the dark value, because there is nothing to be dim *about*.
#[must_use]
pub fn inside_light_level(zlev: i32, outside_light_level: f32) -> f32 {
    if zlev >= 0 && outside_light_level > SOURCE_BRIGHT {
        AMBIENT_DIM * 0.8
    } else {
        AMBIENT_LOW
    }
}

/// The sunlight pass, carrying its two flags from one level to the next.
///
/// Reference: the `fully_outside` and `fully_inside` flags of `build_sunlight_cache` (`lightmap.cpp:316-330`), which are deliberately not constants: they are the whole
/// optimisation, and they only make sense in the order the levels are walked.
#[derive(Debug, Clone, Copy)]
pub struct Sunlight {
    fully_outside: bool,
    fully_inside: bool,
}

/// Which quarters a cardinal neighbour lights, in the reference's order: the square itself, north, west, east, south.
///
/// Reference: `cardinals` and `dir_quadrants` (`lightmap.cpp:384-394`). A neighbour is not a lamp: light from the north lights a square's northern quarters, which is what
/// leaves the far quarters at the indoors value and produces the gradient beside a shaft.
const CARDINAL_QUADRANTS: [(i32, i32, [Quadrant; 2]); 4] = [
    (0, -1, [Quadrant::NorthEast, Quadrant::NorthWest]),
    (-1, 0, [Quadrant::SouthWest, Quadrant::NorthWest]),
    (1, 0, [Quadrant::SouthEast, Quadrant::NorthEast]),
    (0, 1, [Quadrant::SouthEast, Quadrant::SouthWest]),
];

impl Sunlight {
    /// The pass at the top of the world.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fully_outside: true,
            fully_inside: false,
        }
    }

    /// Whether nothing has blocked the sun yet.
    #[must_use]
    pub fn fully_outside(&self) -> bool {
        self.fully_outside
    }

    /// Whether nothing gets through at all any more.
    #[must_use]
    pub fn fully_inside(&self) -> bool {
        self.fully_inside
    }

    /// Lights one level with open sky above it.
    ///
    /// Reference: the first level the loop reaches, whose "previous level" is the empty top of the simulated world - open air, no floors, and lit by the sun itself
    /// (`lightmap.cpp:310-315`). Passing that layer in is what makes a single-level world behave like the reference's: the sun arrives from above, not from nowhere.
    #[must_use]
    pub fn step(
        &mut self,
        level: &SunLayer,
        outside_light: f32,
        inside_light: f32,
        sight_penalty: f32,
    ) -> Vec<FourQuadrants> {
        let cells = level.side * level.side;
        let sky = AboveLayer {
            transparency: vec![TRANSPARENCY_OPEN_AIR; cells],
            floor: vec![false; cells],
            light: vec![outside_light; cells],
        };
        self.step_with_above(level, &sky, outside_light, inside_light, sight_penalty)
    }

    /// Lights one level from the light on the level above it.
    ///
    /// Reference: one iteration of `build_sunlight_cache`'s loop (`lightmap.cpp:331-437`). The centre and cardinal tests read the **previous** level's transparency,
    /// floor and light - that is where a ceiling lives - and only the weather reads the level being lit.
    #[must_use]
    pub fn step_with_above(
        &mut self,
        level: &SunLayer,
        above: &AboveLayer,
        outside_light: f32,
        inside_light: f32,
        sight_penalty: f32,
    ) -> Vec<FourQuadrants> {
        let side = level.side;
        let cells = side * side;
        // Nothing reaches this level: the reference fills and moves on without looking at a single square.
        if self.fully_inside {
            return vec![FourQuadrants::uniform(inside_light); cells];
        }
        // Nothing has blocked the sun yet: fill with the sky, then decide which regime the next level is in.
        if self.fully_outside {
            let out = vec![FourQuadrants::uniform(outside_light); cells];
            self.fully_outside = true;
            self.fully_inside = true;
            for x in 0..side {
                for y in 0..side {
                    let index = x * side + y;
                    let transparent = level.transparency.get(index).copied().unwrap_or(0.0);
                    let floor = level.floor.get(index).copied().unwrap_or(true);
                    self.fully_outside =
                        self.fully_outside && transparent >= TRANSPARENCY_OPEN_AIR && !floor;
                    self.fully_inside = self.fully_inside && (transparent <= 0.0 || floor);
                }
            }
            return out;
        }

        let mut lit = vec![FourQuadrants::uniform(inside_light); cells];
        self.fully_inside = true;
        for x in 0..side {
            for y in 0..side {
                let index = x * side + y;
                let outside = level.outside.get(index).copied().unwrap_or(false);
                // The square itself first; if it gives light, the cardinals are never consulted.
                let mut from_above = false;
                if let Some(light) =
                    sunlight_from(above, index, outside, inside_light, sight_penalty)
                {
                    if let Some(cell) = lit.get_mut(index) {
                        *cell = FourQuadrants::uniform(light);
                    }
                    self.fully_inside &= light <= inside_light;
                    from_above = true;
                }
                if from_above {
                    continue;
                }
                for (dx, dy, quarters) in CARDINAL_QUADRANTS {
                    let px = i32::try_from(x).unwrap_or(0) + dx;
                    let py = i32::try_from(y).unwrap_or(0) + dy;
                    let (Ok(pxu), Ok(pyu)) = (usize::try_from(px), usize::try_from(py)) else {
                        continue;
                    };
                    if pxu >= side || pyu >= side {
                        continue;
                    }
                    let previous = pxu * side + pyu;
                    let Some(light) =
                        sunlight_from(above, previous, outside, inside_light, sight_penalty)
                    else {
                        continue;
                    };
                    self.fully_inside &= light <= inside_light;
                    if let Some(cell) = lit.get_mut(index) {
                        for quarter in quarters {
                            if cell.get(quarter) < light {
                                cell.set(quarter, light);
                            }
                        }
                    }
                }
            }
        }
        lit
    }
}

/// The light one square of the level above delivers, if any.
///
/// Reference: the guard and the clamp at `lightmap.cpp:406-420`. Three conditions have to hold on the **previous** level's square - it lets light through, it has no floor,
/// and it is lit - and the answer is `clamp( light * OPEN_AIR / transparency, indoors, light )`, where the transparency is divided by the weather's cost when the square being
/// lit is in the open. That last detail is the reference's own and it looks backwards: dividing by a penalty above one makes the previous square *more* transparent to the
/// lower level, cancelling the weather the cache already charged up there.
fn sunlight_from(
    above: &AboveLayer,
    index: usize,
    outside: bool,
    inside_light: f32,
    sight_penalty: f32,
) -> Option<f32> {
    let light = above.light.get(index).copied().unwrap_or(0.0);
    if light <= 0.0 {
        return None;
    }
    if above.floor.get(index).copied().unwrap_or(true) {
        return None;
    }
    let mut transparency = above.transparency.get(index).copied().unwrap_or(0.0);
    if outside {
        transparency /= sight_penalty;
    }
    if transparency <= 0.0 {
        return None;
    }
    Some((light * TRANSPARENCY_OPEN_AIR / transparency).clamp(inside_light, light))
}

impl Default for Sunlight {
    fn default() -> Self {
        Self::new()
    }
}

/// How much of the light above survives a square's transparency.
///
/// Reference: `clamp( prev_light_max * LIGHT_TRANSPARENCY_OPEN_AIR / prev_transparency, inside_light_level, prev_light_max )` (`lightmap.cpp:419-420`). Open air passes
/// everything, thicker air dims it, thinner air is held at the value above rather than brightened.
#[must_use]
pub fn sunlight_through(
    above: f32,
    transparency: f32,
    _outside: bool,
    inside_light: f32,
    _sight_penalty: f32,
) -> f32 {
    (above * TRANSPARENCY_OPEN_AIR / transparency).clamp(inside_light, above)
}
