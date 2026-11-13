//! **A vehicle is one entity holding its parts**, built from the reference's own blueprint.
//!
//! Reference: `vehicle` (`vehicle.h`) holds `std::vector<vehicle_part>` and answers `parts_at_relative( mount, use_variant )` - "what is in this square" - over that list.
//! This gate builds the reference's 2-seater helicopter from `corpus/vehicles.json`, checks that its 71 placements became 71 parts with the store's own durability as their hit
//! points, breaks one of them with the damage rule, and asks what stands in the origin square.
//!
//! It also checks **where the vehicle stands**: every distinct mount its parts occupy becomes a square in the one derived index, so a square query answers the vehicle
//! without ever scanning the vehicles.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn store() -> cdda_content::vehicles::Vehicles {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicles.json");
    let bytes = std::fs::read(path).expect("the recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    cdda_content::vehicles::from_dump(&value).expect("the store loads")
}

fn world_with_the_helicopter() -> (World, Entity) {
    let mut world = World::new();
    let materials = store();
    cdda_sim::vehicles::set_store(&mut world, materials);
    let vehicle = cdda_sim::vehicles::spawn(&mut world, "2seater2", (10, 10, 0)).expect("the helicopter builds");
    (world, vehicle)
}

#[test]
fn a_prototype_becomes_one_entity_holding_every_part_its_blueprint_names() {
    let (world, vehicle) = world_with_the_helicopter();
    let built = world.get::<cdda_sim::vehicles::Vehicle>(vehicle).expect("a vehicle");
    assert_eq!(built.parts.len(), 71, "**one part per placement**, and the blueprint has 71");
    let frame = &built.parts[0];
    assert_eq!(frame.part, "frame", "the blueprint's first placement");
    assert_eq!((frame.x, frame.y), (0, 0), "in the origin square");
    assert_eq!(
        frame.hp, 400,
        "**a fresh part's hit points are its store durability**, which for a cross frame is 400"
    );
    assert!(!frame.broken, "and nothing starts broken");
    assert_eq!(
        world.get::<cdda_sim::Position>(vehicle).copied().map(|at| (at.x, at.y, at.z)),
        Some((10, 10, 0)),
        "the vehicle stands where it was built"
    );
}

#[test]
fn a_square_can_hold_several_parts_and_the_query_says_which() {
    let (world, vehicle) = world_with_the_helicopter();
    let occupants = cdda_sim::vehicles::parts_at(&world, vehicle, 0, 0);
    assert!(
        occupants.len() > 1,
        "**the origin square holds the frame, the seat, the seatbelt and the controls**: {occupants:?}"
    );
    assert_eq!(
        cdda_sim::vehicles::parts_at(&world, vehicle, 99, 99).len(),
        0,
        "and a square nothing stands in holds nothing"
    );
}

#[test]
fn a_hit_that_reaches_the_hit_points_breaks_the_part_and_leaves_it_in_place() {
    let (mut world, vehicle) = world_with_the_helicopter();
    // A cross frame: 400 hit points, so its threshold is `min(20, 40) = 20`, and it shaves **20** off every kind - the dump's own number for a frame.
    let outcome = cdda_sim::vehicles::damage(&mut world, vehicle, 0, 450, "bash", false);
    assert_eq!(
        outcome,
        cdda_sim::vehicle_damage::Outcome::Applied { net: 430, hp_after: 0, broke: true },
        "a 450-point bash lands for 430 after the frame's 20 armour, which takes it to zero"
    );
    let built = world.get::<cdda_sim::vehicles::Vehicle>(vehicle).expect("a vehicle");
    assert!(built.parts[0].broken, "**the part is broken and still there**: a vehicle does not lose its frame");
    assert_eq!(built.parts[0].hp, 0, "with no hit points left");
    assert_eq!(
        cdda_sim::vehicles::parts_at(&world, vehicle, 0, 0).len(),
        built.parts.iter().filter(|part| (part.x, part.y) == (0, 0)).count(),
        "and it still occupies its square"
    );
}

#[test]
fn a_vehicle_stands_on_every_square_its_parts_occupy() {
    let (mut world, vehicle) = world_with_the_helicopter();
    cdda_sim::PositionIndex::rebuild(&mut world);

    let occupied = cdda_sim::vehicles::occupied_squares(&world, vehicle);
    assert_eq!(
        occupied.len(),
        24,
        "**71 parts stand on 24 distinct squares**, which is what the dump's mounts say"
    );
    for square in [(10, 10, 0), (5, 10, 0), (11, 9, 0)] {
        assert!(
            occupied.contains(&square),
            "the blueprint puts a part at {square:?}: {occupied:?}"
        );
    }
    assert!(
        !occupied.contains(&(12, 10, 0)),
        "nothing stands one column past the tail"
    );

    let index = cdda_sim::singletons::index(&world).expect("the index was rebuilt");
    let mut listed = Vec::new();
    for z in -1..=1 {
        for y in 0..=20 {
            for x in 0..=20 {
                if index.at_level(x, y, z).contains(&vehicle) {
                    listed.push((x, y, z));
                }
            }
        }
    }
    let mut expected = occupied.clone();
    expected.sort_unstable();
    listed.sort_unstable();
    assert_eq!(
        listed, expected,
        "**the index lists the vehicle on exactly the squares it stands on, once each**"
    );
}

#[test]
fn a_square_answers_which_vehicle_stands_on_it() {
    let (mut world, vehicle) = world_with_the_helicopter();
    // `spawn` records the vehicle as it builds it, so a vehicle made at run time is visible to a square query without a rebuild.
    assert_eq!(
        cdda_sim::vehicles::at(&world, (10, 10, 0)),
        Some(vehicle),
        "the origin square holds the frame"
    );
    assert_eq!(
        cdda_sim::vehicles::at(&world, (5, 10, 0)),
        Some(vehicle),
        "the tail stands at mount (-5, 0)"
    );
    assert_eq!(
        cdda_sim::vehicles::at(&world, (12, 10, 0)),
        None,
        "a square past the tail answers nothing"
    );

    // Something else on a square is not a vehicle: the answer filters by what the thing is, not by being on the square.
    let beside = world
        .spawn((
            cdda_sim::Position { x: 12, y: 10, z: 0 },
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
        ))
        .id();
    cdda_sim::singletons::index_mut(&mut world).insert(beside, (12, 10, 0));
    assert_eq!(
        cdda_sim::vehicles::at(&world, (12, 10, 0)),
        None,
        "**an item is not a vehicle**, even where one stands beside a vehicle"
    );

    let report = cdda_sim::inspect::what_is_at(&world, (10, 10, 0));
    let Some((_, names)) = report.iter().find(|(entity, _)| *entity == vehicle) else {
        panic!("the vehicle is on its origin square");
    };
    assert!(
        names.iter().any(|name| name.starts_with("Vehicle")),
        "the square report names the vehicle instead of a question mark: {names:?}"
    );
}
