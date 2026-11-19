//! **A vehicle that drives onto a ramp changes level.** Reference: `map::displace_vehicle`'s ramp block (`map.cpp:1463-1476`) - the **destination** square decides, `RAMP_UP` lifts
//! one level, `RAMP_DOWN` drops one, and the reference writes the pair as `if`/`else if` so a square carrying both would rise.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::square::Terrain;
use cdda_sim::vehicle_move::ramp_offset;

fn world_with(terrains: &[(&str, u32, &[&str])]) -> World {
    let mut world = World::new();
    // The table is installed first, because a flag is a property of a code and a code only exists once its terrain does.
    cdda_sim::terrain_types::set_types(
        &mut world,
        terrains
            .iter()
            .map(|(name, code, _)| ((*name).to_owned(), *code)),
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        terrains.iter().map(|(name, _, flags)| {
            (
                (*name).to_owned(),
                flags.iter().map(|flag| (*flag).to_owned()).collect(),
            )
        }),
    );
    world
}

/// **The offset is the destination's**: up one, down one, or nothing, and up wins where both flags are present.
#[test]
fn the_destination_decides_the_level_change() {
    assert_eq!(ramp_offset(true, false), 1, "**an up ramp lifts**");
    assert_eq!(ramp_offset(false, true), -1, "a down ramp drops");
    assert_eq!(ramp_offset(false, false), 0, "flat ground does nothing");
    assert_eq!(
        ramp_offset(true, true),
        1,
        "**a square carrying both rises**, because the reference's second test is an `else if`"
    );
}

/// **Driving onto a ramp changes the vehicle's level**, and driving off it does not change it back: the reference reads only the destination square, so a vehicle that has left a
/// ramp stays at the level it reached.
#[test]
fn a_vehicle_drives_up_a_ramp_and_stays_there() {
    let mut world = world_with(&[
        ("t_floor", 1, &[]),
        ("t_ramp_up", 2, &["RAMP_UP"]),
        ("t_ramp_down", 3, &["RAMP_DOWN"]),
    ]);
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: vec![cdda_content::vehicles::PartSpec {
                id: "hull".to_owned(),
                ..cdda_content::vehicles::PartSpec::default()
            }],
            prototypes: Vec::new(),
        },
    );
    let vehicle = world
        .spawn((
            cdda_sim::vehicles::Vehicle {
                parts: vec![cdda_sim::vehicles::VehiclePart {
                    part: "hull".to_owned(),
                    ..cdda_sim::vehicles::VehiclePart::default()
                }],
                ..cdda_sim::vehicles::Vehicle::default()
            },
            cdda_sim::Position { x: 0, y: 0, z: 0 },
        ))
        .id();

    // **The ground at each square** is its own entity, the way the map keeps it - and a ramp is a structure with a half on each level, so the level a vehicle has risen to has
    // ground of its own. Driving off the top of a ramp onto open air is a fall, which is a different rule and not what this test is about.
    let ground = |world: &mut World, x: i32, y: i32, z: i32, code: u32| {
        world.spawn((Terrain { code }, cdda_sim::Position { x, y, z }));
    };
    ground(&mut world, 0, 0, 0, 1);
    ground(&mut world, 1, 0, 0, 2);
    ground(&mut world, 2, 0, 0, 1);
    ground(&mut world, 3, 0, 0, 3);
    // The same run one level up: the ramp's lower square has a floor above it, and the flat squares carry a deck.
    ground(&mut world, 1, 0, 1, 1);
    ground(&mut world, 2, 0, 1, 1);
    ground(&mut world, 3, 0, 1, 3);
    cdda_sim::PositionIndex::rebuild(&mut world);

    assert!(
        cdda_sim::vehicles::move_vehicle(&mut world, vehicle, (1, 0)),
        "the vehicle steps onto the up ramp"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(vehicle).map(|pos| pos.z),
        Some(1),
        "**and it is one level higher**"
    );
    assert!(
        cdda_sim::vehicles::move_vehicle(&mut world, vehicle, (1, 0)),
        "then onto flat ground"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(vehicle).map(|pos| pos.z),
        Some(1),
        "**and stays at the level it reached**, because only the destination is read"
    );
    assert!(
        cdda_sim::vehicles::move_vehicle(&mut world, vehicle, (1, 0)),
        "then onto a down ramp"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(vehicle).map(|pos| pos.z),
        Some(0),
        "which drops it back"
    );
}
