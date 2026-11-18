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
#[allow(clippy::struct_excessive_bools)]
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
    /// Whether the part is open, for a part that can open - the reference's `vehicle_part::open` (`vehicle.h`), the state a door, curtain or dome carries.
    pub open: bool,
    /// What the part holds, when it holds fuel: the liquid's item id for a tank, `battery` for a battery, empty for a part that holds nothing. The reference keeps this inside
    /// the part's base item (`vehicle_part::ammo_current`, `vehicle_part.cpp:253-267`); two fields carry the same meaning without an item in the component.
    pub fuel: String,
    /// How much of it, in charges - the reference's `vehicle_part::ammo_remaining` (`vehicle_part.cpp:307-317`).
    pub charges: i32,
    /// Whether the part is switched on - the reference's `vehicle_part::enabled`, the switch `start_engine` flips for an engine and every other part ignores.
    ///
    /// **A fresh part is on**: `bool enabled = true` (`vehicle.h:519`), which is why the reference's `total_power( fueled = false )` counts the engines of a
    /// vehicle straight out of its blueprint.
    pub enabled: bool,
    /// Whether the air scan counts this part as inside the cabin - the reference's `vehicle_part::inside` (`vehicle.h`), which `refresh_insides`
    /// (`vehicle.cpp:7525-7575`) maintains and `coeff_air_drag` reads.
    ///
    /// **A fresh part is false**, and it stays false until something calls `refresh_insides`: the reference's own `refresh` only marks the flag dirty
    /// (`vehicle.cpp:6899`) and recomputes it on demand from `enclosed_at` (`vehicle.cpp:7516`). So a vehicle straight out of the blueprint has every part
    /// outside, which is exactly the state the air-drag recording was taken in. The rule that fills it in is not ported yet.
    pub inside: bool,
    /// The fuel this engine last chose, when it can burn more than one - the reference's `vehicle_part::ammo_pref` (`vehicle_part.cpp:232-238`). Empty means "the type the
    /// spec names", which is exactly what the reference's `fuel_current` falls back to.
    pub fuel_preference: String,
}

/// A creature's place in a vehicle: which vehicle, and which of its parts the creature sits on.
///
/// Reference: the reference keeps this fact twice - `Character::in_vehicle` and the part's `vp_flag::passenger_flag`/`passenger_id` (`map::board_vehicle`, `map.cpp:1385-1418`) - and asks the
/// part's flag to find the occupant. The port keeps it once, on the creature, and derives "who sits here" with [`rider_at`], so there is no copy to drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Riding {
    /// The vehicle the creature is in.
    pub vehicle: Entity,
    /// The index of the part it sits on.
    pub part: usize,
}

/// A vehicle: the parts it is made of, in its blueprint's order, and the direction it faces.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct Vehicle {
    /// Its parts.
    pub parts: Vec<VehiclePart>,
    /// The direction it faces, in degrees clockwise from east - the reference's `turn_dir`. A placed vehicle faces 0 unless its placement names another direction.
    pub rotation: f64,
    /// How fast it is going, in hundredths of a mile per hour, negative for reverse - the reference's `velocity` (`vehicle.h`), which is a plain `int` member and
    /// the state every movement rule reads. A vehicle placed from a blueprint stands still.
    pub velocity: i32,
    /// The speed the driver has asked for, in the same units - the reference's `cruise_velocity` (`vehicle.h`). A vehicle nobody has asked to move stands at zero,
    /// which is what its constructor sets and what `thrust` reads as "hold still".
    pub cruise_velocity: i32,
    /// How much of a turn's movement the vehicle has left - the reference's `of_turn` (`vehicle.h`), a float that a move spends one square at a time. A vehicle
    /// placed from a blueprint has none left, which is why `gain_moves` refills it at the start of every turn.
    pub of_turn: f32,
    /// What was left over when a turn ended mid-square - the reference's `of_turn_carry` (`vehicle.h`). `gain_moves` starts the next turn with `1 + carry`, so a
    /// slow vehicle does not lose the fraction it could not spend.
    pub of_turn_carry: f32,
    /// The turn a collision has lined up for the **next** move, in degrees - the reference's `coll_turn`, which `part_smash` sets and `move_vehicle` spends when the
    /// vehicle can move again (`map.cpp:938-941`). A vehicle that is stopped clears it, because a stopped vehicle cannot be mid-skid.
    pub coll_turn: i32,
    /// The direction of the hull's **last accepted move**, in degrees clockwise from east - the reference's `move` (`vehicle.h`), which is set to the direction of travel on
    /// every accepted step (`map.cpp:942`) and is what makes a skid measurable: the angle between where a vehicle is going and where it points is `move` against `face`.
    pub move_dir: i32,
    /// How much steering is still owed to the hull, in degrees - the reference's `last_turn` (`vehicle.h`). A steering press sets it, every accepted step walks it back a
    /// degree (`map.cpp:917-928`), and `stop` clears it (`vehicle_move.cpp:693`).
    pub last_turn: i32,
    /// Whether the vehicle is sliding rather than steering - the reference's `skidding` (`vehicle.h`). While it is set the hull's facing follows `turn_dir` instead of
    /// the direction of travel (`map.cpp:934-937`), which is the whole difference a skid makes.
    pub skidding: bool,
}

/// A hull's rotation as a whole heading, which is the units the reference keeps `move` and `turn_dir` in.
///
/// Rotations are multiples of fifteen degrees once they come from the wheels, so this is a round rather than a truncation - and it is one function so the places that compare a
/// rotation with a heading agree about the conversion.
#[must_use]
pub fn floor_to_heading(rotation: f64) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let heading = rotation.round() as i32;
    heading.rem_euclid(360)
}

/// Builds a vehicle facing east, standing at a square.
///
/// The reference's own default: a mapgen entry that names no rotation places the vehicle at 0 degrees (`mapgen.cpp:3316-3320`).
pub fn spawn(world: &mut World, prototype: &str, at: (i32, i32, i32)) -> Option<Entity> {
    spawn_facing(world, prototype, at, 0.0)
}

/// Builds a vehicle from the world's prototype, standing at a square and facing a direction.
///
/// Reference: `map::add_vehicle` (`mapgen.cpp:7523-7560`) creates the vehicle, sets its pivot to the origin (`:7543-7550`), gives it `turn_dir` and calls
/// `precalc_mounts(0, dir, (0,0))`. `vehicle::init_state` fills it from the prototype's `parts`, and a part's hit points start at its `vpart_info` durability. The port refuses when
/// the prototype or one of its parts is not in the store, rather than building a vehicle with holes in it.
pub fn spawn_facing(
    world: &mut World,
    prototype: &str,
    at: (i32, i32, i32),
    rotation: f64,
) -> Option<Entity> {
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
            inside: false,
            open: false,
            fuel: String::new(),
            charges: 0,
            enabled: true,
            fuel_preference: String::new(),
        });
    }
    let entity = world
        .spawn((
            Vehicle {
                parts,
                cruise_velocity: 0,
                of_turn: 0.0,
                of_turn_carry: 0.0,
                coll_turn: 0,
                skidding: false,
                last_turn: 0,
                move_dir: floor_to_heading(rotation),
                rotation,
                velocity: 0,
            },
            crate::Position {
                x: at.0,
                y: at.1,
                z: at.2,
            },
        ))
        .id();
    // **The wheels start where the hull points** - the reference's `add_vehicle` gives the vehicle a `turn_dir` beside its facing (`mapgen.cpp:7543-7550`), and a
    // vehicle's step is planned from the wheels, so a hull with no wheel direction would travel north whatever way it was placed.
    world
        .entity_mut(entity)
        .insert(crate::vehicle_move::Wheels {
            turn_dir: floor_to_heading(rotation),
        });
    // A vehicle stands on every square its parts occupy, so the index holds it there as it is built; a rebuild derives the same entries from the same rule.
    let squares = occupied_squares(world, entity);
    let mut index = crate::singletons::index_mut(world);
    for square in squares {
        index.insert(entity, square);
    }
    Some(entity)
}

/// What a one-square move would hit, as far as the port can see it - the reference's `veh_collision` (`vehicle.h:143-156`) filled by `vehicle::collision` in **detect-only**
/// mode (`vehicle_move.cpp:702-826`).
///
/// The reference projects every part forward by the displacement (`vehicle::part_project_points`, `vehicle.cpp:8205-8224`), walks the parts **in their own order**, and skips
/// any part that is not real, not `structure` and not a rotor. The first part whose projected square holds something answers the collision. A collision with a vehicle is
/// returned before any speed is considered at all (`vehicle_move.cpp:838-846`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collision {
    /// Another vehicle stands where the part would go - `veh_coll_veh`.
    Vehicle {
        /// The moving vehicle's part that would hit it.
        part: usize,
        /// The vehicle standing there.
        target: Entity,
        /// The target's part on that square, which is what the reference's `vpart_position` answers: the map's square cache is **filled part by part and the last one wins**
        /// (`map::add_vehicle_to_cache`, `map.cpp:471-491`), so a square holding a frame, a wheel and a basket answers with the basket.
        target_part: usize,
    },
    /// Terrain stands where the part would go.
    ///
    /// Two reference kinds share this: `veh_coll_bashable` (`bashable: true`) and `veh_coll_other` (`bashable: false`).
    Terrain {
        /// The moving vehicle's part that would hit it.
        part: usize,
        /// The square it would hit.
        square: (i32, i32, i32),
        /// Whether the reference calls it bashable. Non-bashable terrain that still collides is the reference's `veh_coll_other`.
        bashable: bool,
    },
    /// A creature stands where the part would go - `veh_coll_body`.
    ///
    /// The reference does **not** count a creature riding the vehicle that is moving: on your own vehicle, a critter on a `BOARDABLE` part is a passenger, not an obstacle
    /// (`vehicle_move.cpp:867-872`), and a character in a vehicle is not a critter at all (`vehicle_move.cpp:832-836`).
    Creature {
        /// The moving vehicle's part that would hit it.
        part: usize,
        /// The creature standing there.
        target: Entity,
    },
}

/// The first thing a one-square move would hit, or `None` for a clear square - the reference's `vehicle::collision` answering in detect-only mode.
///
/// **Named, not guessed:** the reference also detects impassable fields, bashable terrain and impassable terrain, with a long list of terrain flags deciding the last two
/// (`vehicle_move.cpp:922-967`), and it swaps the hitting part for an `ARMOR` part first (`vehicle_move.cpp:922-929`). The port detects vehicles and creatures until the
/// terrain branches have a recording to be measured against.
#[must_use]
pub fn first_collision(world: &World, vehicle: Entity, delta: (i32, i32)) -> Option<Collision> {
    if delta == (0, 0) {
        return None;
    }
    let squares = part_squares(world, vehicle);
    let built = world.get::<Vehicle>(vehicle)?;
    for (index, part) in built.parts.iter().enumerate() {
        let Some(spec) = spec_of(world, &part.part) else {
            continue;
        };
        if spec.location != "structure" && !spec.flags.contains("ROTOR") {
            continue;
        }
        let Some(square) = squares.get(index).copied() else {
            continue;
        };
        let destination = (square.0 + delta.0, square.1 + delta.1, square.2);
        // A vehicle is answered first: the reference returns a vehicle collision before it looks for a creature at all (`vehicle_move.cpp:838-846`). A square holding the moving
        // vehicle itself is not a vehicle collision (`&ovp->vehicle() != this`), and the reference still asks about a creature standing there.
        if let Some(target) = at(world, destination)
            && target != vehicle
        {
            // The **last** part on the square, because the reference fills its one square cache part by part and the last part written answers `veh_at` (`map.cpp:479-485`).
            let target_part = part_squares(world, target)
                .iter()
                .rposition(|standing| *standing == destination)?;
            return Some(Collision::Vehicle {
                part: index,
                target,
                target_part,
            });
        }
        // Terrain comes after creatures, and its two kinds are tried in the reference's order: bashable first, impassable second (`vehicle_move.cpp:950-970`).
        if let Some(square_data) = crate::mapsquare::at(world, destination) {
            if terrain_collision(world, &square_data, built, part) {
                return Some(Collision::Terrain {
                    part: blamed_part(world, built, index),
                    square: destination,
                    bashable: true,
                });
            }
            if square_data.impassable() {
                return Some(Collision::Terrain {
                    part: blamed_part(world, built, index),
                    square: destination,
                    bashable: false,
                });
            }
        }
        if let Some(target) = creature_at(world, destination) {
            let riding_this = world
                .get::<Riding>(target)
                .is_some_and(|riding| riding.vehicle == vehicle);
            if riding_this {
                continue;
            }
            return Some(Collision::Creature {
                part: blamed_part(world, built, index),
                target,
            });
        }
    }
    None
}

/// The part the reference blames for a hit: the part itself when it is armour, else the first **unbroken** armour part on the same mount.
///
/// Reference: `vehicle::part_with_feature( part, VPFLAG_ARMOR, /* unbroken */ true )` (`vehicle.cpp:3030-3048`), called from `part_collision` before it works
/// out what kind of hit this is (`vehicle_move.cpp:922-929`) - "damage armor before damaging any other parts". The vehicle-vehicle branch returns before that
/// call, so a collision with another vehicle names the part that reached it, not the plate beside it.
fn blamed_part(world: &World, vehicle: &Vehicle, part: usize) -> usize {
    let Some(colliding) = vehicle.parts.get(part) else {
        return part;
    };
    let is_armour = |candidate: &VehiclePart| {
        !candidate.broken
            && spec_of(world, &candidate.part).is_some_and(|spec| spec.flags.contains("ARMOR"))
    };
    if is_armour(colliding) {
        return part;
    }
    for (index, other) in vehicle.parts.iter().enumerate() {
        if other.x == colliding.x && other.y == colliding.y && is_armour(other) {
            return index;
        }
    }
    part
}

/// Whether a part would hit the terrain on a square as **bashable** - the reference's long condition at `vehicle_move.cpp:950-967`.
///
/// Every clause is the reference's own, and each one exists because a vehicle should not stop for something trivial:
///
/// - the square must be bashable at all, and **not a plain floor** (`move_cost != 2`);
/// - a `TINY` thing (a flower) is only hit when the part's own mount carries a wheel;
/// - a `SHORT` thing (a low table) is not hit by a `PROTRUSION` (a ram or a spike);
/// - `NOCOLLIDE` things never interact with vehicles;
/// - `RAIL` tiles are skipped by a vehicle that can use rails - **the port cannot answer that yet** (`can_use_rails` needs the rail-riding parts), so the port collides
///   with them and names the gap rather than guessing.
fn terrain_collision(
    world: &World,
    data: &crate::mapsquare::SquareData,
    vehicle: &Vehicle,
    part: &VehiclePart,
) -> bool {
    let on_mount = |flag: &str| {
        vehicle.parts.iter().any(|other| {
            other.x == part.x
                && other.y == part.y
                && spec_of(world, &other.part).is_some_and(|spec| spec.flags.contains(flag))
        })
    };
    data.bashable()
        && data.move_cost() != 2
        && (on_mount("WHEEL") || !data.has_flag("TINY"))
        && !(on_mount("PROTRUSION") && data.has_flag("SHORT"))
        && !data.has_flag("NOCOLLIDE")
}

/// The creature standing on one square, if one does - the reference's `creature_tracker::creature_at` (`map.cpp:1340-1356` asks the map's cache; the port asks its one index and
/// keeps the entries that are creatures).
fn creature_at(world: &World, square: (i32, i32, i32)) -> Option<Entity> {
    let index = crate::singletons::index(world)?;
    index
        .at_level(square.0, square.1, square.2)
        .iter()
        .copied()
        .find(|entity| world.get::<crate::creature::Creature>(*entity).is_some())
}

/// Moves a vehicle one square, taking everything on it - the displacement half of `map::move_vehicle` (`map.cpp:773-...)`, which reaches
/// `map::displace_vehicle` (`map.cpp:1460-...)`.
///
/// The reference refuses an **empty** displacement and one longer than a square before anything else happens (`map.cpp:775-782`), and the port answers `false` for both without
/// touching the vehicle. A real move shifts the vehicle's origin, drops its old squares from the one index and takes the new ones, and puts every rider on the new square of the part
/// it sits on (`psg->setpos( psgp )`, `map.cpp:1559-1590`) - a passenger left behind would be a second source of truth about where the vehicle is.
///
/// **Named, not guessed:** the reference's displacement also handles ramps, stops at the map's edge, moves pets, remote control, and falling or floating. The port's squares are
/// unbounded and it has no ramps, no pets and no falling yet; collision, velocity and the damage a crash does belong to the movement rounds that follow, not to the plain shift.
pub fn move_vehicle(world: &mut World, vehicle: Entity, delta: (i32, i32)) -> bool {
    if delta == (0, 0) || delta.0.abs() > 1 || delta.1.abs() > 1 {
        return false;
    }
    let before = occupied_squares(world, vehicle);
    {
        let Some(mut position) = world.get_mut::<Position>(vehicle) else {
            return false;
        };
        position.x += delta.0;
        position.y += delta.1;
    }
    let after = occupied_squares(world, vehicle);
    {
        let mut index = crate::singletons::index_mut(world);
        for square in before {
            index.remove(vehicle, square);
        }
        for square in &after {
            index.insert(vehicle, *square);
        }
    }
    let squares = part_squares(world, vehicle);
    let riders: Vec<(Entity, usize)> = {
        let Some(mut query) = world.try_query::<(Entity, &Riding)>() else {
            return true;
        };
        query
            .iter(world)
            .filter(|(_, riding)| riding.vehicle == vehicle)
            .map(|(creature, riding)| (creature, riding.part))
            .collect()
    };
    for (rider, part) in riders {
        let Some(square) = squares.get(part).copied() else {
            continue;
        };
        let from = world
            .get::<Position>(rider)
            .map(|position| (position.x, position.y, position.z));
        {
            let Some(mut position) = world.get_mut::<Position>(rider) else {
                continue;
            };
            position.x = square.0;
            position.y = square.1;
            position.z = square.2;
        }
        if let Some(from) = from {
            let mut index = crate::singletons::index_mut(world);
            index.remove(rider, from);
            index.insert(rider, square);
        }
    }
    true
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

/// The square each part stands on, in the vehicle's own part order.
///
/// Reference: a part's square is `pos_abs() + precalc[0]` (`vehicle.cpp:3772-3775`), and `precalc[0]` is `coord_translate` of its mount at the vehicle's facing
/// (`vehicle.cpp:3643-3660`), with the pivot at the origin for a placed vehicle (`mapgen.cpp:7543-7550`).
#[must_use]
pub fn part_squares(world: &World, vehicle: Entity) -> Vec<(i32, i32, i32)> {
    let Some(at) = world.get::<Position>(vehicle) else {
        return Vec::new();
    };
    let Some(built) = world.get::<Vehicle>(vehicle) else {
        return Vec::new();
    };
    built
        .parts
        .iter()
        .map(|part| {
            let (dx, dy) =
                crate::rotation::coord_translate(built.rotation, (0, 0), (part.x, part.y));
            (at.x + dx, at.y + dy, at.z)
        })
        .collect()
}

/// The squares a vehicle stands on, sorted and without repeats.
///
/// Reference: `vehicle::get_points` (`vehicle.cpp:8186-8204`) walks the parts by **mount** and takes one `abs_part_pos` per distinct mount.
#[must_use]
pub fn occupied_squares(world: &World, vehicle: Entity) -> Vec<(i32, i32, i32)> {
    part_squares(world, vehicle)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The vehicle standing on one square, if one does.
///
/// Reference: `map::veh_at` (`map.cpp:1340-1356`) never scans the level's vehicles; it asks the level's vehicle cache, and the port's cache is the one [`crate::PositionIndex`].
/// The reference also answers **which part**, because its cache stores one; the port's index stores the vehicle, and a part's square is derived from the vehicle's own mount and
/// facing through [`part_squares`].
#[must_use]
pub fn at(world: &World, square: (i32, i32, i32)) -> Option<Entity> {
    let index = crate::singletons::index(world)?;
    index
        .at_level(square.0, square.1, square.2)
        .iter()
        .copied()
        .find(|entity| world.get::<Vehicle>(*entity).is_some())
}

/// What a vehicle's square costs a walker to move through - the reference's `vpart_position::get_movecost` (`vehicle.cpp:2871-2881`).
///
/// **0** when an unbroken part on the square is an `OBSTACLE`; **2** when an unbroken part is an `AISLE`; **8** for an open vehicle square. The obstacle is asked first and over every part
/// on the square, which is the reference's `obstacle_at_part` (`:2857-2869`): a windshield blocks even beside a seat. An **open door** stops being an obstacle (`vehicle_part::open`,
/// `vehicle.h:519`, which a fresh part starts false), and only the first obstacle part is asked, as the reference asks only it.
///
/// Answers `None` when no vehicle stands on the square - the caller then falls back to the terrain's own cost, which is the reference's `map::move_cost` order.
#[must_use]
pub fn move_cost_at(world: &World, square: (i32, i32, i32)) -> Option<i32> {
    let vehicle = at(world, square)?;
    let built = world.get::<Vehicle>(vehicle)?;
    let squares = part_squares(world, vehicle);
    let mut obstacle = None;
    let mut aisle = false;
    for (index, part) in built.parts.iter().enumerate() {
        if squares.get(index).copied() != Some(square) || part.broken {
            continue;
        }
        let Some(spec) = spec_of(world, &part.part) else {
            continue;
        };
        if spec.flags.contains("OBSTACLE") {
            // The reference asks only the **first** obstacle part, and lets it stop blocking when it is an openable part left open (`vehicle.cpp:2859-2866`).
            if obstacle.is_none() {
                obstacle = Some(!(spec.flags.contains("OPENABLE") && part.open));
            }
            continue;
        }
        if spec.flags.contains("AISLE") {
            aisle = true;
        }
    }
    if obstacle == Some(true) {
        return Some(0);
    }
    Some(if aisle { 2 } else { 8 })
}

/// Puts a creature on the boardable part it stands on, unboarding whoever was already there.
///
/// Reference: `map::board_vehicle` (`map.cpp:1385-1418`). The part is the **first unbroken `BOARDABLE` part** on the creature's square - the reference's
/// `part_with_feature( VPFLAG_BOARDABLE, true )` - and a square with none refuses the boarding, as does a broken seat. A part holds one creature: the reference unboards the occupant
/// before marking the seat, and so does the port.
///
/// The reference finds the vehicle itself through the square cache; the port takes the vehicle the caller means and still refuses if the creature does not stand on one of its squares,
/// so naming the wrong vehicle cannot seat anyone.
#[must_use]
pub fn board(world: &mut World, vehicle: Entity, creature: Entity) -> Option<usize> {
    let position = *world.get::<Position>(creature)?;
    let square = (position.x, position.y, position.z);
    let seat = {
        let built = world.get::<Vehicle>(vehicle)?;
        let squares = part_squares(world, vehicle);
        let mut found = None;
        for (index, part) in built.parts.iter().enumerate() {
            if part.broken || squares.get(index).copied() != Some(square) {
                continue;
            }
            if spec_of(world, &part.part).is_some_and(|spec| spec.flags.contains("BOARDABLE")) {
                found = Some(index);
                break;
            }
        }
        found?
    };
    if let Some(occupant) = rider_at(world, vehicle, seat)
        && occupant != creature
    {
        unboard(world, occupant);
    }
    world.entity_mut(creature).insert(Riding {
        vehicle,
        part: seat,
    });
    Some(seat)
}

/// **A step taken from a vehicle**: the rider climbs out, steps, and climbs back in if the square it lands on has a seat.
///
/// Reference: `avatar_action::move` (`avatar_action.cpp:653-672`) unboards the character **before** the move and boards it again afterwards, only if the destination holds a
/// boardable part. That single order is what makes walking seat to seat stay "in the vehicle" while walking off the hull leaves the character standing on the ground - and it
/// carries the driver rule with it, because unboarding the driver sets the hull skidding (`map::unboard_vehicle`, `map.cpp:1432-1440`).
///
/// The cost is the caller's, as with [`crate::creature::try_step`]: what a square costs to enter is the map's answer, and the vehicle only decides who is sitting in it.
pub fn step_from_vehicle(
    world: &mut World,
    creature: Entity,
    to: (i32, i32, i32),
    cost: i32,
) -> crate::creature::StepOutcome {
    if world.get::<Riding>(creature).is_some() {
        unboard(world, creature);
    }
    let outcome = crate::creature::try_step(world, creature, to, cost);
    if outcome == crate::creature::StepOutcome::Moved
        && let Some(vehicle) = at(world, to)
    {
        // **A seat that refuses the rider is not an error**: the reference boards only when the part says boardable, and a square the vehicle merely covers leaves the character
        // standing on it, which is exactly what `veh_at(p).part_with_feature(BOARDABLE)` answering nullopt means.
        let _ = board(world, vehicle, creature);
    }
    outcome
}

/// Takes a creature out of its seat; answers whether it was in one.
///
/// Reference: `map::unboard_vehicle` (`map.cpp:1420-1440`). The reference also clears `controlling_vehicle` and stops autodriving; the port has no driving yet, and names that in
/// `docs/parity.md` rather than pretending the rest is the whole rule.
pub fn unboard(world: &mut World, creature: Entity) -> bool {
    let Some(riding) = world.get::<Riding>(creature).copied() else {
        return false;
    };
    // **A driver who climbs out leaves the machine sliding.** Reference: `map::unboard_vehicle` (`map.cpp:1432-1440`): when the character leaving *was* controlling the vehicle, it
    // stops autodriving and sets `skidding = true` - a vehicle with nobody at the wheel does not politely pull over. A passenger's exit changes nothing about the hull.
    //
    // The port has no autodrive target to clear yet, so the skid is the whole of the rule here; and it asks who was in the **controls** seat rather than a `controlling_vehicle`
    // flag, because sitting in that seat is what being in control means in this store.
    let was_driver = driver_of(world, riding.vehicle) == Some(creature);
    let had_one = world.entity_mut(creature).take::<Riding>().is_some();
    if was_driver && let Some(mut built) = world.get_mut::<Vehicle>(riding.vehicle) {
        built.skidding = true;
    }
    had_one
}

/// Every unbroken part that is a wheel, by index - the reference's `wheelcache` (`vehicle.cpp:6738-6740`).
#[must_use]
pub fn wheels(world: &World, vehicle: Entity) -> Vec<usize> {
    let count = world
        .get::<Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    (0..count)
        .filter(|index| {
            world
                .get::<Vehicle>(vehicle)
                .and_then(|built| built.parts.get(*index))
                .is_some_and(|part| !part.broken)
                && part_spec(world, vehicle, *index)
                    .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "WHEEL"))
        })
        .collect()
}

/// Whether the vehicle has enough wheels to be moved at all.
///
/// Reference: `vehicle::sufficient_wheel_config` (`vehicle.cpp:4984-4997`): no wheels is no; **one** wheel is only enough when it is `STABLE` and the hull is a three-tile
/// vehicle at most; anything more is enough. The reference counts the structure parts for that second test, which is the `structure` location on the part's spec.
#[must_use]
pub fn sufficient_wheel_config(world: &World, vehicle: Entity) -> bool {
    let wheels = wheels(world, vehicle);
    if wheels.is_empty() {
        return false;
    }
    if wheels.len() != 1 {
        return true;
    }
    let Some(only) = wheels.first().copied() else {
        return false;
    };
    let stable = part_spec(world, vehicle, only)
        .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "STABLE"));
    let count = world
        .get::<Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    let structure = (0..count)
        .filter(|index| {
            part_spec(world, vehicle, *index).is_some_and(|spec| spec.location == "structure")
        })
        .count();
    stable && structure <= 3
}

/// Whether the weight of the vehicle sits over its wheels.
///
/// Reference: `vehicle::balanced_wheel_config` (`vehicle.cpp:5104-5121`): the bounding box of the wheel mounts must contain the centre of mass. A vehicle with no wheels has no box
/// to contain anything, which the reference's own `point::max`/`point::min` sentinels make false; here it is answered directly.
#[must_use]
pub fn balanced_wheel_config(world: &World, vehicle: Entity) -> bool {
    let wheels = wheels(world, vehicle);
    let Some(first) = wheels.first().copied() else {
        return false;
    };
    let Some(entry) = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(first))
    else {
        return false;
    };
    let (mut min_x, mut max_x) = (entry.x, entry.x);
    let (mut min_y, mut max_y) = (entry.y, entry.y);
    for index in wheels {
        let Some(part) = world
            .get::<Vehicle>(vehicle)
            .and_then(|built| built.parts.get(index))
        else {
            continue;
        };
        min_x = min_x.min(part.x);
        max_x = max_x.max(part.x);
        min_y = min_y.min(part.y);
        max_y = max_y.max(part.y);
    }
    let (com_x, com_y) = crate::vehicle_mass::center_of_mass(world, vehicle);
    (min_x..=max_x).contains(&com_x) && (min_y..=max_y).contains(&com_y)
}

/// Whether the vehicle's wheels can carry it at all - `vehicle::valid_wheel_config` (`vehicle.cpp:5123-5126`).
#[must_use]
pub fn valid_wheel_config(world: &World, vehicle: Entity) -> bool {
    sufficient_wheel_config(world, vehicle) && balanced_wheel_config(world, vehicle)
}

/// The creature sitting on one part of a vehicle, if one is.
#[must_use]
pub fn rider_at(world: &World, vehicle: Entity, part: usize) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<Riding>>()?;
    query.iter(world).find(|entity| {
        world
            .get::<Riding>(*entity)
            .is_some_and(|riding| riding.vehicle == vehicle && riding.part == part)
    })
}

/// Opens or closes one openable part, and answers whether it changed.
///
/// Reference: `vehicle::open` and `vehicle::close` (`vehicle_use.cpp:1215-1240`) set the part's `open` state through `vehicle::open_or_close` (`:1327-1370`), and a part that cannot
/// open is refused with a debug message rather than changed - which is the `false` this answers. The reference's `open_or_close` also unlocks a locked door, mirrors the state onto a
/// paired fake part, and propagates along a multisquare openable's lines; the port has no locks, fake parts or multisquare openables in its data, and names that in `docs/parity.md`.
/// The transparency and air caches the reference dirties are derived in the port, not stored.
///
/// **Opening a door from outside costs the rest of the turn.** Reference: `avatar_action::move` (`avatar_action.cpp:561-566`): stepping into a vehicle's closed door opens it and
/// then charges `you.mod_moves( -you.get_speed() )` - the opener's **whole speed for the turn**, not the price of a square, because a door is opened with a hand and a moment rather
/// than walked through. A part that cannot open is not a door: nothing happens and nothing is charged.
pub fn open_from_outside(world: &mut World, vehicle: Entity, index: usize, opener: Entity) -> bool {
    if !open_or_close(world, vehicle, index, true) {
        return false;
    }
    let speed = world
        .get::<crate::creature::Speed>(opener)
        .map_or(0, |speed| speed.base + speed.bonus);
    if let Some(mut moves) = world.get_mut::<crate::creature::Moves>(opener) {
        moves.left -= speed;
    }
    true
}

/// **Opens or closes one part**, and answers whether the state changed.
///
/// Reference: `vehicle::open` and `vehicle::close` (`vehicle_use.cpp:1215-1240`) set the part's `open` state through `vehicle::open_or_close` (`:1327-1370`); a part whose data does
/// not say `OPENABLE` is refused, which is the `false` this answers.
pub fn open_or_close(world: &mut World, vehicle: Entity, index: usize, opening: bool) -> bool {
    let openable = {
        let Some(built) = world.get::<Vehicle>(vehicle) else {
            return false;
        };
        let Some(part) = built.parts.get(index) else {
            return false;
        };
        spec_of(world, &part.part).is_some_and(|spec| spec.flags.contains("OPENABLE"))
    };
    if !openable {
        return false;
    }
    let Some(mut built) = world.get_mut::<Vehicle>(vehicle) else {
        return false;
    };
    let Some(part) = built.parts.get_mut(index) else {
        return false;
    };
    part.open = opening;
    true
}

/// The fuel an engine is currently set to burn - the reference's `vehicle_part::fuel_current` (`vehicle_part.cpp:232-238`): the fuel it last chose, or else the type its
/// spec names. Returns the empty string for a part that is not an engine.
#[must_use]
pub fn fuel_current(world: &World, vehicle: Entity, index: usize) -> String {
    let Some(part) = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
    else {
        return String::new();
    };
    if !part.fuel_preference.is_empty() {
        return part.fuel_preference.clone();
    }
    spec_of(world, &part.part).map_or_else(String::new, |spec| spec.fuel_type.clone())
}

/// How much fuel an engine can draw on right now - the reference's `vehicle::engine_fuel_left` (`vehicle.cpp:3924-3927`), which asks `fuel_left` for the fuel the engine is
/// currently set to burn.
#[must_use]
pub fn engine_fuel_left(world: &World, vehicle: Entity, index: usize) -> i64 {
    fuel_left(world, vehicle, &fuel_current(world, vehicle, index))
}

/// Whether an engine is switched on - the reference's `vehicle::is_engine_on` (`vehicle.cpp:1061-1064`): available **and** enabled. A broken part is not available, so a broken
/// engine that was left switched on reads as off.
#[must_use]
pub fn is_engine_on(world: &World, vehicle: Entity, index: usize) -> bool {
    world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
        .is_some_and(|part| !part.broken && part.enabled)
}

/// Makes an engine burn a fuel it can use - the reference's `vehicle::auto_select_fuel` (`vehicle_use.cpp:627-643`).
///
/// Three steps, in the reference's order: a part that is not an available engine cannot select anything; an engine whose **spec fuel type is none** (`null`) or that already has
/// fuel of its current type needs no choice; otherwise the engine takes the **first** of its `fuel_opts` the vehicle actually has, and remembers it. The answer is whether the
/// engine can be started at all, which is why `start_engine` calls this rather than a separate check.
pub fn auto_select_fuel(world: &mut World, vehicle: Entity, index: usize) -> bool {
    let Some(part) = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
    else {
        return false;
    };
    let (broken, name) = (part.broken, part.part.clone());
    let Some(spec) = spec_of(world, &name) else {
        return false;
    };
    if broken || !spec.flags.contains("ENGINE") {
        return false;
    }
    if spec.fuel_type == "null" || engine_fuel_left(world, vehicle, index) > 0 {
        return true;
    }
    let Some(option) = spec
        .fuel_opts
        .iter()
        .find(|fuel| fuel_left(world, vehicle, fuel) > 0)
        .cloned()
    else {
        return false;
    };
    if let Some(mut built) = world.get_mut::<Vehicle>(vehicle)
        && let Some(part) = built.parts.get_mut(index)
    {
        part.fuel_preference = option;
        return true;
    }
    false
}

/// Turns an engine on or off - the reference's three-argument `vehicle::start_engine` (`vehicle.cpp:5412-5431`), the switch half of starting.
///
/// Turning **off** always succeeds. Turning **on** flips the switch, asks `auto_select_fuel` for fuel, and flips it back when there is none - so a failed start never leaves an
/// engine that looks on. Asking for the state it already has answers `false` and changes nothing.
///
/// **Named, not guessed:** the reference's single-argument `start_engine` (`vehicle_use.cpp:645-...`) then checks the driver's limbs and size, immobilisers, starter motors and
/// battery charge, fuel pumps, and rolls a chance of failure against engine damage, with sounds and backfires. The port has no driver, faults, battery discharge or start sounds yet,
/// so those checks are not here; `auto_select_fuel` is the one start condition that needs only the vehicle.
pub fn start_engine(world: &mut World, vehicle: Entity, index: usize, turn_on: bool) -> bool {
    let Some(part) = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
    else {
        return false;
    };
    if part.enabled == turn_on {
        return false;
    }
    if let Some(mut built) = world.get_mut::<Vehicle>(vehicle)
        && let Some(part) = built.parts.get_mut(index)
    {
        part.enabled = turn_on;
    }
    if !turn_on {
        return true;
    }
    if auto_select_fuel(world, vehicle, index) {
        return true;
    }
    if let Some(mut built) = world.get_mut::<Vehicle>(vehicle)
        && let Some(part) = built.parts.get_mut(index)
    {
        part.enabled = false;
    }
    false
}

/// How much fuel of a type the vehicle carries - the reference's `vehicle::fuel_left` (`vehicle.cpp:3861-3925`).
///
/// Two branches, as the reference has two: a **battery** is summed over the parts that are batteries, and every other fuel over the parts that can hold fuel and carry that
/// type. `fuel_left` asks for fuel stores, not working ones, so a broken tank still counts - which is the reference's `is_fuel_store( false )`.
///
/// **Named, not guessed:** the reference's battery branch adds every **connected** vehicle's batteries with a cable loss (`search_connected_vehicles`), skips a tank whose
/// liquid is frozen, and adds ten charges for a muscle engine with a ready driver or a perpetual fuel. The port has none of those inputs yet, so it sums the vehicle's own
/// stores: the same answer whenever nothing is connected, nothing is frozen and no muscle engine is fitted.
#[must_use]
pub fn fuel_left(world: &World, vehicle: Entity, fuel_type: &str) -> i64 {
    let Some(built) = world.get::<Vehicle>(vehicle) else {
        return 0;
    };
    let battery = fuel_type == "battery";
    built
        .parts
        .iter()
        .filter(|part| part.fuel == fuel_type)
        .filter(|part| {
            spec_of(world, &part.part).is_some_and(|spec| {
                if battery {
                    spec.battery
                } else {
                    spec.fuel_store
                }
            })
        })
        .map(|part| i64::from(part.charges))
        .sum()
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
            if spec_of(world, &part.part).is_some_and(|other| {
                other.location == "structure" && !other.flags.contains("PROTRUSION")
            }) {
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
pub fn install_part(
    world: &mut World,
    vehicle: Entity,
    part: &str,
    x: i32,
    y: i32,
) -> Option<usize> {
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
            inside: false,
            open: false,
            fuel: String::new(),
            charges: 0,
            enabled: true,
            fuel_preference: String::new(),
        });
        built.parts.len() - 1
    };
    // The new part's square is part of the vehicle now; the origin is already listed by construction.
    let rotation = world
        .get::<Vehicle>(vehicle)
        .map_or(0.0, |built| built.rotation);
    let (dx, dy) = crate::rotation::coord_translate(rotation, (0, 0), (x, y));
    let square = (at.x + dx, at.y + dy, at.z);
    let mut positions = crate::singletons::index_mut(world);
    if !positions
        .at_level(square.0, square.1, square.2)
        .contains(&vehicle)
    {
        positions.insert(vehicle, square);
    }
    Some(index)
}

/// Removes one part by its index, answering the part that came out.
///
/// Reference: `vehicle::remove_part` (`vehicle.cpp:2213-2357`). The part comes out, and so does the part that depends on it at the same mount: a `WINDOW` takes its `CURTAIN`, a
/// `SEAT` its `SEATBELT`, a `BATTERY_MOUNT` its `NEEDS_BATTERY_MOUNT`, a `HANDHELD_BATTERY_MOUNT` its `NEEDS_HANDHELD_BATTERY_MOUNT` (`:2258-2283`), and the removal recurses
/// because a dependent may itself have one. Unboarding what stood on the part, dropping its tools, releasing a caught animal and the `removed`-flag cleanup pass are named in
/// `docs/parity.md`, not guessed at.
///
/// A square leaves the index when no other part stands there. A vehicle's **origin** always stays, because that is where its [`Position`] is.
pub fn remove_part(world: &mut World, vehicle: Entity, index: usize) -> Option<VehiclePart> {
    let removed = remove_one_part(world, vehicle, index)?;
    let mount = (removed.x, removed.y);
    let parent_flags = spec_of(world, &removed.part)
        .map(|spec| spec.flags.clone())
        .unwrap_or_default();
    for (parent, child) in DEPENDENT_PAIRS {
        if !parent_flags.contains(parent) {
            continue;
        }
        // The reference removes one dependent per pair, and that removal recurses through the child's own dependents.
        if let Some(child_index) = first_part_at_with_flag(world, vehicle, mount, child) {
            let _ = remove_part(world, vehicle, child_index);
        }
    }
    Some(removed)
}

/// The pairs of flags where the first part cannot stay without the second - the reference's `remove_dependent_part` calls (`vehicle.cpp:2258-2283`).
const DEPENDENT_PAIRS: [(&str, &str); 4] = [
    ("WINDOW", "CURTAIN"),
    ("SEAT", "SEATBELT"),
    ("BATTERY_MOUNT", "NEEDS_BATTERY_MOUNT"),
    ("HANDHELD_BATTERY_MOUNT", "NEEDS_HANDHELD_BATTERY_MOUNT"),
];

/// The first part at one mount that carries a flag, broken or not - the reference's `part_with_feature( mount, flag, false )`.
fn first_part_at_with_flag(
    world: &World,
    vehicle: Entity,
    mount: (i32, i32),
    flag: &str,
) -> Option<usize> {
    parts_at(world, vehicle, mount.0, mount.1)
        .into_iter()
        .find(|index| {
            world
                .get::<Vehicle>(vehicle)
                .and_then(|built| built.parts.get(*index))
                .and_then(|part| spec_of(world, &part.part))
                .is_some_and(|spec| spec.flags.contains(flag))
        })
}

/// Erases one part by its index and keeps the index honest; the cascade lives in [`remove_part`].
fn remove_one_part(world: &mut World, vehicle: Entity, index: usize) -> Option<VehiclePart> {
    let at = *world.get::<Position>(vehicle)?;
    // The reference unboards whoever stands on a removed BOARDABLE part before the part is gone (`vehicle.cpp:2238-2241`).
    let boardable = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
        .and_then(|part| spec_of(world, &part.part))
        .is_some_and(|spec| spec.flags.contains("BOARDABLE"));
    if boardable && let Some(rider) = rider_at(world, vehicle, index) {
        unboard(world, rider);
    }
    let removed = {
        let mut built = world.get_mut::<Vehicle>(vehicle)?;
        if index >= built.parts.len() {
            return None;
        }
        built.parts.remove(index)
    };
    let origin = (at.x, at.y, at.z);
    let rotation = world
        .get::<Vehicle>(vehicle)
        .map_or(0.0, |built| built.rotation);
    let (dx, dy) = crate::rotation::coord_translate(rotation, (0, 0), (removed.x, removed.y));
    let square = (at.x + dx, at.y + dy, at.z);
    if square != origin {
        let still_held = world.get::<Vehicle>(vehicle).is_some_and(|built| {
            built.parts.iter().any(|part| {
                crate::rotation::coord_translate(built.rotation, (0, 0), (part.x, part.y))
                    == (dx, dy)
            })
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
    if let crate::vehicle_damage::Outcome::Applied {
        hp_after, broke, ..
    } = outcome
        && let Some(mut built) = world.get_mut::<Vehicle>(vehicle)
        && let Some(part) = built.parts.get_mut(index)
    {
        part.hp = hp_after;
        part.broken = part.broken || broke;
    }
    outcome
}

/// The spec of one part of one vehicle, copied out of the world's store - the reference's `vehicle_part::info()`.
#[must_use]
pub fn part_spec(
    world: &World,
    vehicle: Entity,
    index: usize,
) -> Option<cdda_content::vehicles::PartSpec> {
    let store = crate::singletons::holder::<TheVehicleStore>(world)?;
    let name = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
        .map(|part| part.part.clone())?;
    world.get::<VehicleStore>(store)?.parts.get(&name).cloned()
}

/// The fraction of a charge a vehicle could not burn, by fuel.
///
/// Reference: `vehicle::fuel_remainder` (`vehicle.h:520`), a map from fuel to energy that carries a turn's unburnt fraction into the next one - which is why a car does not
/// lose fuel to rounding every turn. Kept as a component on the vehicle rather than a field inside [`Vehicle`], because it is state about burning that most vehicles never
/// have any of, and a component an entity does not carry is absent rather than zero.
#[derive(Debug, Clone, Default, PartialEq, bevy_ecs::component::Component)]
pub struct FuelRemainder(pub std::collections::BTreeMap<String, f64>);

impl FuelRemainder {
    /// What is left over for a fuel, or nothing when the vehicle has never burned it.
    #[must_use]
    pub fn of(&self, fuel: &str) -> f64 {
        self.0.get(fuel).copied().unwrap_or(0.0)
    }
}

/// A vehicle's fuel leftovers, read from the store.
#[must_use]
pub fn fuel_remainder_of(world: &bevy_ecs::world::World, vehicle: Entity) -> FuelRemainder {
    world
        .get::<FuelRemainder>(vehicle)
        .cloned()
        .unwrap_or_default()
}

/// Writes a vehicle's fuel leftovers back into the store.
pub fn set_fuel_remainder(
    world: &mut bevy_ecs::world::World,
    vehicle: Entity,
    remainder: FuelRemainder,
) {
    world.entity_mut(vehicle).insert(remainder);
}

/// The creature steering a vehicle, if anyone is.
///
/// Reference: `veh_utils::player_in_control` / `vehicle::player_in_control` asks the square the creature stands on for a part with the `CONTROLS` flag - a seat is only the
/// driver's seat if its data says so, which is why the port asks the part's flags rather than its position. The first rider on such a part answers, and a creature riding
/// anything else is a passenger.
#[must_use]
pub fn driver_of(world: &World, vehicle: Entity) -> Option<Entity> {
    let mut query = world.try_query::<(Entity, &Riding)>()?;
    let riders: Vec<(Entity, Riding)> = query
        .iter(world)
        .filter(|(_, riding)| riding.vehicle == vehicle)
        .map(|(entity, riding)| (entity, *riding))
        .collect();
    drop(query);
    for (creature, riding) in riders {
        let Some(spec) = part_spec(world, vehicle, riding.part) else {
            continue;
        };
        if spec.flags.iter().any(|flag| flag == "CONTROLS") {
            return Some(creature);
        }
    }
    None
}

/// **The parts that can shoot**: a turret that is not broken - the reference's `vehicle::turrets()` (`turret.cpp:40-51`), which walks `turret_locations` and keeps
/// `!e.is_broken() && e.is_turret()`.
///
/// The port keeps no `turret_locations` cache: the list is derived from the parts, which is what the index does for the ground. Broken turrets are excluded here rather than at
/// the call site because every caller asks the same question.
#[must_use]
pub fn turrets(world: &World, vehicle: Entity) -> Vec<usize> {
    let Some(built) = world.get::<Vehicle>(vehicle) else {
        return Vec::new();
    };
    (0..built.parts.len())
        .filter(|index| {
            let Some(part) = built.parts.get(*index) else {
                return false;
            };
            !part.broken
                && spec_of(world, &part.part).is_some_and(|spec| spec.flags.contains("TURRET"))
        })
        .collect()
}

/// **Whether a turret draws its ammunition from the vehicle's tanks or batteries** - the reference's `turret_data::uses_vehicle_tanks_or_batteries` (`turret.cpp:92-97`), which is
/// `has_flag( "USE_TANKS" ) || has_flag( "USE_BATTERIES" )`.
///
/// Those flags are not written in the data for ordinary turret mounts: `vehicles::parts::finalize` (`veh_type.cpp:589-597`) copies a generic turret and sets `USE_TANKS` when the
/// mountable gun's ammunition is a liquid and `USE_BATTERIES` when it is a battery magazine. The recording of the parts therefore carries them, and four of the 240 turret mounts use
/// tanks.
#[must_use]
pub fn uses_vehicle_tanks_or_batteries(world: &World, vehicle: Entity, index: usize) -> bool {
    world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
        .and_then(|part| spec_of(world, &part.part))
        .is_some_and(|spec| {
            spec.flags.contains("USE_TANKS") || spec.flags.contains("USE_BATTERIES")
        })
}

/// **How much ammunition a turret has left** - the reference's `turret_data::ammo_remaining` (`turret.cpp:99-110`): a mount that feeds from the vehicle's tanks or batteries answers
/// the vehicle's `fuel_left` of the ammunition's own type; every other mount answers the ammunition in its base item.
///
/// The port's base item is the part's own charge count, because a turret's gun is installed with its ammunition rather than as a container of its own.
#[must_use]
pub fn turret_ammo_remaining(world: &World, vehicle: Entity, index: usize) -> i64 {
    let Some(part) = world
        .get::<Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
    else {
        return 0;
    };
    if uses_vehicle_tanks_or_batteries(world, vehicle, index) {
        return fuel_left(world, vehicle, &part.fuel);
    }
    i64::from(part.charges)
}

/// **A turret ready to be used**, or nothing: the reference's `vehicle::turret_query( vehicle_part & )` (`turret.cpp:53-60`) refuses a mount that is not a turret, has been removed,
/// or is broken.
#[must_use]
pub fn turret_query(world: &World, vehicle: Entity, index: usize) -> Option<usize> {
    let part = world.get::<Vehicle>(vehicle)?.parts.get(index)?;
    if part.broken || !spec_of(world, &part.part).is_some_and(|spec| spec.flags.contains("TURRET"))
    {
        return None;
    }
    Some(index)
}

/// **Takes fuel out of a vehicle's stores**, in part order, and answers how much came out - the reference's `vehicle::drain( map *, itype_id, amount, filter )`
/// (`vehicle.cpp:3959-3986`).
///
/// The reference walks `parts` in order, skips a part whose `ammo_current()` is not the fuel asked for, and takes `min( amount, part.ammo_remaining() )` from each until the amount is
/// met. The port walks the same list in the same order and takes the same share, so a drain that spans two tanks leaves the first empty and the second short - which is what a
/// flamethrower on a two-tank vehicle actually does.
///
/// **Named, not guessed:** the reference handles a **battery** through `discharge_battery`, which follows jumper cables into connected vehicles and applies their cable loss, and its
/// part-index overload refuses a part that is not a tank. The port has no connected-vehicle network, so a battery drains from its own vehicle like any other fuel, at zero loss; the
/// index overload has no port caller.
pub fn drain(world: &mut World, vehicle: Entity, fuel_type: &str, amount: i64) -> i64 {
    let mut drained = 0;
    let mut wanted = amount;
    let count = world
        .get::<Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    for index in 0..count {
        if wanted <= 0 {
            break;
        }
        let take = world
            .get::<Vehicle>(vehicle)
            .and_then(|built| built.parts.get(index))
            .filter(|part| part.fuel == fuel_type)
            .map_or(0, |part| i64::from(part.charges).min(wanted));
        if take <= 0 {
            continue;
        }
        let mut held = world.get_mut::<Vehicle>(vehicle);
        if let Some(part) = held.as_mut().and_then(|built| built.parts.get_mut(index)) {
            part.charges -= i32::try_from(take).unwrap_or(i32::MAX);
        }
        drained += take;
        wanted -= take;
    }
    drained
}

/// **What a turret can do right now**: the reference's `turret_data::status` (`turret.cpp:250-275`), in its own order - no ammunition first, then no power, then ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretStatus {
    /// There is no such mount, or it is not a turret, or it is broken - `status::invalid`.
    Invalid,
    /// The mount cannot feed a shot - `status::no_ammo`.
    NoAmmo,
    /// The mount has ammunition but the batteries cannot pay for the shot - `status::no_power`.
    NoPower,
    /// The mount can fire - `status::ready`.
    Ready,
}

/// **The ladder a turret passes before it fires** - the reference's `turret_data::query` (`turret.cpp:250-275`).
///
/// The order matters and the two ammunition branches are **not** the same test, which is why this is a rule rather than two predicates:
///
/// - a mount that feeds from the tanks asks `fuel_left( ammo ) < ammo_required` **unconditionally**, so a gun that spends nothing never runs dry;
/// - a mount with its own magazine asks `ammo_required && remaining < ammo_required`, and a gun that spends nothing is never short;
/// - only then does the energy question arise: `energy_drain × qty` against the battery's charges, one kilojoule each, so a gun whose shot costs more than the vehicle's batteries
///   hold is **no power** even with a full magazine.
pub fn turret_status(
    world: &World,
    vehicle: Entity,
    index: usize,
    ammo_required: i32,
    energy_drain_kj: i32,
) -> TurretStatus {
    if turret_query(world, vehicle, index).is_none() {
        return TurretStatus::Invalid;
    }
    let held = turret_ammo_remaining(world, vehicle, index);
    let short = if uses_vehicle_tanks_or_batteries(world, vehicle, index) {
        held < i64::from(ammo_required)
    } else {
        ammo_required != 0 && held < i64::from(ammo_required)
    };
    if short {
        return TurretStatus::NoAmmo;
    }
    let battery = fuel_left(world, vehicle, "battery");
    if i64::from(energy_drain_kj) > battery {
        return TurretStatus::NoPower;
    }
    TurretStatus::Ready
}
