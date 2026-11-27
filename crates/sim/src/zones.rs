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
            ZoneEnabled(true),
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

/// Whether a zone is switched on. A zone without this component is **on**: `place` writes `true`, and the queries below read a missing component as enabled so a zone made
/// straight from components is not silently invisible.
///
/// Reference: `zone_data::enabled`, whose manager builds its caches from the enabled zones only (`clzones.cpp:940-968`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct ZoneEnabled(pub bool);

/// Whether a zone covers a square, both corners inclusive.
///
/// Reference: `zone_data::has_inside` (`clzones.h:533-539`), an `inclusive_cuboid::contains`. The corners are a pair rather than a direction: `place` keeps the first corner where the
/// entity stands and the far one beside it, and the two may be written in any order.
#[must_use]
pub fn covers(near: (i32, i32), far: (i32, i32), level: i32, point: (i32, i32, i32)) -> bool {
    let (x1, x2) = if near.0 <= far.0 {
        (near.0, far.0)
    } else {
        (far.0, near.0)
    };
    let (y1, y2) = if near.1 <= far.1 {
        (near.1, far.1)
    } else {
        (far.1, near.1)
    };
    level == point.2 && (x1..=x2).contains(&point.0) && (y1..=y2).contains(&point.1)
}

/// The distance zones are measured by: **the larger of the two axes**, not the diagonal.
///
/// Reference: `square_dist( const tripoint &, const tripoint & )` (`line.cpp:1234-1240`), `max( |dx|, |dy| )`. The vertical axis is not in it.
#[must_use]
pub fn square_dist(a: (i32, i32, i32), b: (i32, i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// The squares of the zones of a kind and faction within `range` of a place, in a stable order.
///
/// Reference: `zone_manager::get_near( type, where, range, it, fac )` (`clzones.cpp:1172-1200`): every square of every **enabled** zone of that type and faction with
/// `square_dist( point, where ) <= range`, the bound **inclusive**. The reference walks a hash set and returns a hash set; the port returns the squares sorted, because a rule whose
/// answer depends on iteration order cannot be compared with itself.
///
/// **Named gaps**: the `LOOT_CUSTOM`/`LOOT_ITEM_GROUP` filter on the item being moved, the vehicle-zone half of the function, the personal shift and the `invert` flag.
#[must_use]
pub fn zones_near(
    world: &mut World,
    kind: &str,
    faction: &str,
    where_at: (i32, i32, i32),
    range: i32,
) -> Vec<(i32, i32, i32)> {
    let mut query = world.query::<(&Position, &Zone, &FarCorner, Option<&ZoneEnabled>)>();
    let mut near = std::collections::BTreeSet::new();
    for (near_corner, zone, far, enabled) in query.iter(world) {
        if zone.kind != kind || zone.faction != faction || !enabled.is_none_or(|on| on.0) {
            continue;
        }
        let (x1, x2) = (near_corner.x.min(far.x), near_corner.x.max(far.x));
        let (y1, y2) = (near_corner.y.min(far.y), near_corner.y.max(far.y));
        for y in y1..=y2 {
            for x in x1..=x2 {
                let point = (x, y, near_corner.z);
                if square_dist(point, where_at) <= range {
                    near.insert(point);
                }
            }
        }
    }
    near.into_iter().collect()
}

/// The zone of a kind and faction that covers a square, or `None`.
///
/// Reference: `zone_manager::get_zone_at( where, type, fac )` (`clzones.cpp:1120-1145`), which walks the zones in **reverse** so the one added **last** wins where two overlap.
/// The port asks for the newest entity id, which is the same answer without keeping a list to reverse.
#[must_use]
pub fn zone_at(
    world: &mut World,
    kind: &str,
    faction: &str,
    point: (i32, i32, i32),
) -> Option<Zone> {
    let mut query = world.query::<(Entity, &Position, &Zone, &FarCorner, Option<&ZoneEnabled>)>();
    let mut found: Option<(Entity, Zone)> = None;
    for (entity, near_corner, zone, far, enabled) in query.iter(world) {
        if zone.kind != kind || zone.faction != faction || !enabled.is_none_or(|on| on.0) {
            continue;
        }
        if !covers(
            (near_corner.x, near_corner.y),
            (far.x, far.y),
            near_corner.z,
            point,
        ) {
            continue;
        }
        if found
            .as_ref()
            .is_none_or(|(newest, _)| entity.index() > newest.index())
        {
            found = Some((entity, zone.clone()));
        }
    }
    found.map(|(_, zone)| zone)
}
