//! Vehicles in the store: one entity per vehicle, holding its parts and their hit points.
//!
//! Reference: `vehicle` (`vehicle.h`) holds `std::vector<vehicle_part>` and answers `parts_at_relative( mount )` over that list. The port keeps the same thing in the one way this
//! architecture has: a vehicle is an **entity** with a [`Vehicle`] component and a [`Position`], its parts are a `Vec` inside that component because a part has no meaning apart
//! from its vehicle and no independent identity - an id into a parts list nobody else holds would be a pointer by another name. The data a vehicle is built from - the part
//! specs and the blueprints - lives in one entity of its own, reached through [`set_store`].

use bevy_ecs::prelude::*;

use crate::Position;

/// The vehicle data a world can build from: the part specs and the blueprints, as the reference's store holds them.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct VehicleStore {
    /// Every part, by id.
    pub parts: std::collections::BTreeMap<String, cdda_content::vehicles::PartSpec>,
    /// Every prototype, by id.
    pub prototypes: std::collections::BTreeMap<String, cdda_content::vehicles::Prototype>,
}

/// The marker that says a single entity holds the vehicle data.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheVehicleStore;

/// Installs the vehicle data, replacing whatever the world held.
pub fn set_store(world: &mut World, materials: cdda_content::vehicles::Vehicles) {
    let holder = crate::singletons::singleton::<VehicleStore, TheVehicleStore>(world);
    world.entity_mut(holder).insert(VehicleStore {
        parts: materials
            .parts
            .into_iter()
            .map(|part| (part.id.clone(), part))
            .collect(),
        prototypes: materials
            .prototypes
            .into_iter()
            .map(|prototype| (prototype.id.clone(), prototype))
            .collect(),
    });
}

/// One part of one vehicle, in the square it occupies and with the damage it has taken.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VehiclePart {
    /// The part's id, which names an entry in the world's [`VehicleStore`].
    pub part: String,
    /// Its offset from the vehicle's origin, in squares.
    pub x: i32,
    /// Its offset from the vehicle's origin, in squares.
    pub y: i32,
    /// Its hit points now; a fresh part starts at its spec's `durability`.
    pub hp: i32,
    /// Whether it has been taken to zero, which the reference calls broken - the part stays and stops working.
    pub broken: bool,
}

/// A vehicle: the parts it is made of, in its blueprint's order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct Vehicle {
    /// Its parts.
    pub parts: Vec<VehiclePart>,
}

/// Builds a vehicle from the world's prototype, standing at a square.
///
/// Reference: `vehicle::init_state` fills a vehicle from its prototype's `parts` list, and a part's hit points start at its `vpart_info` durability. The port refuses when the
/// prototype or one of its parts is not in the store, rather than building a vehicle with holes in it.
pub fn spawn(world: &mut World, prototype: &str, at: (i32, i32, i32)) -> Option<Entity> {
    let holder = crate::singletons::holder::<TheVehicleStore>(world)?;
    let store = world.get::<VehicleStore>(holder)?.clone();
    let blueprint = store.prototypes.get(prototype)?.clone();
    let mut parts = Vec::with_capacity(blueprint.parts.len());
    for placement in &blueprint.parts {
        let spec = store.parts.get(&placement.part)?;
        parts.push(VehiclePart {
            part: placement.part.clone(),
            x: placement.x,
            y: placement.y,
            hp: spec.durability,
            broken: spec.durability <= 0,
        });
    }
    let entity = world
        .spawn((
            Vehicle { parts },
            crate::Position {
                x: at.0,
                y: at.1,
                z: at.2,
            },
        ))
        .id();
    // A vehicle stands on every square its parts occupy, so the index holds it there as it is built; a rebuild derives the same entries from the same rule.
    let squares = occupied_squares(world, entity);
    let mut index = crate::singletons::index_mut(world);
    for square in squares {
        index.insert(entity, square);
    }
    Some(entity)
}

/// The indices of the parts standing in one square of a vehicle.
///
/// Reference: `vehicle::parts_at_relative( const point_rel_ms &, bool use_variant )` - several parts may share a square, which is what a frame with a seat, a seatbelt and
/// controls on it is.
#[must_use]
pub fn parts_at(world: &World, vehicle: Entity, x: i32, y: i32) -> Vec<usize> {
    world
        .get::<Vehicle>(vehicle)
        .map(|built| {
            built
                .parts
                .iter()
                .enumerate()
                .filter(|(_, part)| part.x == x && part.y == y)
                .map(|(index, _)| index)
                .collect()
        })
        .unwrap_or_default()
}

/// The squares a vehicle stands on, sorted and without repeats.
///
/// Reference: `vehicle::get_points` (`vehicle.cpp:8186-8204`) walks the parts by **mount** and takes one `abs_part_pos` per distinct mount, which is
/// `pos_abs() + precalc[0]` (`vehicle.cpp:3772-3775`). A placed vehicle starts with its pivot at the origin (`mapgen.cpp:7546-7550`) facing the rotation the placement gave it,
/// and a mapgen entry that names no rotation faces 0 (`mapgen.cpp:3316-3320`); at 0 degrees `coord_translate` moves a mount by nothing, so the square is the origin plus the mount.
///
/// **Rotation is not transcribed yet**: every vehicle the port builds faces 0, and a caller that needs a turned vehicle has to bring `precalc_mounts` with it.
#[must_use]
pub fn occupied_squares(world: &World, vehicle: Entity) -> Vec<(i32, i32, i32)> {
    let Some(at) = world.get::<Position>(vehicle) else {
        return Vec::new();
    };
    let Some(built) = world.get::<Vehicle>(vehicle) else {
        return Vec::new();
    };
    let mut squares = std::collections::BTreeSet::new();
    for part in &built.parts {
        squares.insert((at.x + part.x, at.y + part.y, at.z));
    }
    squares.into_iter().collect()
}

/// The vehicle standing on one square, if one does.
///
/// Reference: `map::veh_at` (`map.cpp:1340-1356`) never scans the level's vehicles; it asks the level's vehicle cache, and the port's cache is the one [`crate::PositionIndex`].
/// The reference also answers **which part**, because its cache stores one; the port's index stores the vehicle, and a part's square is derived from the vehicle's own mount, so
/// [`parts_at`] with the mount answers that at facing 0.
#[must_use]
pub fn at(world: &World, square: (i32, i32, i32)) -> Option<Entity> {
    let index = crate::singletons::index(world)?;
    index
        .at_level(square.0, square.1, square.2)
        .iter()
        .copied()
        .find(|entity| world.get::<Vehicle>(*entity).is_some())
}

/// Applies one blow to one part of a vehicle, answering what became of it.
///
/// Reference: `vehicle::damage_direct` (`vehicle.cpp:7925-7945`), whose rules are [`crate::vehicle_damage::hit`]. A part that reaches zero hit points is **broken and stays**.
pub fn damage(
    world: &mut World,
    vehicle: Entity,
    index: usize,
    amount: i32,
    kind: &str,
    no_resist: bool,
) -> crate::vehicle_damage::Outcome {
    let Some(spec) = part_spec(world, vehicle, index) else {
        return crate::vehicle_damage::Outcome::Ignored;
    };
    let Some(current) = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
        .map(|part| part.hp)
    else {
        return crate::vehicle_damage::Outcome::Ignored;
    };
    let outcome = crate::vehicle_damage::hit(&spec, current, amount, kind, no_resist);
    if let crate::vehicle_damage::Outcome::Applied { hp_after, broke, .. } = outcome
        && let Some(mut built) = world.get_mut::<Vehicle>(vehicle)
        && let Some(part) = built.parts.get_mut(index)
    {
        part.hp = hp_after;
        part.broken = part.broken || broke;
    }
    outcome
}

/// The spec of one part of one vehicle, copied out of the world's store.
fn part_spec(world: &World, vehicle: Entity, index: usize) -> Option<cdda_content::vehicles::PartSpec> {
    let store = crate::singletons::holder::<TheVehicleStore>(world)?;
    let name = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
        .map(|part| part.part.clone())?;
    world.get::<VehicleStore>(store)?.parts.get(&name).cloned()
}
