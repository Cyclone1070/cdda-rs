//! The zones a map records: labelled rectangles the game's own rules ask about.
//!
//! Reference: `mapgen_place_zone` (`mapgen.cpp:4044-4073`, called from `jmapgen_zone::apply`), whose zone is a **rectangle** between two corners, tagged with a type, an
//! owning faction, a name and a filter.
//!
//! The reference keeps zones in the zone manager, a store of rectangles beside the map. The port keeps each zone as **one entity** carrying its rectangle, because a zone is a
//! thing with a life of its own: a base's workshop can be renamed, a camp's stockpile can be removed, and either way the squares it covers are a property of the zone rather
//! than of every square it touches.

use crate::Position;
use bevy_ecs::prelude::*;

/// One zone: a labelled rectangle.
#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct Zone {
    /// The zone's type, which is what the game's rules ask about (`ZONE_STOCKPILE`, `NPC_NO_INVESTIGATE`, ...).
    pub kind: String,
    /// The faction that owns it, or `no_faction`.
    pub faction: String,
    /// Its name, empty when the data gives none.
    pub name: String,
    /// Its filter, empty when the data gives none.
    pub filter: String,
}

/// Records a zone over a rectangle, its first corner being where the entity stands.
pub fn place(
    world: &mut World,
    near: (i32, i32, i32),
    far: (i32, i32),
    kind: &str,
    faction: &str,
    name: &str,
    filter: &str,
) -> Entity {
    world
        .spawn((
            Position {
                x: near.0,
                y: near.1,
                z: near.2,
            },
            Zone {
                kind: kind.to_owned(),
                faction: faction.to_owned(),
                name: name.to_owned(),
                filter: filter.to_owned(),
            },
            FarCorner { x: far.0, y: far.1 },
        ))
        .id()
}

/// The far corner a zone reaches, kept beside its [`Zone`] because the component names the zone and this names the shape.
#[derive(Debug, Clone, Copy, Component)]
pub struct FarCorner {
    /// The column of the far corner.
    pub x: i32,
    /// The row of the far corner.
    pub y: i32,
}

/// Every zone covering a square, in the order the store lists them.
#[must_use]
pub fn zones_at(world: &mut World, square: (i32, i32, i32)) -> Vec<Zone> {
    let mut query = world.query::<(&Position, &Zone, &FarCorner)>();
    query
        .iter(world)
        .filter(|(near, _, far)| {
            let (x1, x2) = if near.x <= far.x {
                (near.x, far.x)
            } else {
                (far.x, near.x)
            };
            let (y1, y2) = if near.y <= far.y {
                (near.y, far.y)
            } else {
                (far.y, near.y)
            };
            near.z == square.2 && (x1..=x2).contains(&square.0) && (y1..=y2).contains(&square.1)
        })
        .map(|(_, zone, _)| zone.clone())
        .collect()
}

/// How many zones exist.
#[must_use]
pub fn census(world: &mut World) -> usize {
    let mut query = world.query::<&Zone>();
    query.iter(world).count()
}
