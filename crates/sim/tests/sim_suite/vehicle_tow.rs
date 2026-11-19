//! **Which end of a vehicle a tow cable leaves from, and what a towing link is.**
//!
//! Reference: `vehicle::set_tow_directions` (`vehicle.cpp:7231-7251`) and `towing_data::set_towing` (`vehicle.cpp:7253-7263`). The direction rule is pure geometry over the vehicle's
//! mounts, so it is checked against the reference's own branches without a world at all; the link is checked as entity ids, because the reference's two raw pointers are the one thing the
//! port may not import.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::vehicle_tow::{
    TowPoint, Towing, clear_towing, no_towing_slack, set_towing, tow_cable_too_far, tow_direction,
    towed_velocity_at_stretch, towing_pushes, towing_strains,
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
