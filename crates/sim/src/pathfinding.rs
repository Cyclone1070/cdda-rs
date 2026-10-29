//! Finding a short way across a grid of squares.
//!
//! Reference: `pf::greedy_path` (`src/simple_pathfinding.cpp:48-135`), a greedy best-first search over
//! a rectangle, scored by a caller-supplied function that may refuse a square outright.
//!
//! The reference keeps three arrays the size of the search rectangle - closed, open, dirs - inside the
//! function. So does this: they are **scratch**, local to the call and thrown away, which is not the
//! same thing as a per-square array beside the truth. Nothing survives a search, and the answer is the
//! path it returns.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// One entry in the search's frontier: its priority, the order it was queued in, and the square.
type Frontier = (Reverse<i32>, Reverse<usize>, (i32, i32));

/// What a square costs and how far it looks from the destination, asked of the caller's rule.
pub type Scorer<'a> = &'a dyn Fn((i32, i32), Option<(i32, i32)>) -> Score;

/// Which way a step goes, in the reference's own order: north, east, south, west.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    /// Up the screen.
    North,
    /// Right.
    East,
    /// Down the screen.
    South,
    /// Left.
    West,
}

impl Direction {
    /// Every direction, in the order the reference tries them (`om_direction::all`).
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    /// The step this direction makes, from `om_direction::displace` (`overmap.cpp:7039`).
    #[must_use]
    pub const fn displacement(self) -> (i32, i32) {
        match self {
            Self::North => (0, -1),
            Self::East => (1, 0),
            Self::South => (0, 1),
            Self::West => (-1, 0),
        }
    }

    /// The direction back the way it came, which is what a path node records.
    #[must_use]
    pub const fn reversed(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

/// What a square costs to stand on, and how far it looks from being there yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    /// The cost of the square. Negative means the square cannot be walked at all.
    pub node_cost: i32,
    /// The estimate of the remaining distance.
    pub estimated_dest_cost: i32,
}

impl Score {
    /// A square that cannot be entered.
    pub const REJECTED: Self = Self {
        node_cost: -1,
        estimated_dest_cost: 0,
    };

    /// A square that costs `node_cost` and is `estimated_dest_cost` away.
    #[must_use]
    pub const fn new(node_cost: i32, estimated_dest_cost: i32) -> Self {
        Self {
            node_cost,
            estimated_dest_cost,
        }
    }
}

/// One square on a path, with the direction it was reached from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    /// Column.
    pub x: i32,
    /// Row.
    pub y: i32,
    /// The way back towards the start, or `None` for the start itself.
    pub direction: Option<Direction>,
}

/// The number of steps between two squares, ignoring anything in the way.
#[must_use]
pub fn manhattan_distance(from: (i32, i32), to: (i32, i32)) -> i32 {
    (from.0 - to.0).abs() + (from.1 - to.1).abs()
}

/// The cheapest-looking path from `source` to `dest` inside a `max`-wide rectangle at the origin.
///
/// The reference's search is greedy best-first: it always expands the node with the smallest
/// `node_cost + estimated_dest_cost`, and the first time it reaches `dest` that path is the answer. So
/// the result is short, not shortest - which is the reference's behaviour and the caller's assumption.
///
/// The returned path runs **from the destination back to the source**, the same order the reference
/// returns, with each node recording the direction that leads back towards the start.
#[must_use]
pub fn greedy_path(
    source: (i32, i32),
    dest: (i32, i32),
    max: (i32, i32),
    scorer: Scorer<'_>,
) -> Vec<Node> {
    let inbounds = |(x, y): (i32, i32)| x >= 0 && x < max.0 && y >= 0 && y < max.1;
    if source == dest || !inbounds(source) || !inbounds(dest) {
        return Vec::new();
    }
    if scorer(source, None).node_cost < 0 {
        return Vec::new();
    }
    let width = usize::try_from(max.0.max(0)).unwrap_or(0);
    let height = usize::try_from(max.1.max(0)).unwrap_or(0);
    let size = width.saturating_mul(height);
    let index = |(x, y): (i32, i32)| -> Option<usize> {
        let column = usize::try_from(x).ok()?;
        let row = usize::try_from(y).ok()?;
        if column >= width || row >= height {
            return None;
        }
        Some(row * width + column)
    };
    let mut closed = vec![false; size];
    let mut best: Vec<Option<i32>> = vec![None; size];
    let mut came_from: Vec<Option<Direction>> = vec![None; size];

    // A min-heap keyed on priority, with the insertion number breaking ties so the search is
    // deterministic: the reference's heap leaves ties to the container, which is not a rule.
    let mut open: BinaryHeap<Frontier> = BinaryHeap::new();
    if let Some(start) = index(source) {
        if let Some(slot) = best.get_mut(start) {
            *slot = Some(i32::MAX);
        }
        open.push((Reverse(i32::MAX), Reverse(0_usize), source));
    }
    let mut sequence = 1_usize;
    while let Some((Reverse(priority), _, position)) = open.pop() {
        let Some(node_index) = index(position) else {
            continue;
        };
        let already_closed = closed.get(node_index).copied().unwrap_or(true);
        let known = best.get(node_index).copied().flatten();
        if already_closed || known.is_some_and(|known| known < priority) {
            continue;
        }
        if let Some(slot) = closed.get_mut(node_index) {
            *slot = true;
        }
        if position == dest {
            let mut path = Vec::new();
            let mut step = position;
            let mut heading = index(step).and_then(|at| came_from.get(at).copied().flatten());
            while step != source {
                path.push(Node {
                    x: step.0,
                    y: step.1,
                    direction: heading,
                });
                let Some(towards) = heading else {
                    break;
                };
                let (dx, dy) = towards.displacement();
                step = (step.0 + dx, step.1 + dy);
                heading = index(step).and_then(|at| came_from.get(at).copied().flatten());
            }
            path.push(Node {
                x: step.0,
                y: step.1,
                direction: None,
            });
            return path;
        }
        for heading in Direction::ALL {
            let (dx, dy) = heading.displacement();
            let next = (position.0 + dx, position.1 + dy);
            let Some(next_index) = index(next) else {
                continue;
            };
            if !inbounds(next) || closed.get(next_index).copied().unwrap_or(true) {
                continue;
            }
            let score = scorer(next, Some(position));
            if score.node_cost < 0 {
                continue;
            }
            let priority = score.node_cost + score.estimated_dest_cost;
            let previous_best = best.get(next_index).copied().flatten();
            if previous_best.is_none_or(|known| known > priority) {
                if let Some(slot) = best.get_mut(next_index) {
                    *slot = Some(priority);
                }
                // The way back from the child is the reverse of the step that reached it, which is
                // what `dirs[n] = ( dir + 2 ) % 4` records upstream (`simple_pathfinding.cpp:114`).
                if let Some(slot) = came_from.get_mut(next_index) {
                    *slot = Some(heading.reversed());
                }
                open.push((Reverse(priority), Reverse(sequence), next));
                sequence += 1;
            }
        }
    }
    Vec::new()
}
