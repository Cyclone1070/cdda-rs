//! Which squares are under open sky.
//!
//! Reference: `map::build_outside_cache( zlev )` (`src/map.cpp:9260-9314`). Every light question in the game eventually asks this - is the square rained on, does the sun
//! reach it, is it dim inside today - and the answer is a single boolean per square, decided by a rule with only two branches:
//!
//! * below ground, everything is inside;
//! * otherwise everything starts outside, and each square flagged `INDOORS` stamps a **3x3 block of inside** centred on itself.
//!
//! The stamp is what makes this more than a copy of the flag: a doorway, a threshold and a wall's own square all count as indoors without any of them being listed, because
//! they are within one square of a tile that is.
//!
//! The reference builds the answer in a cache **padded by one square on every side**, so a stamp at the map's edge can write all nine of its squares and let the padding
//! swallow the two that fall outside. That is cheaper than a bounds check per square and it is also the reason an indoor tile in the corner still marks its whole
//! neighbourhood.

/// What the outside cache needs to know about a level.
///
/// Reference: the input of `build_outside_cache` (`map.cpp:9286-9300`) - the level's depth and, per square, whether the terrain **or** the furniture carries
/// `TFLAG_INDOORS`. The disjunction is the caller's, because the flag lives on two registries that both contribute.
#[derive(Debug, Clone)]
pub struct OutsideFacts {
    /// The side of the level, in squares.
    pub side: usize,
    /// Whether each square's terrain or furniture is flagged as indoors.
    pub indoors: Vec<bool>,
}

/// Decides which squares of a level are under open sky.
///
/// Reference: `map::build_outside_cache` (`map.cpp:9260-9314`). Below ground there is one fill and no per-square work; above ground every indoor square stamps the
/// three-by-three block around it.
#[must_use]
pub fn outside_of(zlev: i32, facts: &OutsideFacts) -> Vec<bool> {
    let side = facts.side;
    let cells = side * side;
    if zlev < 0 {
        return vec![false; cells];
    }

    // The padded cache, so a stamp at the edge needs no bounds check: one square of padding on every side, all of it starting as sky.
    let padded_side = side + 2;
    let mut padded = vec![true; padded_side * padded_side];
    for x in 0..side {
        for y in 0..side {
            if !facts.indoors.get(x * side + y).copied().unwrap_or(false) {
                continue;
            }
            for dx in 0..3 {
                for dy in 0..3 {
                    // The padding is one square, so the stamp runs from one before the square to one after it.
                    if let Some(slot) = padded.get_mut((x + dx) * padded_side + (y + dy)) {
                        *slot = false;
                    }
                }
            }
        }
    }

    let mut outside = Vec::with_capacity(cells);
    for x in 0..side {
        for y in 0..side {
            outside.push(
                padded
                    .get((x + 1) * padded_side + (y + 1))
                    .copied()
                    .unwrap_or(true),
            );
        }
    }
    outside
}
