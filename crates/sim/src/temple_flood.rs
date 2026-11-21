//! The temple's flood, once it has started.
//!
//! Reference: `timed_event_type::TEMPLE_FLOOD` (`timed_event.cpp:223-280`). The trap starts the water; this is what keeps it rising, one step every two or three turns,
//! until no square changes - shallow water next to deep water becomes deep, and a rock floor next to any water becomes shallow.
//!
//! **The level is read once and the changes applied after every decision is made**, which is the reference's shape: it decides each square from `here.ter` and copies a
//! buffer back at the end. The test proves the two answers a step gives (deep, shallow) and the stop when nothing changes; the read-then-apply ordering is not
//! independently observable in that fixture, because the reference's own predicate never matches a square against itself.

use bevy_ecs::prelude::{With, World};

/// What the flood did this step, which is what the caller needs to decide whether to reschedule it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flood {
    /// Whether any square changed. The reference stops rescheduling when this is false: the chamber is full.
    pub flooded: bool,
    /// Whether the player stood on a square that changed, and whether it became deep water.
    pub reached_player: Option<Reached>,
}

/// How deep the water got where the player was standing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reached {
    /// Water to the knees.
    Knees,
    /// Water to the ceiling, which the reference follows with a forced swim.
    Ceiling,
}

/// One step of the flood, and whether it should be scheduled again.
///
/// The caller owns the calendar and the rolls; this function owns the rule.
pub fn run(world: &mut World) -> Flood {
    let (Some(shallow), Some(deep), Some(floor)) = (
        crate::terrain_types::code_of(world, "t_water_sh"),
        crate::terrain_types::code_of(world, "t_water_dp"),
        crate::terrain_types::code_of(world, "t_rock_floor"),
    ) else {
        return Flood {
            flooded: false,
            reached_player: None,
        };
    };
    // The level as it stands: every ground square, with its code. Read before anything is written, so every decision in this step sees the same world the reference's
    // buffer copy does.
    let standing: Vec<((i32, i32, i32), u32)> = {
        let mut query = world
            .try_query_filtered::<(&crate::Position, &crate::square::Terrain), With<crate::square::Ground>>();
        let Some(query) = query.as_mut() else {
            return Flood {
                flooded: false,
                reached_player: None,
            };
        };
        query
            .iter(world)
            .map(|(position, terrain)| ((position.x, position.y, position.z), terrain.code))
            .collect()
    };
    let at = |square: (i32, i32, i32)| -> Option<u32> {
        standing
            .iter()
            .find(|(place, _)| *place == square)
            .map(|(_, code)| *code)
    };
    let mut changes: Vec<((i32, i32, i32), u32)> = Vec::new();
    for (square, code) in &standing {
        // The reference is silent about the square itself, so it reads the centre like any other neighbour: `points_in_radius(p, 1)` includes `p`.
        let neighbours: Vec<u32> = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
            .filter_map(|(dx, dy)| at((square.0 + dx, square.1 + dy, square.2)))
            .collect();
        if *code == shallow && neighbours.contains(&deep) {
            changes.push((*square, deep));
        } else if *code == floor && (neighbours.contains(&deep) || neighbours.contains(&shallow)) {
            changes.push((*square, shallow));
        }
    }
    if changes.is_empty() {
        return Flood {
            flooded: false,
            reached_player: None,
        };
    }
    // **The player's own square decides the line**, and it is read from the change list rather than from the water: a square that did not change says nothing.
    let reached_player = crate::missions::avatar(world)
        .and_then(|avatar| {
            let position = world.get::<crate::Position>(avatar)?;
            Some((position.x, position.y, position.z))
        })
        .and_then(|place| {
            changes
                .iter()
                .find(|(square, _)| *square == place)
                .map(|(_, code)| {
                    if *code == deep {
                        Reached::Ceiling
                    } else {
                        Reached::Knees
                    }
                })
        });
    for (square, code) in changes {
        crate::square::set_terrain(world, square, code);
    }
    Flood {
        flooded: true,
        reached_player,
    }
}

/// The message the reference prints as the water reaches the player, if it did.
///
/// Reference: `timed_event.cpp:254-268` — "Water quickly floods up to your knees." or "Water fills nearly to the ceiling!"; the deep case is followed by a forced swim,
/// which the port has not built.
#[must_use]
pub fn line(reached: Option<Reached>) -> Option<&'static str> {
    match reached {
        Some(Reached::Knees) => Some("Water quickly floods up to your knees."),
        Some(Reached::Ceiling) => Some("Water fills nearly to the ceiling!"),
        None => None,
    }
}
