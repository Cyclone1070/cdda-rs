//! Scent diffusion: how a smell spreads one square at a time and fades.
//!
//! Reference: `scent_map::update` and `scent_map::decay` (`src/scent_map.cpp:60-285`). Each turn a
//! square keeps most of its scent and hands a little to its eight neighbours; `NO_SCENT` terrain
//! blocks the exchange entirely and sets its own square to zero, while `REDUCE_SCENT` terrain takes
//! part at a fifth of the weight. `decay` then takes one point off every square that has any.
//!
//! The port keeps the arithmetic and the two terrain rules and leaves the player-facing parts - the
//! radius around the character, the pause after standing still, the scent types - to the caller.

/// How strongly scent moves between squares, as the reference fixes it: under 125 for stability.
pub const DIFFUSIVITY: i32 = 100;

/// What the map says about one square, as far as scent is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScentCell {
    /// Scent strength at the square, before the turn.
    pub value: i32,
    /// `NO_SCENT`: nothing passes in or out, and the square is emptied.
    pub blocks: bool,
    /// `REDUCE_SCENT`: scent crosses at a fifth of the weight.
    pub reduces: bool,
}

/// Runs one turn of diffusion over a square grid given row by row.
///
/// # Panics
///
/// Does not panic: the grid is checked against `side` first and every index below is inside it.
/// The grid is `values.len()` rows of equal length; squares outside it are simply absent, which is
/// how the reference's margin behaves.
// His loops run over the grid's own dimensions; clippy cannot see that.
#[allow(clippy::indexing_slicing)]
pub fn diffuse_step(cells: &mut [ScentCell], side: usize) {
    if side == 0 || cells.len() != side * side {
        return;
    }
    let at = |x: i32, y: i32| -> Option<ScentCell> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        if x >= side || y >= side {
            return None;
        }
        cells.get(y * side + x).copied()
    };
    // The reference sums its neighbours in the y direction first, then reads three such columns.
    let mut sums = vec![(0_i32, 0_i32); side * side];
    for x in 0..side {
        for y in 0..side {
            let (mut sum, mut used) = (0, 0);
            for dy in -1..=1 {
                let Some(neighbour) = at(
                    i32::try_from(x).unwrap_or(0),
                    i32::try_from(y).unwrap_or(0) + dy,
                ) else {
                    continue;
                };
                if neighbour.blocks {
                    continue;
                }
                if neighbour.reduces {
                    sum += 2 * neighbour.value;
                    used += 2;
                } else {
                    sum += 10 * neighbour.value;
                    used += 10;
                }
            }
            sums[y * side + x] = (sum, used);
        }
    }
    for x in 0..side {
        for y in 0..side {
            let index = y * side + x;
            let Some(here) = cells.get(index).copied() else {
                continue;
            };
            if here.blocks {
                if let Some(cell) = cells.get_mut(index) {
                    cell.value = 0;
                }
                continue;
            }
            let column = |dx: i32| -> (i32, i32) {
                let column = i32::try_from(x).unwrap_or(0) + dx;
                if column < 0 || usize::try_from(column).map_or(true, |c| c >= side) {
                    (0, 0)
                } else {
                    sums.get(y * side + usize::try_from(column).unwrap_or(0))
                        .copied()
                        .unwrap_or((0, 0))
                }
            };
            let (left, middle, right) = (column(-1), column(0), column(1));
            let squares_used = left.1 + middle.1 + right.1;
            let this_diffusivity = if here.reduces {
                DIFFUSIVITY / 5
            } else {
                DIFFUSIVITY
            };
            let mut temp = here.value * (10 * 1000 - squares_used * this_diffusivity);
            temp -= here.value * this_diffusivity * (90 - squares_used) / 5;
            let spread = this_diffusivity * (left.0 + middle.0 + right.0);
            if let Some(cell) = cells.get_mut(index) {
                cell.value = (temp + spread) / (1000 * 10);
            }
        }
    }
}

/// Takes one point of scent off every square that has any, which is `scent_map::decay`.
pub fn decay(cells: &mut [ScentCell]) {
    for cell in cells.iter_mut() {
        cell.value = (cell.value - 1).max(0);
    }
}
