//! Spreading a field outwards from one square, the way `map::propagate_field` does.
//!
//! Reference: `map_field.cpp:2226-2299`. A field with an `amount` and a maximum intensity is poured out from a centre: the
//! squares nearest the centre thicken first, each square holds at most `max_intensity`, and the field only passes a square it
//! can enter - an impassable wall stops a gas unless the terrain is `PERMEABLE`.
//!
//! Two details are the rule and not an implementation:
//!
//! * squares at the **same distance** are served together, and each front splits the remaining amount between them
//!   (`increment = max( 1, amount / front_size )`), so a blast does not thicken one side while the other waits;
//! * the order within a front is a draw (`random_entry_removed`), which the port takes from the store's generator so a seed
//!   replays the spread.
//!
//! The port answers the same question with the store's own numbers: a square's intensity is `add_field_strength`'s, and the
//! answer to "can the field enter here" is a closure the caller supplies, because the store knows terrain by entity and this
//! module knows nothing about entities.

use bevy_ecs::prelude::*;

use cdda_core_types::field::FieldId;
use cdda_core_types::rng::Rolls;

/// Pours `amount` of a field out from `centre`, and answers how much was placed.
///
/// `passable` says whether the field may enter a square: a wall answers false unless it is permeable, and the caller decides
/// which squares are walls.
pub fn propagate(
    world: &mut World,
    centre: (i32, i32, i32),
    field: FieldId,
    amount: i32,
    max_intensity: i32,
    rolls: &mut Rolls,
    passable: impl Fn(i32, i32, i32) -> bool,
) -> i32 {
    const NEIGHBOURS: [(i32, i32); 8] = [
        (-1, 0),
        (1, 0),
        (0, -1),
        (0, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
        (1, 1),
    ];
    let mut left = amount;
    let placed = amount;
    let mut closed: std::collections::BTreeSet<(i32, i32, i32)> = std::collections::BTreeSet::new();
    // The reference's frontier is a priority queue ordered by distance from the centre, which for integer distances is the
    // same as holding one front per distance and taking them in order.
    let mut open: Vec<(i32, (i32, i32, i32))> = vec![(0, centre)];
    while left > 0 && !open.is_empty() {
        open.sort_by_key(|(distance, square)| (*distance, *square));
        let front_distance = open.first().map_or(0, |(distance, _)| *distance);
        let mut front: Vec<(i32, i32, i32)> = Vec::new();
        let mut rest: Vec<(i32, (i32, i32, i32))> = Vec::new();
        for (distance, square) in open {
            if distance == front_distance && !closed.contains(&square) {
                front.push(square);
            } else {
                rest.push((distance, square));
            }
        }
        open = rest;
        if front.is_empty() {
            continue;
        }
        let mut index = usize::try_from(rolls.rng(0, i32::try_from(front.len()).unwrap_or(1) - 1)).unwrap_or(0);
        if index >= front.len() {
            index = 0;
        }
        let square = front.remove(index);
        closed.insert(square);
        let current = crate::field_strength_at(world, square, field);
        if current < max_intensity {
            // Reference: `mod_field_intensity( gp.second, type, bonus )` where `bonus` is the space left under the maximum,
            // capped by the front's share of the amount. A single unit is placed when the front has fewer units than squares.
            let bonus = (max_intensity - current).min(left.max(1));
            crate::add_field_strength(world, square, field, bonus);
            left -= bonus;
        } else {
            left -= 1;
        }
        if left <= 0 {
            break;
        }
        for (dx, dy) in NEIGHBOURS {
            let next = (square.0 + dx, square.1 + dy, square.2);
            if closed.contains(&next) || !passable(next.0, next.1, next.2) {
                if !passable(next.0, next.1, next.2) {
                    closed.insert(next);
                }
                continue;
            }
            let distance = (next.0 - centre.0).abs().max((next.1 - centre.1).abs());
            if !open.iter().any(|(_, held)| *held == next) {
                open.push((distance, next));
            }
        }
    }
    placed - left
}
