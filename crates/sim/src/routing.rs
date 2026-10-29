//! Asking the world for a route, and letting the store say what each square costs.
//!
//! The search is [`crate::pathfinding::greedy_path`], which knows nothing about terrain: it asks a
//! scorer about every square it looks at. This module is that scorer, built from the world - the
//! square's terrain code, the code's `move_cost` from the loaded table, and the reference's rule for
//! turning two of those into the cost of a step ([`crate::creature::combined_move_cost`]).
//!
//! Reference: `map::route` (`src/map.cpp`) takes exactly this shape, a `cost` function the caller
//! supplies. The difference is that the reference's caller closes over the map object; here the scorer
//! closes over an `&World`, and there is nothing else to close over.

use crate::creature::combined_move_cost;
use crate::pathfinding::{Direction, Node, Score, Scorer, greedy_path};
use bevy_ecs::prelude::*;

/// What one square costs to stand on, or `None` when it cannot be entered.
///
/// A square with no terrain entity, or whose code the loaded table never described, cannot be entered:
/// answering zero would be pretending the world knows it is open, and answering a guess would put a
/// number in the simulation that the reference never had.
#[must_use]
pub fn square_cost(world: &World, square: (i32, i32, i32)) -> Option<i32> {
    let code = crate::square::terrain_at(world, square)?;
    let cost = crate::singletons::prototypes(world)?.terrain.move_cost(code)?;
    if cost <= 0 { None } else { Some(cost) }
}

/// A scorer over the world, for a search that ends at `dest`.
///
/// The cost of a step is `combined_movecost`'s: both squares charged for, scaled by how many axes
/// differ. The estimate is the reference's own - one step's worth per square of Manhattan distance -
/// which is what makes the search greedy rather than exhaustive.
pub fn scorer(world: &World, dest: (i32, i32)) -> impl Fn((i32, i32), Option<(i32, i32)>) -> Score + '_ {
    move |position: (i32, i32), previous: Option<(i32, i32)>| {
        let Some(cost_here) = square_cost(world, (position.0, position.1, 0)) else {
            return Score::REJECTED;
        };
        let cost_before = previous
            .and_then(|behind| square_cost(world, (behind.0, behind.1, 0)))
            .unwrap_or(cost_here);
        let different_axes = previous.map_or(1, |behind| {
            usize::from(behind.0 != position.0) + usize::from(behind.1 != position.1)
        });
        let step = combined_move_cost(cost_before, cost_here, 0, different_axes);
        let estimate = crate::pathfinding::manhattan_distance(position, dest) * 100;
        Score::new(step, estimate)
    }
}

/// A route from `from` to `to`, across the squares the world describes.
///
/// The path runs from the destination back to the source, exactly as
/// [`crate::pathfinding::greedy_path`] returns it, and is empty when no way through exists.
#[must_use]
pub fn route(world: &World, from: (i32, i32), to: (i32, i32), max: (i32, i32)) -> Vec<Node> {
    let scorer = scorer(world, to);
    greedy_path(from, to, max, &scorer)
}

/// The direction a path leads away from its source, which is what a creature walking it would take.
#[must_use]
pub fn first_step_towards_the_destination(path: &[Node]) -> Option<Direction> {
    path.last().and_then(|node| node.direction)
}

/// The scorer's shape, for callers that want to name it.
pub type WorldScorer<'a> = Scorer<'a>;
