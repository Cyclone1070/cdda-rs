//! **Whether the ground holds a vehicle up.** Reference: `vehicle::check_falling_or_floating` (`vehicle_move.cpp:2059-2135`), three passes in one: the wheels, then every part, then
//! the water the hull stands in.
//!
//! The asymmetry is the rule: **a wheel on water is not supported, a hull over water is** - the reference passes `water_supports` false for the wheels and true for the parts.

#![allow(clippy::panic, clippy::expect_used, clippy::too_many_lines)]

use bevy_ecs::prelude::*;
use cdda_sim::square::{Furniture, Terrain};
use cdda_sim::vehicle_falling::{Support, check_support, has_support, supports_above};
use cdda_sim::vehicles::{Vehicle, VehiclePart};
use cdda_sim::{Position, PositionIndex};

/// A world with a terrain table, the squares named, and one vehicle carrying `parts`.
fn world_with(
    terrains: &[(&str, u32, i32, &[&str])],
    squares: &[(i32, i32, u32)],
    parts: &[(&str, i32, i32)],
) -> (World, Entity) {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        terrains
            .iter()
            .map(|(name, code, _, _)| ((*name).to_owned(), *code)),
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        terrains.iter().map(|(name, _, _, flags)| {
            (
                (*name).to_owned(),
                flags.iter().map(|flag| (*flag).to_owned()).collect(),
            )
        }),
    );
    cdda_sim::terrain_types::set_move_costs(
        &mut world,
        terrains
            .iter()
            .map(|(name, _, cost, _)| ((*name).to_owned(), *cost)),
    );
    // One part prototype per distinct part name, so that a wheel is a wheel because the store says so.
    let mut specs = Vec::new();
    for (name, _, _) in parts {
        if specs
            .iter()
            .any(|spec: &cdda_content::vehicles::PartSpec| spec.id == *name)
        {
            continue;
        }
        specs.push(cdda_content::vehicles::PartSpec {
            id: (*name).to_owned(),
            flags: if *name == "wheel" {
                ["WHEEL".to_owned()].into_iter().collect()
            } else {
                std::collections::BTreeSet::new()
            },
            ..cdda_content::vehicles::PartSpec::default()
        });
    }
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: specs,
            prototypes: Vec::new(),
        },
    );
    for (x, y, code) in squares {
        world.spawn((Terrain { code: *code }, Position { x: *x, y: *y, z: 0 }));
    }
    let vehicle = world
        .spawn((
            Vehicle {
                parts: parts
                    .iter()
                    .map(|(name, x, y)| VehiclePart {
                        part: (*name).to_owned(),
                        x: *x,
                        y: *y,
                        ..VehiclePart::default()
                    })
                    .collect(),
                ..Vehicle::default()
            },
            Position { x: 0, y: 0, z: 0 },
        ))
        .id();
    PositionIndex::rebuild(&mut world);
    (world, vehicle)
}

#[test]
fn a_hull_whose_wheels_stand_on_the_ground_is_not_falling() {
    // Ground is any square that is neither open air nor water.
    let (world, vehicle) = world_with(
        &[("t_floor", 1, 2, &[])],
        &[(0, 0, 1), (1, 0, 1)],
        &[("wheel", 0, 0), ("wheel", 1, 0)],
    );
    assert_eq!(
        check_support(&world, vehicle),
        Support::default(),
        "**both wheels are held up, so nothing else is asked**"
    );
}

#[test]
fn a_hull_over_open_air_falls() {
    // `t_open_air` is open air with nothing under it. A hull standing on its own hull is not supported either: the only vehicle is the one falling.
    let (world, vehicle) = world_with(
        &[("t_open_air", 1, 2, &["NO_FLOOR"])],
        &[(0, 0, 1), (1, 0, 1)],
        &[("wheel", 0, 0), ("wheel", 1, 0)],
    );
    let support = check_support(&world, vehicle);
    assert!(
        support.falling,
        "**open air holds nothing up**: {support:?}"
    );
    assert!(
        !support.in_water && !support.in_deep_water,
        "and there is no water to be in"
    );
}

#[test]
fn open_air_over_a_wall_is_held_up_by_it() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_open_air".to_owned(), 1_u32),
            ("t_wall".to_owned(), 2_u32),
        ],
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [
            ("t_open_air".to_owned(), vec!["NO_FLOOR".to_owned()]),
            ("t_wall".to_owned(), Vec::new()),
        ],
    );
    cdda_sim::terrain_types::set_move_costs(
        &mut world,
        [
            ("t_open_air".to_owned(), 0_i32),
            ("t_wall".to_owned(), 0_i32),
        ],
    );
    // The hull is on the second floor: open air at z=1, a wall at z=0 under it.
    world.spawn((Terrain { code: 1 }, Position { x: 0, y: 0, z: 1 }));
    world.spawn((Terrain { code: 2 }, Position { x: 0, y: 0, z: 0 }));
    PositionIndex::rebuild(&mut world);

    assert!(
        supports_above(&world, (0, 0, 0)),
        "**a wall holds up what stands above it**, because nothing can walk onto it"
    );
    assert!(
        has_support(&world, (0, 0, 1), true),
        "so the open air above the wall is held up"
    );
}

#[test]
fn water_holds_a_hull_but_not_a_wheel() {
    let (world, vehicle) = world_with(
        &[("t_water", 1, 0, &["SWIMMABLE", "DEEP_WATER"])],
        &[(0, 0, 1), (1, 0, 1)],
        &[("wheel", 0, 0), ("wheel", 1, 0)],
    );
    assert!(
        !has_support(&world, (0, 0, 0), false),
        "**a wheel does not float**: water does not hold the wheel up"
    );
    assert!(
        has_support(&world, (0, 0, 0), true),
        "but the same water holds the hull up, which is the same rule asked the other way"
    );
    let support = check_support(&world, vehicle);
    assert!(!support.falling, "so the hull does not fall");
    assert!(support.in_water, "it is in water: both parts stand in it");
    assert!(
        support.in_deep_water,
        "and in deep water, since all of it is deep"
    );
}

#[test]
fn half_in_shallow_water_is_in_water_but_not_deep_water() {
    let (world, vehicle) = world_with(
        &[
            ("t_water_shallow", 1, 0, &["SWIMMABLE"]),
            ("t_water", 2, 0, &["SWIMMABLE", "DEEP_WATER"]),
        ],
        &[(0, 0, 1), (1, 0, 2), (2, 0, 1)],
        &[("hull", 0, 0), ("hull", 1, 0), ("hull", 2, 0)],
    );
    let support = check_support(&world, vehicle);
    assert!(!support.falling, "water holds the hull up");
    assert!(support.in_water, "**all three squares are water**");
    assert!(
        !support.in_deep_water,
        "but only one of three is deep, and two thirds is the threshold"
    );
}

#[test]
fn the_bottom_of_the_world_holds_up_everything() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_open_air".to_owned(), 1_u32)]);
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [("t_open_air".to_owned(), vec!["NO_FLOOR".to_owned()])],
    );
    world.spawn((Terrain { code: 1 }, Position { x: 0, y: 0, z: -10 }));
    PositionIndex::rebuild(&mut world);
    assert!(
        has_support(&world, (0, 0, -10), false),
        "**there is nothing below the bottom of the world**, so it holds"
    );
}

#[test]
fn half_the_wheels_standing_is_enough() {
    let (world, vehicle) = world_with(
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 2, &["NO_FLOOR"])],
        &[(0, 0, 1), (1, 0, 2)],
        // **The wheel over the open air comes first**, so that the early return is what answers: without it the part pass would reach this wheel and call the hull falling, which
        // is the difference between "half the wheels is enough" and "the first part decides".
        &[("wheel", 1, 0), ("wheel", 0, 0)],
    );
    assert!(
        !check_support(&world, vehicle).falling,
        "**one of two wheels standing is half of them**, which is the reference's threshold"
    );
}

#[test]
fn furniture_that_cannot_be_walked_through_holds_up_what_is_above_it() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_open_air".to_owned(), 1_u32)]);
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [("t_open_air".to_owned(), vec!["NO_FLOOR".to_owned()])],
    );
    cdda_sim::terrain_types::set_move_costs(&mut world, [("t_open_air".to_owned(), 2_i32)]);
    cdda_sim::furniture_types::set_types(&mut world, [("f_wall".to_owned(), 7_u32)]);
    cdda_sim::furniture_types::set_move_costs(&mut world, [("f_wall".to_owned(), -1_i32)]);
    world.spawn((Furniture { code: 7 }, Position { x: 0, y: 0, z: 0 }));
    PositionIndex::rebuild(&mut world);
    assert!(
        supports_above(&world, (0, 0, 0)),
        "**furniture with a negative move cost is an obstacle**, and an obstacle holds up what stands on it"
    );
}

/// **One supported part is enough**, whatever the order the parts come in. `check_falling_or_floating` lowers `is_falling` as soon as a part stands on something and skips the
/// support test afterwards, and the flag never rises again - so a hull hanging over open air with a single supported part is **not falling**. Looser than "every part must be
/// held up", and worth pinning: a port that tightened it would disagree with the reference on every hull that overhangs an edge.
#[test]
fn one_supported_part_is_enough_whatever_the_order() {
    let (world, vehicle) = world_with(
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 2, &["NO_FLOOR"])],
        &[(0, 0, 1), (1, 0, 2)],
        &[("hull", 0, 0), ("hull", 1, 0)],
    );
    assert!(
        !check_support(&world, vehicle).falling,
        "**a part is held up, so the hull is not falling** - open air and all"
    );

    // The other order, and the same squares: the open air now comes first, and the answer is the same.
    let (world, vehicle) = world_with(
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 2, &["NO_FLOOR"])],
        &[(0, 0, 1), (1, 0, 2)],
        &[("hull", 1, 0), ("hull", 0, 0)],
    );
    assert!(
        !check_support(&world, vehicle).falling,
        "**and the order does not matter**: the flag is lowered at the supported part either way"
    );

    // Every part over open air is the only hull that falls.
    let (world, vehicle) = world_with(
        &[("t_open_air", 2, 2, &["NO_FLOOR"])],
        &[(0, 0, 2), (1, 0, 2)],
        &[("hull", 0, 0), ("hull", 1, 0)],
    );
    assert!(
        check_support(&world, vehicle).falling,
        "while a hull with **no** part held up does fall"
    );
}

/// **Two thirds of the hull in deep water is in deep water** - the threshold exactly, which is the only case that tells `>=` from `>`.
#[test]
fn two_thirds_of_the_hull_in_deep_water_is_deep_water() {
    let (world, vehicle) = world_with(
        &[
            ("t_water_shallow", 1, 0, &["SWIMMABLE"]),
            ("t_water", 2, 0, &["SWIMMABLE", "DEEP_WATER"]),
        ],
        &[(0, 0, 2), (1, 0, 2), (2, 0, 1)],
        &[("hull", 0, 0), ("hull", 1, 0), ("hull", 2, 0)],
    );
    let support = check_support(&world, vehicle);
    assert!(support.in_water, "all three squares are water");
    assert!(
        support.in_deep_water,
        "**two of three is two thirds**, which meets the reference's threshold exactly"
    );
}

/// **A fall accelerates, and the numbers are the reference's.** `v_2 = -sqrt( 2 * d * g + v_1^2 )` with a four-metre level and `g` at 9.8, computed in single precision exactly as
/// `vehicle_move.cpp:1869-1877` does - so `-1980` after one level, not `-1981`, because the conversion to hundredths of a mile per hour truncates towards zero.
#[test]
fn a_fall_accelerates_by_the_references_own_numbers() {
    assert_eq!(
        cdda_sim::vehicle_falling::falling_velocity(0),
        -1980,
        "**from rest, one level of fall is 8.85 m/s**, which is -1980 in hundredths of a mile per hour"
    );
    assert_eq!(
        cdda_sim::vehicle_falling::falling_velocity(-1980),
        -2800,
        "and the next level takes it to 12.52 m/s"
    );
    assert_eq!(
        cdda_sim::vehicle_falling::falling_velocity(-2800),
        -3429,
        "and the one after that to 15.33"
    );
    // The sign is the reference's: a fall is negative whichever way it was going.
    assert!(
        cdda_sim::vehicle_falling::falling_velocity(1980) < 0,
        "**a hull moving upwards still comes down**, because the square root drops the sign"
    );
}

#[test]
fn the_speed_conversions_round_trip_through_metres_per_second() {
    use cdda_sim::vehicle_falling::{metres_per_second_to_vmiph, vmiph_to_metres_per_second};
    assert_eq!(metres_per_second_to_vmiph(0.0), 0, "standing still is zero");
    assert_eq!(
        metres_per_second_to_vmiph(1.0),
        223,
        "**one metre a second is 223**"
    );
    let one_mile_per_hour = vmiph_to_metres_per_second(100);
    assert!(
        (one_mile_per_hour - 0.447_039_3).abs() < 1e-7,
        "**a hundred is one mile per hour**, 0.447 m/s: got {one_mile_per_hour}"
    );
    // **And it does not come back exactly**, which is the reference's own arithmetic rather than the port's: `vehicle_move.cpp:1873` stores the conversion in a `float`, so a
    // mile per hour narrows to 0.44703928 and 223.694 of *that* is 99.99999, which the `int` return truncates to **99**. A round trip that gained the digit back would be a port
    // that had widened the reference's single precision somewhere.
    assert_eq!(
        metres_per_second_to_vmiph(vmiph_to_metres_per_second(100)),
        99,
        "**the reference's float narrowing costs a hundredth**, so a mile per hour returns as 99"
    );
}

#[test]
fn a_hull_keeps_falling_once_it_has_started() {
    use cdda_sim::vehicle_falling::should_fall;
    assert!(
        !should_fall(false, 0),
        "a hull on the ground with no velocity is not falling"
    );
    assert!(
        should_fall(true, 0),
        "**a flagged hull falls even at rest**"
    );
    assert!(
        should_fall(false, -1980),
        "and a hull with downward velocity keeps falling after the flag clears"
    );
    assert!(should_fall(false, 1980), "in either direction");
}

/// **The ground is asked after every step of a turn**, which is the reference's `veh.check_falling_or_floating()` at `map.cpp:953` - so a hull that has just put its wheels over
/// open air is flagged before the next turn reads it. The test drives the **turn**, not `settle`, so that a turn which stopped asking the ground would fail it.
#[test]
fn a_hull_driven_onto_open_air_is_flagged_as_falling() {
    let (mut world, vehicle) = world_with(
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 2, &["NO_FLOOR"])],
        &[(0, 0, 1), (1, 0, 2), (2, 0, 2)],
        // **Two wheels in a row**, because one is not a wheeled vehicle (`sufficient_wheel_config` wants the single one to be `STABLE` on a three-tile hull) - and because half the
        // wheels standing is enough to hold a hull up, so the step has to take **both** of them over the edge for the flag to fire.
        &[("wheel", 0, 0), ("wheel", 1, 0)],
    );
    assert!(
        !world
            .get::<Vehicle>(vehicle)
            .is_some_and(|hull| hull.is_falling),
        "it starts on solid ground"
    );
    // Rolling east at four miles an hour, with a move to spend: `move_vehicles` only steps a hull that has both, and the next square is open air.
    if let Some(mut hull) = world.get_mut::<Vehicle>(vehicle) {
        hull.velocity = 400;
        hull.of_turn = 10_000.0;
    }
    assert_eq!(
        cdda_sim::vehicle_move::move_vehicles(&mut world),
        1,
        "the hull takes a step"
    );
    assert_eq!(
        world.get::<Position>(vehicle).map(|at| (at.x, at.y, at.z)),
        Some((1, 0, 0)),
        "onto the square with no floor"
    );
    assert!(
        world
            .get::<Vehicle>(vehicle)
            .is_some_and(|hull| hull.is_falling),
        "**and the turn flags the hull as falling**, because the ground under it no longer holds it"
    );
}

/// **Landing is not the support rule's doing.** The reference zeroes `vertical_velocity` in its vertical collision (`vehicle_move.cpp:803-807`), so a hull that is held up again
/// keeps whatever fall speed it had until an impact spends it - the flag and the velocity are set by different rules.
#[test]
fn being_held_up_again_clears_the_flag_but_not_the_speed() {
    let (mut world, vehicle) = world_with(
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 2, &["NO_FLOOR"])],
        &[(0, 0, 1)],
        &[("wheel", 0, 0)],
    );
    let falling = Vehicle {
        is_falling: true,
        vertical_velocity: -1980,
        ..world.get::<Vehicle>(vehicle).cloned().expect("a hull")
    };
    world.entity_mut(vehicle).insert(falling);
    cdda_sim::vehicle_falling::settle(&mut world, vehicle);
    let hull = world.get::<Vehicle>(vehicle).cloned().expect("a hull");
    assert!(
        !hull.is_falling,
        "solid ground under the wheel clears the flag"
    );
    assert_eq!(
        hull.vertical_velocity, -1980,
        "**but the fall speed survives**, because only an impact spends it"
    );
}

/// **A falling hull steps down a level and gains speed**, and it does not move sideways while it does - the reference's `dp.z() = -1` with no lateral component
/// (`vehicle_move.cpp:1958-1998`). The velocity is the gravity ladder the port already gates: `0 -> -1980 -> -2800`.
#[test]
fn a_falling_hull_steps_down_and_gains_speed() {
    // A hull hanging over open air at the **second** level, so that there is a level below to fall into.
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_open_air".to_owned(), 1_u32),
            ("t_floor".to_owned(), 2_u32),
        ],
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [
            ("t_open_air".to_owned(), vec!["NO_FLOOR".to_owned()]),
            ("t_floor".to_owned(), Vec::new()),
        ],
    );
    cdda_sim::terrain_types::set_move_costs(
        &mut world,
        [
            ("t_open_air".to_owned(), 2_i32),
            ("t_floor".to_owned(), 2_i32),
        ],
    );
    // **Both levels are open air**, because a hull falling through empty space is only held up if the square it enters says so - the port reads a square nobody described as solid,
    // which is the one place its world is thinner than the reference's and is named in `docs/parity.md`.
    for z in [1, 2] {
        world.spawn((Terrain { code: 1 }, Position { x: 0, y: 0, z }));
        world.spawn((Terrain { code: 1 }, Position { x: 1, y: 0, z }));
    }
    let vehicle = world
        .spawn((
            Vehicle {
                parts: vec![
                    VehiclePart {
                        part: "wheel".to_owned(),
                        ..VehiclePart::default()
                    },
                    VehiclePart {
                        part: "wheel".to_owned(),
                        x: 1,
                        ..VehiclePart::default()
                    },
                ],
                // Falling, and fast already: the first step of this fall follows the ladder from zero.
                is_falling: true,
                vertical_velocity: 0,
                ..Vehicle::default()
            },
            Position { x: 0, y: 0, z: 2 },
        ))
        .id();
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: vec![cdda_content::vehicles::PartSpec {
                id: "wheel".to_owned(),
                flags: ["WHEEL".to_owned()].into_iter().collect(),
                ..cdda_content::vehicles::PartSpec::default()
            }],
            prototypes: Vec::new(),
        },
    );
    PositionIndex::rebuild(&mut world);

    assert_eq!(
        cdda_sim::vehicle_move::move_vehicles(&mut world),
        1,
        "the hull falls one level"
    );
    let at = world.get::<Position>(vehicle).copied().expect("a hull");
    assert_eq!(
        (at.x, at.y, at.z),
        (0, 0, 1),
        "**straight down, and not sideways**: a fall has no air control"
    );
    let hull = world.get::<Vehicle>(vehicle).cloned().expect("a hull");
    assert_eq!(
        hull.vertical_velocity, -1980,
        "and it is now moving at the first rung of the gravity ladder"
    );
    assert!(hull.is_falling, "still falling, until something stops it");

    assert_eq!(
        cdda_sim::vehicle_move::move_vehicles(&mut world),
        1,
        "the next turn falls another level"
    );
    let hull = world.get::<Vehicle>(vehicle).cloned().expect("a hull");
    assert_eq!(hull.vertical_velocity, -2800, "at the second rung");
    assert_eq!(
        world.get::<Position>(vehicle).map(|at| at.z),
        Some(0),
        "one level lower again"
    );
}
