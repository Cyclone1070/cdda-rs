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

/// What walking one step towards a target did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walk {
    /// It stepped, and paid for the step.
    Stepped,
    /// This step reached the target.
    Arrived,
    /// It has fewer points than the step costs, so it stayed put.
    TooSlow,
    /// There is no way through from where it stands.
    NoRoute,
}

/// Walks a creature one step towards `target`, asking the world for the route first.
///
/// The creature does not know the way; it asks [`route`] each time it moves. That is the reference's
/// shape too - `monster::move` re-runs its path search rather than keeping one - and it means a creature
/// reacts to a door closing without anything having to invalidate a cached path.
pub fn walk_towards(
    world: &mut World,
    entity: bevy_ecs::prelude::Entity,
    target: (i32, i32, i32),
    bounds: (i32, i32),
) -> Walk {
    let Some(position) = world.get::<crate::Position>(entity).copied() else {
        return Walk::NoRoute;
    };
    let from = (position.x, position.y, position.z);
    if from == target {
        return Walk::Arrived;
    }
    let path = route(world, (from.0, from.1), (target.0, target.1), bounds);
    // The path runs from the destination back to the source, so the square to step into is the one
    // just before the last - the source itself is last, and it has no direction.
    let Some(step) = path.len().checked_sub(2).and_then(|before| path.get(before)) else {
        return Walk::NoRoute;
    };
    let next = (step.x, step.y, from.2);
    let Some(cost_here) = square_cost(world, from) else {
        return Walk::NoRoute;
    };
    let Some(cost_next) = square_cost(world, next) else {
        return Walk::NoRoute;
    };
    let axes = usize::from(next.0 != from.0) + usize::from(next.1 != from.1);
    let cost = combined_move_cost(cost_here, cost_next, 0, axes);
    match crate::creature::try_step(world, entity, next, cost) {
        crate::creature::StepOutcome::Moved if next == target => Walk::Arrived,
        crate::creature::StepOutcome::Moved => Walk::Stepped,
        crate::creature::StepOutcome::TooSlow => Walk::TooSlow,
    }
}

/// The direction a path leads away from its source, which is what a creature walking it would take.
#[must_use]
pub fn first_step_towards_the_destination(path: &[Node]) -> Option<Direction> {
    path.last().and_then(|node| node.direction)
}

/// The scorer's shape, for callers that want to name it.
pub type WorldScorer<'a> = Scorer<'a>;
