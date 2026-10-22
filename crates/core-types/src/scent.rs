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

/// How many turns a character may stand still before the map stops updating its scent.
///
/// Reference: `else if( player_last_moved + 1000_turns < calendar::turn ) { return; }` (`scent_map.cpp:178`). Standing still does
/// not reset the clock, so the pause holds until the character moves again.
pub const SCENT_PAUSE: i64 = 1_000;

/// The reference's `SCENT_RADIUS` (`scent_map.cpp:18`).
pub const SCENT_RADIUS: i32 = 40;

/// When scent last moved, and from where.
///
/// Reference: the two file-scope statics in `scent_map::update` - `player_last_position` and `player_last_moved`. They are state of
/// the map's, not of the call, which is exactly the sort of hidden cross-call state the port keeps explicit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScentClock {
    last_position: Option<(i32, i32)>,
    last_moved: i64,
}

impl ScentClock {
    /// Whether the turn at `now` should update the map at all, remembering the character's square.
    ///
    /// The order mirrors the reference: a **new** square resets the clock and updates; an unchanged square updates only while the
    /// pause has not run out.
    pub fn should_update(&mut self, center: (i32, i32), now: i64) -> bool {
        if self.last_position != Some(center) {
            self.last_position = Some(center);
            self.last_moved = now;
            return true;
        }
        self.last_moved + SCENT_PAUSE >= now
    }
}

/// The square of the map that scent is tracked over: the character's surroundings, not the whole map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScentWindow {
    min_x: i32,
    min_y: i32,
    side: i32,
    /// The scent values, and what the map says about each square. Row by row, row `y - min_y`.
    cells: Vec<ScentCell>,
}

/// The window around a square, with the reference's radius.
#[must_use]
pub fn scent_window(center: (i32, i32)) -> ScentWindow {
    ScentWindow::around(center)
}

impl ScentWindow {
    /// The window around `center`: the radius on every side, corners included.
    #[must_use]
    pub fn around(center: (i32, i32)) -> Self {
        let side = SCENT_RADIUS * 2 + 1;
        Self {
            min_x: center.0 - SCENT_RADIUS,
            min_y: center.1 - SCENT_RADIUS,
            side,
            cells: vec![ScentCell::default(); usize::try_from(side * side).unwrap_or(0)],
        }
    }

    /// The leftmost tracked column.
    #[must_use]
    pub fn min_x(&self) -> i32 {
        self.min_x
    }

    /// The rightmost tracked column.
    #[must_use]
    pub fn max_x(&self) -> i32 {
        self.min_x + self.side - 1
    }

    /// The topmost tracked row.
    #[must_use]
    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    /// The bottommost tracked row.
    #[must_use]
    pub fn max_y(&self) -> i32 {
        self.min_y + self.side - 1
    }

    /// How many squares across the window is.
    #[must_use]
    pub fn side(&self) -> i32 {
        self.side
    }

    /// Whether a square is tracked at all.
    #[must_use]
    pub fn holds(&self, square: (i32, i32)) -> bool {
        (self.min_x..=self.max_x()).contains(&square.0)
            && (self.min_y..=self.max_y()).contains(&square.1)
    }
}

impl ScentWindow {
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if !self.holds((x, y)) {
            return None;
        }
        let column = usize::try_from(x - self.min_x).ok()?;
        let row = usize::try_from(y - self.min_y).ok()?;
        Some(row * usize::try_from(self.side).ok()? + column)
    }

    /// The scent on a square; untracked squares read as none.
    #[must_use]
    pub fn get(&self, x: i32, y: i32) -> i32 {
        self.index(x, y)
            .and_then(|index| self.cells.get(index))
            .map_or(0, |cell| cell.value)
    }

    /// Puts scent on a square.
    pub fn set(&mut self, x: i32, y: i32, value: i32) {
        if let Some(cell) = self.index(x, y).and_then(|index| self.cells.get_mut(index)) {
            cell.value = value;
        }
    }

    /// Marks a square as holding the smell in: `NO_SCENT` terrain.
    pub fn block(&mut self, x: i32, y: i32) {
        if let Some(cell) = self.index(x, y).and_then(|index| self.cells.get_mut(index)) {
            cell.blocks = true;
        }
    }

    /// Marks a square as passing scent at a fifth of the weight: `REDUCE_SCENT` terrain.
    pub fn reduce(&mut self, x: i32, y: i32) {
        if let Some(cell) = self.index(x, y).and_then(|index| self.cells.get_mut(index)) {
            cell.reduces = true;
        }
    }

    /// Runs one turn of the reference's diffusion over the window.
    ///
    /// Reference: `scent_map::update` (`scent_map.cpp:208-258`). The reference sums each column's three neighbours first and then
    /// combines three columns - "each square gets called 3 times instead of 9 times" - so the arithmetic is a **weighted average**
    /// over the squares that take part: plain squares count ten times, `REDUCE_SCENT` squares twice, and `NO_SCENT` squares neither
    /// give nor take. The expressions are the reference's own, integer division included, because the port's claim is that a world
    /// run twice runs the same.
    // Its loops run over the window's own dimensions, which clippy cannot see.
    #[allow(clippy::indexing_slicing)]
    pub fn diffuse(&mut self) {
        let side = usize::try_from(self.side).unwrap_or(0);
        if side == 0 || self.cells.len() != side * side {
            return;
        }
        // One square wider on each side in x, as the reference's sum matrices are.
        let stride = side + 2;
        let mut sums = vec![0i64; stride * side];
        let mut used = vec![0i64; stride * side];
        for column in 0..stride {
            for row in 0..side {
                let mut sum = 0i64;
                let mut weight_here = 0i64;
                for offset in [-1i64, 0, 1] {
                    let Some(neighbour) = row.checked_add_signed(
                        usize::try_from(offset.unsigned_abs()).map_or(0, |step| {
                            if offset < 0 {
                                -isize::try_from(step).unwrap_or(0)
                            } else {
                                isize::try_from(step).unwrap_or(0)
                            }
                        }),
                    ) else {
                        continue;
                    };
                    if neighbour >= side {
                        continue;
                    }
                    let cell = self
                        .cells
                        .get(neighbour * side + column.saturating_sub(1))
                        .copied()
                        .unwrap_or_default();
                    if cell.blocks {
                        continue;
                    }
                    let weight = if cell.reduces { 2 } else { 10 };
                    sum += weight * i64::from(cell.value);
                    weight_here += weight;
                }
                sums[row * stride + column] = sum;
                used[row * stride + column] = weight_here;
            }
        }
        for column in 0..side {
            for row in 0..side {
                let index = row * side + column;
                let Some(cell) = self.cells.get(index).copied() else {
                    continue;
                };
                if cell.blocks {
                    if let Some(cell) = self.cells.get_mut(index) {
                        cell.value = 0;
                    }
                    continue;
                }
                let columns = [column, column + 1, column + 2];
                let squares_used: i64 = columns
                    .iter()
                    .filter_map(|offset| used.get(row * stride + offset))
                    .sum();
                let diffusivity: i64 = if cell.reduces { 20 } else { 100 };
                let scent_here = i64::from(cell.value);
                let mut temp = scent_here * (10 * 1_000 - squares_used * diffusivity);
                temp -= scent_here * diffusivity * (90 - squares_used) / 5;
                let incoming: i64 = columns
                    .iter()
                    .filter_map(|offset| sums.get(row * stride + offset))
                    .sum();
                let next = (temp + diffusivity * incoming) / (1_000 * 10);
                if let Some(cell) = self.cells.get_mut(index) {
                    cell.value = i32::try_from(next).unwrap_or(0);
                }
            }
        }
    }
}

impl ScentWindow {
    /// Moves what the window holds, as the map moves under it.
    ///
    /// Reference: `scent_map::shift` (`scent_map.cpp:82-99`), called when the reality bubble moves (`game.cpp:13125`). Two details
    /// matter and both are in the reference's loop bounds: the walk starts from the side the data is moving **towards**, so a square
    /// is always read before it is written over - walking the other way smears every value across the window - and squares the
    /// window uncovers come back **empty**, because a stale value there would claim scent that nothing produced.
    // The loops run over the window's own dimensions, which clippy cannot see.
    #[allow(clippy::indexing_slicing)]
    pub fn shift(&mut self, dx: i32, dy: i32) {
        let side = usize::try_from(self.side).unwrap_or(0);
        if side == 0 || self.cells.len() != side * side {
            return;
        }
        let step_x: i32 = if dx >= 0 { 1 } else { -1 };
        let step_y: i32 = if dy >= 0 { 1 } else { -1 };
        let mut x: i32 = if dx >= 0 { 0 } else { self.side - 1 };
        while (0..self.side).contains(&x) {
            let mut y: i32 = if dy >= 0 { 0 } else { self.side - 1 };
            while (0..self.side).contains(&y) {
                let source_x = x + dx;
                let source_y = y + dy;
                let value =
                    if (0..self.side).contains(&source_x) && (0..self.side).contains(&source_y) {
                        let source = usize::try_from(source_y).unwrap_or(0) * side
                            + usize::try_from(source_x).unwrap_or(0);
                        self.cells.get(source).copied().unwrap_or_default()
                    } else {
                        ScentCell::default()
                    };
                let destination =
                    usize::try_from(y).unwrap_or(0) * side + usize::try_from(x).unwrap_or(0);
                self.cells[destination] = value;
                y += step_y;
            }
            x += step_x;
        }
    }
}
