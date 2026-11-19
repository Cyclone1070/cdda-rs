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
        &[("t_open_air", 1, 0, &["NO_FLOOR"])],
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
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 0, &["NO_FLOOR"])],
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
    cdda_sim::terrain_types::set_move_costs(&mut world, [("t_open_air".to_owned(), 0_i32)]);
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
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 0, &["NO_FLOOR"])],
        &[(0, 0, 1), (1, 0, 2)],
        &[("hull", 0, 0), ("hull", 1, 0)],
    );
    assert!(
        !check_support(&world, vehicle).falling,
        "**a part is held up, so the hull is not falling** - open air and all"
    );

    // The other order, and the same squares: the open air now comes first, and the answer is the same.
    let (world, vehicle) = world_with(
        &[("t_floor", 1, 2, &[]), ("t_open_air", 2, 0, &["NO_FLOOR"])],
        &[(0, 0, 1), (1, 0, 2)],
        &[("hull", 1, 0), ("hull", 0, 0)],
    );
    assert!(
        !check_support(&world, vehicle).falling,
        "**and the order does not matter**: the flag is lowered at the supported part either way"
    );

    // Every part over open air is the only hull that falls.
    let (world, vehicle) = world_with(
        &[("t_open_air", 2, 0, &["NO_FLOOR"])],
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
