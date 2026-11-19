//! **Which end of a vehicle a tow cable leaves from, and what a towing link is.**
//!
//! Reference: `vehicle::set_tow_directions` (`vehicle.cpp:7231-7251`) and `towing_data::set_towing` (`vehicle.cpp:7253-7263`). The direction rule is pure geometry over the vehicle's
//! mounts, so it is checked against the reference's own branches without a world at all; the link is checked as entity ids, because the reference's two raw pointers are the one thing the
//! port may not import.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::vehicle_tow::{
    TowPoint, Towing, clear_towing, no_towing_slack, set_towing, tow_cable_too_far, tow_direction,
    towed_velocity_at_stretch, towing_pull, towing_pushes, towing_strains,
};

/// **A five-mount vehicle is trisected**, and the tow part's normalised position decides the end. Reference: `set_tow_directions` - with `length = 5`, `trisect = 1`, so a mount in the
/// first third is `TOW_BACK`, the second `TOW_SIDE`, and the last `TOW_FRONT`.
#[test]
fn a_tow_cable_leaves_from_the_third_the_part_sits_in() {
    // The vehicle runs from mount 0 to mount 4.
    assert_eq!(tow_direction(0, 0, 4), TowPoint::Back, "the first third");
    assert_eq!(
        tow_direction(1, 0, 4),
        TowPoint::Back,
        "still the first third"
    );
    assert_eq!(
        tow_direction(2, 0, 4),
        TowPoint::Side,
        "**the middle third**"
    );
    assert_eq!(tow_direction(3, 0, 4), TowPoint::Front, "the last third");
    assert_eq!(tow_direction(4, 0, 4), TowPoint::Front, "the far end");

    // **A short vehicle is always front**, whatever it carries: `length < 3` is the reference's own short-circuit, and its comment says a small vehicle is no danger if it flips around.
    assert_eq!(
        tow_direction(0, 0, 1),
        TowPoint::Front,
        "a two-mount vehicle"
    );
    assert_eq!(
        tow_direction(0, 0, 0),
        TowPoint::Front,
        "and a single mount"
    );

    // **The bounds are the vehicle's own**, so the same part on a longer hull lands in a different third.
    assert_eq!(
        tow_direction(4, 0, 8),
        TowPoint::Side,
        "four along a nine-mount hull is the middle third (trisect 3)"
    );
    assert_eq!(
        tow_direction(6, 0, 8),
        TowPoint::Side,
        "**six is still the middle** - the comparison is `x <= 2 * trisect`, so the boundary mount belongs to the side"
    );
    assert_eq!(
        tow_direction(7, 0, 8),
        TowPoint::Front,
        "and seven is the last third"
    );
}

/// **A towing link is two entity ids, not two pointers.** Reference: `towing_data::set_towing` (`vehicle.cpp:7253-7263`) - the pulling records what it pulls, the towed records what pulls it,
/// and both recompute their direction.
#[test]
fn two_vehicles_link_by_id() {
    let mut world = World::new();
    let spec = |flags: &[&str]| cdda_content::vehicles::PartSpec {
        id: "hull".to_owned(),
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
        ..cdda_content::vehicles::PartSpec::default()
    };
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: vec![spec(&[]), spec(&["TOW"])],
            prototypes: Vec::new(),
        },
    );
    let vehicle_with = |world: &mut World, xs: &[i32]| {
        let parts: Vec<cdda_sim::vehicles::VehiclePart> = xs
            .iter()
            .map(|x| cdda_sim::vehicles::VehiclePart {
                part: "hull".to_owned(),
                x: *x,
                y: 0,
                ..cdda_sim::vehicles::VehiclePart::default()
            })
            .collect();
        world
            .spawn((
                cdda_sim::vehicles::Vehicle {
                    parts,
                    ..cdda_sim::vehicles::Vehicle::default()
                },
                cdda_sim::Position { x: 0, y: 0, z: 0 },
            ))
            .id()
    };
    let pulling = vehicle_with(&mut world, &[0, 1, 2, 3, 4]);
    let drawn = vehicle_with(&mut world, &[0, 1]);

    assert!(
        set_towing(&mut world, pulling, drawn),
        "**two real vehicles link**"
    );
    assert_eq!(
        world.get::<Towing>(pulling).map(|state| state.towing),
        Some(Some(drawn)),
        "the pulling records what it pulls"
    );
    assert_eq!(
        world.get::<Towing>(drawn).map(|state| state.towed_by),
        Some(Some(pulling)),
        "and the towed records what pulls it"
    );

    // **A link to nothing is refused**, which is the reference's `!towed || !tower` and, in the port, an entity that is not a vehicle.
    let not_a_vehicle = world.spawn(cdda_sim::Position { x: 9, y: 9, z: 0 }).id();
    assert!(
        !set_towing(&mut world, pulling, not_a_vehicle),
        "**a thing that is not a vehicle cannot be towed**"
    );
    assert!(
        !set_towing(&mut world, pulling, pulling),
        "and a vehicle cannot tow itself"
    );

    // **Clearing breaks both ends**, so neither vehicle is left pointing at a stranger.
    clear_towing(&mut world, pulling);
    assert!(
        world.get::<Towing>(pulling).is_none(),
        "the pulling holds no link"
    );
    assert_eq!(
        world.get::<Towing>(drawn).map(|state| state.towed_by),
        Some(None),
        "and the towed one is no longer pulled"
    );
}

/// **The distances a towing cable goes through.** Reference: `vehicle::no_towing_slack` (`vehicle.cpp:7354-7385`, `>= 8`), `vehicle::tow_cable_too_far` (`:7323-7352`, `>= 25`), and
/// `vehicle::do_towing_move`'s three thresholds (`:7119-7129`) - push under six, strain at seven, snap the towed hull up to the cable's speed at twelve.
#[test]
fn a_cable_goes_slack_taut_stretched_and_snapped() {
    // **Eight is where pulling starts**, and seven is still slack.
    assert!(!no_towing_slack(7), "seven squares is slack");
    assert!(no_towing_slack(8), "**eight pulls**");
    assert!(!towing_pushes(8), "and nothing is being pushed at eight");
    assert!(towing_pushes(5), "**under six the towed hull is pushed**");
    assert!(
        !towing_pushes(6),
        "six itself is neither push nor pull: `< 6`, not `<= 6`"
    );

    // **Seven is where the towed hull is dragged**, twelve where it is snapped up to speed.
    assert!(!towing_strains(6), "six does not drag");
    assert!(towing_strains(7), "**seven drags**");

    // **Twenty-five snaps the cable.** Twenty-four is a stretched but whole cable.
    assert!(!tow_cable_too_far(24), "twenty-four holds");
    assert!(tow_cable_too_far(25), "**twenty-five snaps it**");

    // **The snap-to-speed is the tower's velocity times 1.8**, negated when the cable leaves the towed vehicle's back, and the product truncates toward zero as the reference's
    // `int = int * double` does.
    assert_eq!(
        towed_velocity_at_stretch(100, false),
        180,
        "a hundred becomes a hundred and eighty"
    );
    assert_eq!(
        towed_velocity_at_stretch(100, true),
        -180,
        "and backwards turns it around"
    );
    assert_eq!(
        towed_velocity_at_stretch(50, false),
        90,
        "fifty becomes ninety"
    );
    assert_eq!(
        towed_velocity_at_stretch(3, false),
        5,
        "**three becomes five**, because 5.4 truncates - it does not round"
    );
    assert_eq!(
        towed_velocity_at_stretch(1, false),
        1,
        "**one becomes one**, because 1.8 truncates to 1 - which is what separates truncation from rounding"
    );
    assert_eq!(
        towed_velocity_at_stretch(0, false),
        0,
        "a still tower pulls nothing"
    );
}

/// **What a tower's movement does to the vehicle behind it.** Reference: `do_towing_move`'s arithmetic (`vehicle.cpp:7119-7129`) - push under six, drag at seven when the towed hull is
/// no faster, and the velocity **assigned** (the tower's, or 1.8 times it at twelve or more).
#[test]
fn a_tower_pulls_by_distance_and_direction() {
    // **Under six the towed hull is pushed**, and its velocity is the tower's.
    assert_eq!(
        towing_pull(5, 100, 100, false),
        cdda_sim::vehicle_tow::TowingPull {
            accel_y: 1,
            velocity: 100
        },
        "**five squares pushes it along at the tower's speed**"
    );
    assert_eq!(
        towing_pull(5, 100, 100, true),
        cdda_sim::vehicle_tow::TowingPull {
            accel_y: -1,
            velocity: -100
        },
        "and a reversing tower pushes the other way"
    );

    // **Six is neither**: the push band is `< 6` and the drag band is `>= 7`.
    assert_eq!(
        towing_pull(6, 100, 50, false),
        cdda_sim::vehicle_tow::TowingPull {
            accel_y: 0,
            velocity: 100
        },
        "six squares neither pushes nor drags"
    );

    // **At seven a slower towed hull is dragged**: `accel_y` is minus one forward.
    assert_eq!(
        towing_pull(8, 100, 50, false),
        cdda_sim::vehicle_tow::TowingPull {
            accel_y: -1,
            velocity: 100
        },
        "**eight squares drags a slower hull**"
    );
    // **A towed hull that is already faster is not dragged**, because the reference's second condition asks `towed_veh->velocity <= velocity`.
    assert_eq!(
        towing_pull(8, 100, 150, false),
        cdda_sim::vehicle_tow::TowingPull {
            accel_y: 0,
            velocity: 100
        },
        "and one that is faster is left to coast, though its velocity is still the tower's"
    );

    // **At twelve the velocity is 1.8 times the tower's**, and the drag still applies.
    assert_eq!(
        towing_pull(12, 100, 50, false),
        cdda_sim::vehicle_tow::TowingPull {
            accel_y: -1,
            velocity: 180
        },
        "**twelve squares snaps it up to a hundred and eighty**"
    );
    assert_eq!(
        towing_pull(12, 100, 50, true),
        cdda_sim::vehicle_tow::TowingPull {
            accel_y: 1,
            velocity: -180
        },
        "and in reverse, backwards and pushed"
    );
}

/// **A towed vehicle drives itself only because it is towed**, and its turn and acceleration are the cable's. Reference: `vehicle::selfdrive` (`vehicle_move.cpp:1322-1355`) - the opening
/// guard admits a towed, magic, harnessed or autopilot vehicle and nothing else, the turn is `turn × steer_increment`, and the accelerator is pressed by **minus** the acceleration.
#[test]
fn a_towed_vehicle_drives_itself_but_an_untowed_one_does_not() {
    use cdda_sim::vehicle_tow::{SelfDrive, selfdrive};

    assert!(
        SelfDrive {
            towed: true,
            ..SelfDrive::default()
        }
        .allowed(),
        "**a towed vehicle may be driven**"
    );
    assert!(
        !SelfDrive::default().allowed(),
        "and an untowed one with no magic, animal or autopilot may not"
    );
    assert!(
        SelfDrive {
            autopilot: true,
            ..SelfDrive::default()
        }
        .allowed(),
        "an autopilot part is enough on its own"
    );

    // **A vehicle with no link refuses to drive itself**, which is the guard's `return`.
    let mut world = World::new();
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
    assert!(
        !selfdrive(&mut world, vehicle, 2, 1),
        "**an untowed hull does not steer itself**"
    );

    // **With a tower behind it, the same call turns its wheels** by two increments of fifteen degrees.
    let tower = world
        .spawn((
            cdda_sim::vehicles::Vehicle {
                parts: vec![cdda_sim::vehicles::VehiclePart {
                    part: "hull".to_owned(),
                    ..cdda_sim::vehicles::VehiclePart::default()
                }],
                ..cdda_sim::vehicles::Vehicle::default()
            },
            cdda_sim::Position { x: 0, y: 5, z: 0 },
        ))
        .id();
    assert!(
        cdda_sim::vehicle_tow::set_towing(&mut world, tower, vehicle),
        "the two are linked"
    );
    let before = cdda_sim::vehicle_move::wheel_direction(&world, vehicle);
    assert!(
        selfdrive(&mut world, vehicle, 2, 0),
        "**a towed hull steers itself**"
    );
    let after = cdda_sim::vehicle_move::wheel_direction(&world, vehicle);
    assert_eq!(
        (before + 30).rem_euclid(360),
        after,
        "**two increments is thirty degrees**"
    );
}

/// **A side-hitched hull skids one square toward its tower.** Reference: `do_towing_move`'s `default` branch (`vehicle.cpp:7141-7152`) - `tower − nearby`, each component clamped into
/// `[-1, 1]`, where `nearby` is the first step of the line from the towed hull's tow point toward the tower's.
#[test]
fn a_side_hitched_hull_skids_toward_its_tower() {
    use cdda_sim::vehicle_tow::{nearby_destination, side_hitch_step};

    // **`line_to` does not include its origin**: its walk steps first and reports after, in the port and in the reference's own `bresenham`, so index 1 is **two** squares along the
    // line. That is the square `do_towing_move` subtracts from the tower.
    assert_eq!(
        nearby_destination((0, 0), (0, 10)),
        (0, 2),
        "two squares north"
    );
    assert_eq!(
        side_hitch_step((0, 10), (0, 2)),
        (0, 1),
        "**so the hull is dragged one square north**, toward the tower"
    );

    // **A diagonal line steps diagonally**, and the clamped delta keeps it diagonal rather than preferring an axis.
    assert_eq!(
        nearby_destination((0, 0), (5, 5)),
        (2, 2),
        "two squares north-east"
    );
    assert_eq!(
        side_hitch_step((5, 5), (2, 2)),
        (1, 1),
        "and the drag follows it"
    );

    // **The clamp is what makes it one square**: from twelve squares away the raw delta is eleven, and the reference moves the hull by one.
    assert_eq!(
        side_hitch_step((0, 12), (0, 2)),
        (0, 1),
        "**ten squares of difference is still one square of movement**"
    );

    // **On the same square there is nothing to do**: the delta is zero and the hull stays where it is.
    assert_eq!(
        side_hitch_step((4, 4), (4, 4)),
        (0, 0),
        "no step when it is already there"
    );
    assert_eq!(
        nearby_destination((4, 4), (4, 4)),
        (4, 4),
        "and a line with no second cell falls back to the target's own square"
    );
}
