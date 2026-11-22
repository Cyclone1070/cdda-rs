//! Rotating a chunk that was just drawn.
//!
//! Reference: `mapgen_function_json::generate` (`mapgen.cpp:6072-6096`) draws the mapgen at rotation zero into a temporarily **pre-rotated** map and then rotates it
//! **back** - the two rotations cancel for everything already standing and leave the newly drawn content turned by `rot`. `rot` is `rotation.get() % 4`, a dice roll the
//! mapgen's data writes, and `point::rotate( turns, dim )` (`point.cpp:25-40`) is the square-level rule: turn 1 sends `( 0, 0 )` to `( dim.y - 1, 0 )`, a quarter turn
//! **clockwise**.
//!
//! The port does the same thing in one step instead of two: every thing drawn in the chunk is moved to its rotated square. Nothing outside the chunk moves, and a chunk
//! rotated by zero takes no pass at all.

use bevy_ecs::prelude::{Entity, World};

/// How many squares a chunk has on a side.
const SIDE: i32 = 24;

/// The square-level rule, exactly `point::rotate( turns, dim )` (`point.cpp:30-37`).
///
/// Turn 1 is a quarter turn **clockwise**, which in a grid whose `y` runs down means a point at the top-left corner lands at the top-right.
#[must_use]
pub fn turn_a_point(point: (i32, i32), turns: i32, side: i32) -> (i32, i32) {
    let turns = turns.rem_euclid(4);
    match turns {
        1 => (side - point.1 - 1, point.0),
        2 => (side - point.0 - 1, side - point.1 - 1),
        3 => (point.1, side - point.0 - 1),
        _ => point,
    }
}

/// Turns everything standing in a chunk by `turns` quarter turns, and answers how many things moved.
///
/// Reference: `map::rotate( turns )` (`mapgen.cpp:7763-7830`) rotates terrain, furniture, traps, items, fields and spawns together; the port has all of those in one store, so
/// one pass over the squares of the chunk moves all of them and keeps the index in step with the store.
pub fn rotate_chunk(world: &mut World, origin: (i32, i32, i32), turns: i32) -> usize {
    let turns = turns.rem_euclid(4);
    if turns == 0 {
        return 0;
    }
    let standing: Vec<(Entity, (i32, i32))> = {
        let mut query = world.query::<(Entity, &crate::Position)>();
        query
            .iter(world)
            .filter(|(_entity, position)| {
                position.z == origin.2
                    && (origin.0..origin.0 + SIDE).contains(&position.x)
                    && (origin.1..origin.1 + SIDE).contains(&position.y)
            })
            .map(|(entity, position)| (entity, (position.x, position.y)))
            .collect()
    };
    let mut moved = 0;
    for (entity, (x, y)) in standing {
        let turned = turn_a_point((x - origin.0, y - origin.1), turns, SIDE);
        let at = (origin.0 + turned.0, origin.1 + turned.1, origin.2);
        if let Some(mut position) = world.get_mut::<crate::Position>(entity) {
            position.x = at.0;
            position.y = at.1;
        }
        crate::singletons::index_mut(world).remove(entity, (x, y, origin.2));
        crate::singletons::index_mut(world).insert(entity, at);
        moved += 1;
    }
    moved
}
