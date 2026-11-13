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

#[test]
fn a_part_is_installed_only_where_the_structure_allows() {
    use cdda_sim::vehicles::{install_part, Vehicle};

    let (mut world, vehicle) = world_with_the_helicopter();

    // A frame on an empty square beside an existing frame is a valid mount.
    assert_eq!(
        install_part(&mut world, vehicle, "frame", 2, 1),
        Some(71),
        "the new part is appended after the blueprint's 71"
    );
    assert_eq!(
        cdda_sim::vehicles::at(&world, (12, 11, 0)),
        Some(vehicle),
        "the square the new part stands on answers the vehicle"
    );

    // A seat shares the square with the frame: a different id in a different location.
    assert_eq!(
        install_part(&mut world, vehicle, "reclining_seat", 2, 1),
        Some(72),
        "a seat may sit on a frame"
    );

    // A part type does not stack with itself.
    assert_eq!(
        install_part(&mut world, vehicle, "frame", 2, 1),
        None,
        "**a part type does not stack with itself**"
    );
    // A different id in the same location as the frame is refused.
    assert_eq!(
        install_part(&mut world, vehicle, "xlframe", 2, 1),
        None,
        "**the structure location is already taken**"
    );
    // A box is a different location from the frame, but the seat already holds `center`.
    assert_eq!(
        install_part(&mut world, vehicle, "box", 2, 1),
        None,
        "the center location is already taken by the seat"
    );
    // A headlight has no location, and an empty square's first part must be structural.
    assert_eq!(
        install_part(&mut world, vehicle, "headlight", 2, 0),
        None,
        "**a non-structural part cannot start a square**"
    );
    // A structure with nothing structural on or beside it.
    assert_eq!(
        install_part(&mut world, vehicle, "frame", 5, 5),
        None,
        "**a part must be on or beside existing structure**"
    );
    // A part the store does not hold.
    assert_eq!(
        install_part(&mut world, vehicle, "not_a_part", 2, 1),
        None,
        "a part the store does not hold is not installed"
    );
    // Power-transfer parts are the one exception to the no-stacking rule.
    assert_eq!(
        install_part(&mut world, vehicle, "jumper_cable", 2, 1),
        Some(73),
        "a cable may join a square"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "jumper_cable", 2, 1),
        Some(74),
        "**a power-transfer part may stack with itself**"
    );
    assert_eq!(
        world.get::<Vehicle>(vehicle).map(|built| built.parts.len()),
        Some(75),
        "75 = the blueprint's 71, the frame, the seat, two cables"
    );
}

#[test]
fn a_part_comes_out_and_its_square_gives_the_vehicle_up() {
    use cdda_sim::vehicles::{at, install_part, parts_at, remove_part, Vehicle};

    let (mut world, vehicle) = world_with_the_helicopter();
    assert_eq!(install_part(&mut world, vehicle, "frame", 2, 1), Some(71));
    assert_eq!(at(&world, (12, 11, 0)), Some(vehicle));

    let removed = remove_part(&mut world, vehicle, 71);
    assert_eq!(
        removed.as_ref().map(|part| (part.part.as_str(), part.x, part.y)),
        Some(("frame", 2, 1)),
        "the removed part comes back, mount and all"
    );
    assert_eq!(
        world.get::<Vehicle>(vehicle).map(|built| built.parts.len()),
        Some(71),
        "and it is no longer in the vehicle"
    );
    assert_eq!(
        at(&world, (12, 11, 0)),
        None,
        "**a square gives the vehicle up when its last part comes out**"
    );

    // A square with other parts keeps the vehicle in the index.
    let headlight = parts_at(&world, vehicle, 1, 1)
        .into_iter()
        .find(|index| {
            world
                .get::<Vehicle>(vehicle)
                .and_then(|built| built.parts.get(*index))
                .is_some_and(|part| part.part == "headlight")
        })
        .expect("the (1, 1) square holds a headlight");
    remove_part(&mut world, vehicle, headlight).expect("the headlight comes out");
    assert_eq!(
        at(&world, (11, 11, 0)),
        Some(vehicle),
        "**the square keeps the vehicle while the frame and the windshield stand there**"
    );
    for _ in 0..2 {
        let remaining = parts_at(&world, vehicle, 1, 1);
        let last = remaining.first().copied().expect("two parts stand at (1, 1)");
        remove_part(&mut world, vehicle, last).expect("the part comes out");
    }
    assert_eq!(
        at(&world, (11, 11, 0)),
        None,
        "**and gives it up when the last of them comes out**"
    );

    let seatbelt = parts_at(&world, vehicle, 0, 0)
        .into_iter()
        .find(|index| {
            world
                .get::<Vehicle>(vehicle)
                .and_then(|built| built.parts.get(*index))
                .is_some_and(|part| part.part == "seatbelt")
        })
        .expect("the origin square holds a seatbelt");
    let removed = remove_part(&mut world, vehicle, seatbelt).expect("the seatbelt comes out");
    assert_eq!(removed.part, "seatbelt");
    assert_eq!(
        at(&world, (10, 10, 0)),
        Some(vehicle),
        "**the origin still holds the frame, the seat and the controls**"
    );
    assert_eq!(
        remove_part(&mut world, vehicle, 999),
        None,
        "an index no part has answers nothing"
    );
}

#[test]
fn a_tow_cable_is_the_only_part_that_joins_a_harness_or_a_protrusion() {
    use cdda_sim::vehicles::{install_part, Vehicle};

    let (mut world, vehicle) = world_with_the_helicopter();
    // A harness on an empty square beside the hull: structural, so it is a valid first part.
    assert_eq!(
        install_part(&mut world, vehicle, "cart_handle", 2, -1),
        Some(71),
        "a cart handle is structural"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "headlight", 2, -1),
        None,
        "**a harness is in the way of anything but a tow cable**"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "hd_tow_cable", 2, -1),
        Some(72),
        "a tow cable is allowed on a harness"
    );

    // A protrusion behaves the same way.
    assert_eq!(
        install_part(&mut world, vehicle, "blade", 2, 2),
        Some(73),
        "a blade is structural and protrudes"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "headlight", 2, 2),
        None,
        "**a protrusion is in the way of anything but a tow cable**"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "hd_tow_cable", 2, 2),
        Some(74),
        "and a tow cable is allowed on a protrusion too"
    );
    assert_eq!(
        world.get::<Vehicle>(vehicle).map(|built| built.parts.len()),
        Some(75),
        "75 = the blueprint's 71, the harness, its cable, the blade, its cable"
    );
}

#[test]
fn cargo_and_opaque_parts_refuse_their_company() {
    use cdda_sim::vehicles::{install_part, Vehicle};

    let (mut world, vehicle) = world_with_the_helicopter();
    // A freezer is structural and cargo, so it can start a square.
    assert_eq!(
        install_part(&mut world, vehicle, "ap_apartment_freezer", 2, 1),
        Some(71),
        "an appliance with a structure location starts the square"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "door_internal", 2, 1),
        None,
        "**there cannot be two cargo parts on one square**"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "windshield", 2, 1),
        Some(72),
        "a part that is not cargo still joins the square"
    );

    // A board is opaque; a mirror needs to see.
    assert_eq!(install_part(&mut world, vehicle, "frame", 2, 2), Some(73));
    assert_eq!(install_part(&mut world, vehicle, "board", 2, 2), Some(74));
    assert_eq!(
        install_part(&mut world, vehicle, "inboard_mirror", 2, 2),
        None,
        "**a mirror cannot be mounted on an opaque part**"
    );
    // And the other way round: opaque parts cannot cover a mirror.
    assert_eq!(install_part(&mut world, vehicle, "frame", 2, 0), Some(75));
    assert_eq!(
        install_part(&mut world, vehicle, "inboard_mirror", 2, 0),
        Some(76),
        "a mirror joins a frame"
    );
    assert_eq!(
        install_part(&mut world, vehicle, "board", 2, 0),
        None,
        "**an opaque part cannot be mounted over a mirror**"
    );
    assert_eq!(
        world.get::<Vehicle>(vehicle).map(|built| built.parts.len()),
        Some(77),
        "77 = the blueprint's 71 and the six parts that were accepted"
    );
}

/// One recorded answer: the mount and the square it lands on.
type FacingSquare = ((i32, i32), (i32, i32));

/// The reference's own `coord_translate` answers at five facings, as `(degrees, [(mount, square)])`.
///
/// Written by `--dump-vehicle-rotation` (`oracle/src/src/harness.cpp`), which advances a `tileray` by each mount's x and offsets it by its y - the body of
/// `vehicle::coord_translate` (`vehicle.cpp:3598-3615`) - over a grid that covers every mount the helicopter has.
fn recorded_facings() -> Vec<(i32, Vec<FacingSquare>)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_rotation.json");
    let bytes = std::fs::read(path).expect("the facing recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(facings)) = root.get("facings") else {
        panic!("the recording has facings");
    };
    facings
        .iter()
        .map(|facing| {
            let cdda_json::Value::Object(entry) = facing else {
                panic!("a facing is an object");
            };
            let degrees = match entry.get("degrees") {
                Some(cdda_json::Value::Int(degrees)) => {
                    i32::try_from(*degrees).expect("a small facing")
                }
                _ => panic!("a facing has degrees"),
            };
            let Some(cdda_json::Value::Array(squares)) = entry.get("squares") else {
                panic!("a facing has squares");
            };
            let list = squares
                .iter()
                .map(|square| {
                    let cdda_json::Value::Object(square) = square else {
                        panic!("a square is an object");
                    };
                    (pair_of(square, "mount"), pair_of(square, "at"))
                })
                .collect();
            (degrees, list)
        })
        .collect()
}

fn pair_of(object: &cdda_json::Object, key: &str) -> (i32, i32) {
    let Some(cdda_json::Value::Array(values)) = object.get(key) else {
        panic!("{key} is an array");
    };
    let cdda_json::Value::Int(x) = values[0] else {
        panic!("{key}[0] is an integer");
    };
    let cdda_json::Value::Int(y) = values[1] else {
        panic!("{key}[1] is an integer");
    };
    (
        i32::try_from(x).expect("a small coordinate"),
        i32::try_from(y).expect("a small coordinate"),
    )
}

#[test]
fn a_facing_turns_a_mount_the_way_the_reference_turns_it() {
    use cdda_sim::rotation::coord_translate;

    let facings = recorded_facings();
    assert_eq!(facings.len(), 5, "the recording covers five facings");
    let mut compared = 0;
    for (degrees, squares) in &facings {
        assert_eq!(squares.len(), 77, "11 x 7 mounts at {degrees} degrees");
        for (mount, at) in squares {
            assert_eq!(
                coord_translate(f64::from(*degrees), (0, 0), *mount),
                *at,
                "**mount {mount:?} lands on {at:?} at {degrees} degrees**"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 385, "5 facings x 77 mounts, every pair compared");
}

#[test]
fn a_turned_vehicle_stands_on_the_turned_squares_and_the_index_lists_them() {
    use cdda_sim::vehicles::{occupied_squares, part_squares, spawn_facing, Vehicle};

    let facings = recorded_facings();
    let mut compared = 0;
    for (degrees, squares) in &facings {
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = spawn_facing(&mut world, "2seater2", (10, 10, 0), f64::from(*degrees))
            .expect("the helicopter builds at every facing");
        cdda_sim::PositionIndex::rebuild(&mut world);

        let built = world.get::<Vehicle>(vehicle).expect("a vehicle").clone();
        let recorded: std::collections::BTreeMap<(i32, i32), (i32, i32)> = squares
            .iter()
            .map(|(mount, at)| (*mount, *at))
            .collect();
        let port_squares = part_squares(&world, vehicle);
        assert_eq!(port_squares.len(), built.parts.len(), "one square per part");
        for (index, part) in built.parts.iter().enumerate() {
            let Some(wanted) = recorded.get(&(part.x, part.y)) else {
                panic!("the recording covers mount ({}, {})", part.x, part.y);
            };
            let Some(got) = port_squares.get(index) else {
                panic!("part {index} has a square");
            };
            assert_eq!(
                (got.0 - 10, got.1 - 10),
                *wanted,
                "**part {index} at mount ({}, {}) lands on its recorded square at {degrees} degrees**",
                part.x,
                part.y
            );
            compared += 1;
        }

        let mut listed = Vec::new();
        let index = cdda_sim::singletons::index(&world).expect("the index was rebuilt");
        for z in -1..=1 {
            for y in 0..=24 {
                for x in 0..=24 {
                    if index.at_level(x, y, z).contains(&vehicle) {
                        listed.push((x, y, z));
                    }
                }
            }
        }
        let mut expected: Vec<(i32, i32, i32)> = built
            .parts
            .iter()
            .map(|part| {
                let (x, y) = recorded
                    .get(&(part.x, part.y))
                    .expect("the recording covers every mount");
                (10 + x, 10 + y, 0)
            })
            .collect();
        expected.sort_unstable();
        expected.dedup();
        listed.sort_unstable();
        assert_eq!(
            listed, expected,
            "**the index lists exactly the squares the turned parts stand on**"
        );
        assert_eq!(
            occupied_squares(&world, vehicle).len(),
            expected.len(),
            "and the occupancy query agrees with the index"
        );
    }
    assert_eq!(compared, 355, "5 facings x 71 parts, every part compared");
}
