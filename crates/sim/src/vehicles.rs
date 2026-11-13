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

/// The store's spec for a part id, if the world holds the store and the store holds the part.
fn spec_of<'a>(world: &'a World, part: &str) -> Option<&'a cdda_content::vehicles::PartSpec> {
    let holder = crate::singletons::holder::<TheVehicleStore>(world)?;
    world.get::<VehicleStore>(holder)?.parts.get(part)
}

/// Whether a mount can hold a part, rule for rule.
///
/// Reference: `vehicle::can_mount` (`vehicle.cpp:1267-1380`).
///
/// Ported: the first part in an empty square must be **structural**; a part type does not stack with itself unless it transfers power; two parts cannot share a **location**; only a tow cable
/// may join a square whose first part is an **animal control** or a **protrusion**; two **cargo** parts cannot share a square; a **mirror** cannot be mounted on an opaque part and an opaque part
/// cannot cover a mirror; and a part must stand on or beside existing structure. **Not ported, and named in `docs/parity.md`**: the turret-mount rule (the dump has one turret mount id, and the
/// same-id rule already covers it), engine conflicts (`has_engine_conflict`), and the flags that require another flag (`json_flag::get(...).requires_flag()`), which need the flag table the port
/// does not load.
fn can_mount(
    world: &World,
    vehicle: Entity,
    spec: &cdda_content::vehicles::PartSpec,
    x: i32,
    y: i32,
) -> bool {
    let Some(built) = world.get::<Vehicle>(vehicle) else {
        return false;
    };
    let occupants = parts_at(world, vehicle, x, y);
    if occupants.is_empty() && spec.location != "structure" {
        return false;
    }
    // Only a tow cable can join a square whose first part is an animal control or a protrusion.
    if let Some(first) = occupants.first() {
        let Some(first_part) = built.parts.get(*first) else {
            return false;
        };
        let Some(first_spec) = spec_of(world, &first_part.part) else {
            return false;
        };
        if (first_spec.flags.contains("ANIMAL_CTRL") || first_spec.flags.contains("PROTRUSION"))
            && !spec.flags.contains("TOW_CABLE")
        {
            return false;
        }
    }
    for index in &occupants {
        let Some(part) = built.parts.get(*index) else {
            return false;
        };
        let Some(other) = spec_of(world, &part.part) else {
            return false;
        };
        if other.id == spec.id && !spec.flags.contains("POWER_TRANSFER") {
            return false;
        }
        if !spec.location.is_empty() && spec.location == other.location {
            return false;
        }
        if spec.flags.contains("CARGO") && other.flags.contains("CARGO") {
            return false;
        }
        if spec.flags.contains("VISION")
            && !spec.flags.contains("CAMERA")
            && other.flags.contains("OPAQUE")
        {
            return false;
        }
        if spec.flags.contains("OPAQUE")
            && other.flags.contains("VISION")
            && !other.flags.contains("CAMERA")
        {
            return false;
        }
    }
    built.parts.is_empty() || stands_on_structure(world, vehicle, x, y)
}

/// Whether a structural part stands on or beside one square - the reference's `has_structural_part` over the five squares, with the `PROTRUSION` exception.
fn stands_on_structure(world: &World, vehicle: Entity, x: i32, y: i32) -> bool {
    let Some(built) = world.get::<Vehicle>(vehicle) else {
        return false;
    };
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)] {
        for index in parts_at(world, vehicle, x + dx, y + dy) {
            let Some(part) = built.parts.get(index) else {
                continue;
            };
            if spec_of(world, &part.part)
                .is_some_and(|other| {
                    other.location == "structure" && !other.flags.contains("PROTRUSION")
                })
            {
                return true;
            }
        }
    }
    false
}

/// Installs a part on one of a vehicle's squares, answering its index among the parts.
///
/// Reference: `vehicle::install_part` (`vehicle.cpp:1696-1757`): it refuses a mount that `can_mount` rejects, appends the part with its spec's durability, and lets `refresh()` rebuild the
/// derived lists the part changes. The port has no derived part lists to rebuild and no fake parts to remove, so it appends and answers the index; the square joins the one index at once.
///
/// Returns nothing when the part is not in the store, the vehicle does not exist, or the mount is refused.
pub fn install_part(world: &mut World, vehicle: Entity, part: &str, x: i32, y: i32) -> Option<usize> {
    let spec = spec_of(world, part)?.clone();
    let at = *world.get::<Position>(vehicle)?;
    if !can_mount(world, vehicle, &spec, x, y) {
        return None;
    }
    let hp = spec.durability;
    let index = {
        let mut built = world.get_mut::<Vehicle>(vehicle)?;
        built.parts.push(VehiclePart {
            part: part.to_owned(),
            x,
            y,
            hp,
            broken: hp <= 0,
        });
        built.parts.len() - 1
    };
    // The new part's square is part of the vehicle now; the origin is already listed by construction.
    let square = (at.x + x, at.y + y, at.z);
    let mut positions = crate::singletons::index_mut(world);
    if !positions.at_level(square.0, square.1, square.2).contains(&vehicle) {
        positions.insert(vehicle, square);
    }
    Some(index)
}

/// Removes one part by its index, answering the part that came out.
///
/// Reference: `vehicle::remove_part` (`vehicle.cpp:2213-2357`) erases the part and every part that depends on it, and unboards what stood on it. The port removes exactly the part it is given:
/// the dependent-part cascade (`SEAT`/`SEATBELT`, `WINDOW`/`CURTAIN`), unboarding, tool drops and the `removed`-flag cleanup pass are named in `docs/parity.md`, not guessed at.
///
/// A square leaves the index when no other part stands there. A vehicle's **origin** always stays, because that is where its [`Position`] is.
pub fn remove_part(world: &mut World, vehicle: Entity, index: usize) -> Option<VehiclePart> {
    let at = *world.get::<Position>(vehicle)?;
    let removed = {
        let mut built = world.get_mut::<Vehicle>(vehicle)?;
        if index >= built.parts.len() {
            return None;
        }
        built.parts.remove(index)
    };
    let origin = (at.x, at.y, at.z);
    let square = (at.x + removed.x, at.y + removed.y, at.z);
    if square != origin {
        let still_held = world.get::<Vehicle>(vehicle).is_some_and(|built| {
            built
                .parts
                .iter()
                .any(|part| part.x == removed.x && part.y == removed.y)
        });
        if !still_held {
            crate::singletons::index_mut(world).remove(vehicle, square);
        }
    }
    Some(removed)
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
