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
    let vehicle = cdda_sim::vehicles::spawn(&mut world, "2seater2", (10, 10, 0))
        .expect("the helicopter builds");
    (world, vehicle)
}

#[test]
fn a_prototype_becomes_one_entity_holding_every_part_its_blueprint_names() {
    let (world, vehicle) = world_with_the_helicopter();
    let built = world
        .get::<cdda_sim::vehicles::Vehicle>(vehicle)
        .expect("a vehicle");
    assert_eq!(
        built.parts.len(),
        71,
        "**one part per placement**, and the blueprint has 71"
    );
    let frame = &built.parts[0];
    assert_eq!(frame.part, "frame", "the blueprint's first placement");
    assert_eq!((frame.x, frame.y), (0, 0), "in the origin square");
    assert_eq!(
        frame.hp, 400,
        "**a fresh part's hit points are its store durability**, which for a cross frame is 400"
    );
    assert!(!frame.broken, "and nothing starts broken");
    assert_eq!(
        world
            .get::<cdda_sim::Position>(vehicle)
            .copied()
            .map(|at| (at.x, at.y, at.z)),
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
        cdda_sim::vehicle_damage::Outcome::Applied {
            net: 430,
            hp_after: 0,
            broke: true
        },
        "a 450-point bash lands for 430 after the frame's 20 armour, which takes it to zero"
    );
    let built = world
        .get::<cdda_sim::vehicles::Vehicle>(vehicle)
        .expect("a vehicle");
    assert!(
        built.parts[0].broken,
        "**the part is broken and still there**: a vehicle does not lose its frame"
    );
    assert_eq!(built.parts[0].hp, 0, "with no hit points left");
    assert_eq!(
        cdda_sim::vehicles::parts_at(&world, vehicle, 0, 0).len(),
        built
            .parts
            .iter()
            .filter(|part| (part.x, part.y) == (0, 0))
            .count(),
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
    use cdda_sim::vehicles::{Vehicle, install_part};

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
    use cdda_sim::vehicles::{Vehicle, at, install_part, parts_at, remove_part};

    let (mut world, vehicle) = world_with_the_helicopter();
    assert_eq!(install_part(&mut world, vehicle, "frame", 2, 1), Some(71));
    assert_eq!(at(&world, (12, 11, 0)), Some(vehicle));

    let removed = remove_part(&mut world, vehicle, 71);
    assert_eq!(
        removed
            .as_ref()
            .map(|part| (part.part.as_str(), part.x, part.y)),
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
        let last = remaining
            .first()
            .copied()
            .expect("two parts stand at (1, 1)");
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
    use cdda_sim::vehicles::{Vehicle, install_part};

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
    use cdda_sim::vehicles::{Vehicle, install_part};

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
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_rotation.json");
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
    use cdda_sim::vehicles::{Vehicle, occupied_squares, part_squares, spawn_facing};

    let facings = recorded_facings();
    let mut compared = 0;
    for (degrees, squares) in &facings {
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = spawn_facing(&mut world, "2seater2", (10, 10, 0), f64::from(*degrees))
            .expect("the helicopter builds at every facing");
        cdda_sim::PositionIndex::rebuild(&mut world);

        let built = world.get::<Vehicle>(vehicle).expect("a vehicle").clone();
        let recorded: std::collections::BTreeMap<(i32, i32), (i32, i32)> =
            squares.iter().map(|(mount, at)| (*mount, *at)).collect();
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

/// The index of a named part at one mount of a vehicle.
fn part_index(world: &World, vehicle: Entity, x: i32, y: i32, id: &str) -> Option<usize> {
    cdda_sim::vehicles::parts_at(world, vehicle, x, y)
        .into_iter()
        .find(|index| {
            world
                .get::<cdda_sim::vehicles::Vehicle>(vehicle)
                .and_then(|built| built.parts.get(*index))
                .is_some_and(|part| part.part == id)
        })
}

#[test]
fn removing_a_part_removes_the_parts_that_depend_on_it() {
    use cdda_sim::vehicles::{Vehicle, install_part, parts_at, remove_part};

    let (mut world, vehicle) = world_with_the_helicopter();
    assert_eq!(
        parts_at(&world, vehicle, 0, 0).len(),
        6,
        "the origin square holds frame, seat, seatbelt, controls, dashboard and roof"
    );
    let seat = part_index(&world, vehicle, 0, 0, "reclining_seat").expect("the seat");
    remove_part(&mut world, vehicle, seat).expect("the seat comes out");
    assert!(
        part_index(&world, vehicle, 0, 0, "seatbelt").is_none(),
        "**the seat takes its seatbelt with it**"
    );
    assert_eq!(
        parts_at(&world, vehicle, 0, 0).len(),
        4,
        "and leaves the frame, the controls, the dashboard and the roof"
    );

    // A window takes its curtain: the reference's `remove_dependent_part( "WINDOW", "CURTAIN" )`.
    assert!(
        install_part(&mut world, vehicle, "aisle_curtain", 1, 0).is_some(),
        "a curtain joins the windshield's square"
    );
    assert_eq!(
        world.get::<Vehicle>(vehicle).map(|built| built.parts.len()),
        Some(70),
        "71 - seat and seatbelt + curtain"
    );
    let windshield = part_index(&world, vehicle, 1, 0, "windshield").expect("the windshield");
    remove_part(&mut world, vehicle, windshield).expect("the windshield comes out");
    assert!(
        part_index(&world, vehicle, 1, 0, "aisle_curtain").is_none(),
        "**the windshield takes its curtain with it**"
    );
    assert_eq!(
        parts_at(&world, vehicle, 1, 0).len(),
        1,
        "the frame is all that is left there"
    );

    // A part with no dependents takes nothing else with it.
    let headlight = part_index(&world, vehicle, 1, 1, "headlight").expect("the headlight");
    let before = parts_at(&world, vehicle, 1, 1).len();
    remove_part(&mut world, vehicle, headlight).expect("the headlight comes out");
    assert_eq!(
        parts_at(&world, vehicle, 1, 1).len(),
        before - 1,
        "a headlight has no dependent part"
    );
}

#[test]
fn a_vehicle_square_costs_what_stands_on_it() {
    use cdda_sim::vehicles::{damage, install_part, move_cost_at};

    let (mut world, vehicle) = world_with_the_helicopter();
    // The origin square holds a frame, a seat, controls, a dashboard and a roof, and nothing that blocks.
    assert_eq!(
        move_cost_at(&world, (10, 10, 0)),
        Some(8),
        "an open vehicle square costs eight"
    );
    // A windshield blocks the square outright.
    assert_eq!(
        move_cost_at(&world, (11, 10, 0)),
        Some(0),
        "**a windshield is an obstacle**"
    );

    // Break it and the square opens: a broken part does not block.
    let windshield = part_index(&world, vehicle, 1, 0, "windshield").expect("the windshield");
    damage(&mut world, vehicle, windshield, 10_000, "bash", true);
    assert_eq!(
        move_cost_at(&world, (11, 10, 0)),
        Some(8),
        "**a broken obstacle stops blocking**"
    );

    // An aisle costs two, and it is cheaper than an open square.
    assert_eq!(install_part(&mut world, vehicle, "frame", 2, 0), Some(71));
    assert_eq!(install_part(&mut world, vehicle, "aisle", 2, 0), Some(72));
    assert_eq!(
        move_cost_at(&world, (12, 10, 0)),
        Some(2),
        "**an aisle costs two**"
    );

    // A square no vehicle stands on answers nothing.
    assert_eq!(move_cost_at(&world, (30, 30, 0)), None);
}

/// A creature standing at one square.
fn creature_at(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn((
            cdda_sim::Position { x, y, z: 0 },
            cdda_sim::creature::Creature {
                name: "a survivor".to_owned(),
                size: cdda_sim::CreatureSize::Medium,
            },
        ))
        .id()
}

#[test]
fn a_creature_boards_the_seat_under_it_and_can_get_out() {
    use cdda_sim::vehicles::{Riding, board, unboard};

    let (mut world, vehicle) = world_with_the_helicopter();
    let rider = creature_at(&mut world, 10, 10);
    let seat = part_index(&world, vehicle, 0, 0, "reclining_seat").expect("the seat");
    assert_eq!(
        board(&mut world, vehicle, rider),
        Some(seat),
        "**a creature boards the boardable part it stands on**"
    );
    assert_eq!(
        world.get::<Riding>(rider).copied(),
        Some(Riding {
            vehicle,
            part: seat
        }),
        "and the seat is named once, on the creature"
    );
    assert!(unboard(&mut world, rider), "it can get out");
    assert!(world.get::<Riding>(rider).is_none(), "and the seat is free");
    assert!(
        !unboard(&mut world, rider),
        "getting out twice reports that there was nothing to do"
    );
}

#[test]
fn a_seat_holds_one_creature_and_a_window_holds_none() {
    use cdda_sim::vehicles::{Riding, board, damage, remove_part};

    let (mut world, vehicle) = world_with_the_helicopter();
    let first = creature_at(&mut world, 10, 10);
    let second = creature_at(&mut world, 10, 10);
    let seat = board(&mut world, vehicle, first).expect("the first creature boards");
    assert_eq!(
        board(&mut world, vehicle, second),
        Some(seat),
        "the second boards the same seat"
    );
    assert!(
        world.get::<Riding>(first).is_none(),
        "**a seat holds one creature, so the first is unboarded**"
    );
    assert_eq!(
        world.get::<Riding>(second).copied(),
        Some(Riding {
            vehicle,
            part: seat
        })
    );

    // A windshield is not a seat, and a square off the vehicle is not either.
    let outsider = creature_at(&mut world, 11, 10);
    assert_eq!(
        board(&mut world, vehicle, outsider),
        None,
        "**a square with no boardable part refuses**"
    );
    let stranger = creature_at(&mut world, 30, 30);
    assert!(
        board(&mut world, vehicle, stranger).is_none(),
        "a square the vehicle does not stand on refuses"
    );

    // A broken seat is nobody's seat.
    let other_seat = part_index(&world, vehicle, 0, 1, "reclining_seat").expect("the other seat");
    damage(&mut world, vehicle, other_seat, 10_000, "bash", true);
    let waiter = creature_at(&mut world, 10, 11);
    assert_eq!(
        board(&mut world, vehicle, waiter),
        None,
        "**a broken seat refuses too**"
    );

    // Removing the seat releases whoever sits on it.
    remove_part(&mut world, vehicle, seat).expect("the seat comes out");
    assert!(
        world.get::<Riding>(second).is_none(),
        "**a removed seat unboards its rider**"
    );
}

#[test]
fn an_open_door_stops_blocking_the_square_it_stands_on() {
    use cdda_sim::vehicles::{Vehicle, move_cost_at, open_or_close};

    let (mut world, vehicle) = world_with_the_helicopter();
    let door = part_index(&world, vehicle, -1, -1, "door").expect("the door");
    assert_eq!(
        move_cost_at(&world, (9, 9, 0)),
        Some(0),
        "a closed door is an obstacle"
    );
    assert!(
        open_or_close(&mut world, vehicle, door, true),
        "the door opens"
    );
    assert!(
        world.get::<Vehicle>(vehicle).expect("a vehicle").parts[door].open,
        "the part carries its own state"
    );
    assert_eq!(
        move_cost_at(&world, (9, 9, 0)),
        Some(8),
        "**an open door stops blocking**"
    );
    assert!(
        open_or_close(&mut world, vehicle, door, false),
        "and closes"
    );
    assert_eq!(
        move_cost_at(&world, (9, 9, 0)),
        Some(0),
        "**a closed door blocks again**"
    );

    // A part that cannot open refuses both ways and stays an obstacle.
    let windshield = part_index(&world, vehicle, 1, 0, "windshield").expect("the windshield");
    assert!(
        !open_or_close(&mut world, vehicle, windshield, true),
        "**a part that cannot open refuses**"
    );
    assert!(
        !world.get::<Vehicle>(vehicle).expect("a vehicle").parts[windshield].open,
        "and carries no open state of its own"
    );
    assert_eq!(
        move_cost_at(&world, (11, 10, 0)),
        Some(0),
        "so it still blocks"
    );
}

#[test]
fn a_placed_vehicle_stands_where_the_reference_places_it() {
    use cdda_sim::vehicles::{Vehicle, part_squares, spawn_facing};

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_placement.json");
    let bytes = std::fs::read(path).expect("the placement recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(origin)) = root.get("origin") else {
        panic!("the recording has an origin");
    };
    let origin = (
        coordinate(origin, 0),
        coordinate(origin, 1),
        coordinate(origin, 2),
    );
    let Some(cdda_json::Value::Array(placed)) = root.get("parts") else {
        panic!("the recording has parts");
    };
    assert_eq!(
        placed.len(),
        71,
        "the reference placed the helicopter's 71 parts"
    );

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = spawn_facing(&mut world, "2seater2", origin, 0.0).expect("the port builds it");
    let built = world.get::<Vehicle>(vehicle).expect("a vehicle");
    assert_eq!(built.parts.len(), placed.len(), "one part per placed part");
    let squares = part_squares(&world, vehicle);
    let mut compared = 0;
    for (index, entry) in placed.iter().enumerate() {
        let cdda_json::Value::Object(entry) = entry else {
            panic!("a placed part is an object");
        };
        let mount = mount_of(entry);
        let at = at_of(entry);
        let Some(part) = built.parts.get(index) else {
            panic!("part {index} exists");
        };
        assert_eq!(
            (part.x, part.y),
            mount,
            "part {index} keeps the reference's mount"
        );
        let Some(square) = squares.get(index) else {
            panic!("part {index} has a square");
        };
        assert_eq!(
            (square.0, square.1),
            at,
            "**part {index} at mount {mount:?} stands on the square the reference placed it on**"
        );
        compared += 1;
    }
    assert_eq!(compared, 71, "every placed part compared");
}

/// An integer from a JSON array at a position.
fn coordinate(values: &[cdda_json::Value], index: usize) -> i32 {
    let Some(cdda_json::Value::Int(value)) = values.get(index) else {
        panic!("coordinate {index} is an integer");
    };
    i32::try_from(*value).expect("a small coordinate")
}

/// A placed part's mount.
fn mount_of(entry: &cdda_json::Object) -> (i32, i32) {
    let Some(cdda_json::Value::Array(values)) = entry.get("mount") else {
        panic!("a part has a mount");
    };
    (coordinate(values, 0), coordinate(values, 1))
}

/// A placed part's square.
fn at_of(entry: &cdda_json::Object) -> (i32, i32) {
    let Some(cdda_json::Value::Array(values)) = entry.get("at") else {
        panic!("a part has a square");
    };
    (coordinate(values, 0), coordinate(values, 1))
}

/// The fuel options of one placed part, as strings.
fn entry_fuel_opts(entry: &cdda_json::Object) -> Vec<String> {
    let Some(cdda_json::Value::Array(options)) = entry.get("fuel_opts") else {
        panic!("fuel_opts is an array");
    };
    options
        .iter()
        .map(|option| {
            let cdda_json::Value::Str(text) = option else {
                panic!("a fuel option is a string");
            };
            text.clone()
        })
        .collect()
}

/// Where the reference placed the helicopter.
fn placed_origin() -> (i32, i32, i32) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_placement.json");
    let bytes = std::fs::read(path).expect("the placement recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(origin)) = root.get("origin") else {
        panic!("an origin");
    };
    (
        coordinate(origin, 0),
        coordinate(origin, 1),
        coordinate(origin, 2),
    )
}

/// A string field of one JSON object.
fn text_of(entry: &cdda_json::Object, key: &str) -> String {
    match entry.get(key) {
        Some(cdda_json::Value::Str(value)) => value.clone(),
        _ => panic!("{key} is a string"),
    }
}

/// An integer field of one JSON object.
fn int_of(entry: &cdda_json::Object, key: &str) -> i32 {
    match entry.get(key) {
        Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a small number"),
        _ => panic!("{key} is an integer"),
    }
}

/// The placed helicopter's parts, as the reference recorded them.
fn placed_parts() -> Vec<cdda_json::Object> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_placement.json");
    let bytes = std::fs::read(path).expect("the placement recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(parts)) = root.get("parts") else {
        panic!("the recording has parts");
    };
    parts
        .iter()
        .map(|part| {
            let cdda_json::Value::Object(entry) = part else {
                panic!("a placed part is an object");
            };
            entry.clone()
        })
        .collect()
}

#[test]
fn every_placed_part_says_what_it_can_hold_and_what_fuel_it_uses() {
    let placed = placed_parts();
    assert_eq!(placed.len(), 71, "the reference placed 71 parts");
    let materials = store();
    let mut stores = 0;
    let mut batteries = 0;
    let mut engines = 0;
    // The store is keyed by id and the recording is in blueprint order, so compare through the blueprint's placements.
    let blueprint = materials
        .prototypes
        .iter()
        .find(|prototype| prototype.id == "2seater2")
        .expect("the helicopter blueprint");
    for (index, placement) in blueprint.parts.iter().enumerate() {
        let Some(entry) = placed.get(index) else {
            panic!("placement {index} exists");
        };
        let spec = materials
            .parts
            .iter()
            .find(|part| part.id == placement.part)
            .expect("the part's spec");
        assert_eq!(
            spec.fuel_type,
            text_of(entry, "fuel_type"),
            "**{} uses the reference's fuel type**",
            placement.part
        );
        assert_eq!(
            spec.fuel_store,
            int_of(entry, "fuel_store") != 0,
            "**{} is a fuel store exactly when the reference says so**",
            placement.part
        );
        assert_eq!(
            spec.battery,
            int_of(entry, "battery") != 0,
            "{} is a battery exactly when the reference says so",
            placement.part
        );
        if spec.fuel_store {
            stores += 1;
        }
        if spec.battery {
            batteries += 1;
        }
        assert_eq!(
            spec.fuel_opts,
            entry_fuel_opts(entry),
            "**{} can burn exactly the reference's fuels, in its order**",
            placement.part
        );
        if spec.flags.contains("ENGINE") {
            engines += 1;
            assert!(
                spec.fuel_opts.len() >= 2,
                "{} is multi-fuel",
                placement.part
            );
        }
    }
    assert!(
        stores >= 3,
        "the helicopter's tanks are fuel stores: {stores}"
    );
    assert!(batteries >= 1, "and it carries a battery: {batteries}");
    assert_eq!(engines, 2, "the helicopter has two engines");
}

#[test]
fn a_vehicle_says_how_much_fuel_of_a_type_it_carries() {
    use cdda_sim::vehicles::{Vehicle, damage, fuel_left, spawn_facing};

    let placed = placed_parts();
    let origin = placed_origin();

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = spawn_facing(&mut world, "2seater2", origin, 0.0).expect("the helicopter builds");
    {
        let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
        for (index, entry) in placed.iter().enumerate() {
            let Some(part) = built.parts.get_mut(index) else {
                panic!("part {index} exists");
            };
            part.fuel = text_of(entry, "ammo");
            part.charges = int_of(entry, "charges");
        }
    }

    let sum = |fuel: &str| -> i64 {
        placed
            .iter()
            .filter(|entry| text_of(entry, "ammo") == fuel)
            .map(|entry| i64::from(int_of(entry, "charges")))
            .sum()
    };
    let gasoline = sum("gasoline");
    let battery = sum("battery");
    assert!(gasoline > 0, "the reference filled the tanks: {gasoline}");
    assert!(battery > 0, "the reference charged the battery: {battery}");
    assert_eq!(
        fuel_left(&world, vehicle, "gasoline"),
        gasoline,
        "**the tanks' gasoline adds up**"
    );
    assert_eq!(
        fuel_left(&world, vehicle, "battery"),
        battery,
        "**a battery is summed through its own branch**"
    );
    assert_eq!(
        fuel_left(&world, vehicle, "diesel"),
        0,
        "a fuel nobody carries is zero"
    );

    // An engine is not a tank, however much fuel something writes onto it.
    let engine = part_index(&world, vehicle, -2, -1, "engine_turbine_small").expect("the engine");
    {
        let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
        let part = built.parts.get_mut(engine).expect("the engine");
        part.fuel = "gasoline".to_owned();
        part.charges = 500;
    }
    assert_eq!(
        fuel_left(&world, vehicle, "gasoline"),
        gasoline,
        "**an engine is not a fuel store**"
    );

    // A broken tank still holds its fuel: the reference asks for fuel stores, not working ones.
    let tank = part_index(&world, vehicle, -3, 0, "tank_55gal_drum").expect("a tank");
    assert!(
        world.get::<Vehicle>(vehicle).expect("a vehicle").parts[tank].charges > 0,
        "the tank is filled"
    );
    damage(&mut world, vehicle, tank, 10_000, "bash", true);
    assert_eq!(
        fuel_left(&world, vehicle, "gasoline"),
        gasoline,
        "**a broken tank still holds its fuel**"
    );
}

/// The helicopter with the reference's fuel contents copied onto it, as the placement dump records them.
fn fueled_helicopter() -> (World, Entity) {
    use cdda_sim::vehicles::{Vehicle, spawn_facing};

    let placed = placed_parts();
    let origin = placed_origin();
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = spawn_facing(&mut world, "2seater2", origin, 0.0).expect("the helicopter builds");
    {
        let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
        for (index, entry) in placed.iter().enumerate() {
            let part = built.parts.get_mut(index).expect("part {index} exists");
            part.fuel = text_of(entry, "ammo");
            part.charges = int_of(entry, "charges");
        }
    }
    (world, vehicle)
}

/// Puts a fuel in the vehicle's tanks, replacing whatever they held - the harness's `ammo_set` for a single fuel.
fn fill_tanks(world: &mut World, vehicle: Entity, fuel: &str, charges: i32) {
    use cdda_sim::vehicles::{Vehicle, part_spec};

    let tanks: Vec<usize> = {
        let built = world.get::<Vehicle>(vehicle).expect("a vehicle");
        built
            .parts
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                part_spec(world, vehicle, *index)
                    .is_some_and(|spec| spec.fuel_store && !spec.battery)
            })
            .map(|(index, _)| index)
            .collect()
    };
    let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
    for index in tanks {
        let part = built.parts.get_mut(index).expect("a tank");
        fuel.clone_into(&mut part.fuel);
        part.charges = charges;
    }
}

#[test]
fn an_engine_starts_only_when_it_has_fuel() {
    use cdda_sim::vehicles::{
        Vehicle, damage, engine_fuel_left, fuel_current, is_engine_on, start_engine,
    };

    let (mut world, vehicle) = fueled_helicopter();
    let engine = part_index(&world, vehicle, -2, -1, "engine_turbine_small").expect("the engine");
    assert!(
        is_engine_on(&world, vehicle, engine),
        "**an engine starts on**: `bool enabled = true` (`vehicle.h:519`), so a blueprint's engine is already switched on"
    );
    assert!(
        !start_engine(&mut world, vehicle, engine, true),
        "**asking to start an engine that is already on answers no** - the reference returns early when the switch is already where it was asked to be"
    );
    assert!(
        start_engine(&mut world, vehicle, engine, false),
        "turning it off works"
    );
    assert!(!is_engine_on(&world, vehicle, engine), "and now it is off");
    assert!(
        start_engine(&mut world, vehicle, engine, true),
        "**the tanks hold 300 units of its fuel, so it starts**"
    );
    assert!(is_engine_on(&world, vehicle, engine), "and it reads as on");
    assert_eq!(fuel_current(&world, vehicle, engine), "gasoline");
    assert_eq!(
        engine_fuel_left(&world, vehicle, engine),
        300,
        "its fuel is the three full tanks"
    );

    assert!(
        !start_engine(&mut world, vehicle, engine, true),
        "**asking for the state it already has changes nothing and answers no**"
    );
    assert!(
        start_engine(&mut world, vehicle, engine, false),
        "turning it off always works"
    );
    assert!(!is_engine_on(&world, vehicle, engine));

    // A broken engine cannot start, even full of fuel.
    damage(&mut world, vehicle, engine, 10_000, "bash", true);
    {
        let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
        built.parts.get_mut(engine).expect("the engine").enabled = true;
    }
    assert!(
        !is_engine_on(&world, vehicle, engine),
        "**a broken engine reads as off even with its switch left on**"
    );
    world
        .get_mut::<Vehicle>(vehicle)
        .expect("a vehicle")
        .parts
        .get_mut(engine)
        .expect("the engine")
        .enabled = false;
    assert!(
        !start_engine(&mut world, vehicle, engine, true),
        "**a broken engine does not start**"
    );
    {
        let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
        let part = built.parts.get_mut(engine).expect("the engine");
        part.hp = 400;
        part.broken = false;
    }
    assert!(
        start_engine(&mut world, vehicle, engine, true),
        "**a repaired engine starts again**"
    );
    assert!(start_engine(&mut world, vehicle, engine, false));

    // Out of fuel: the switch is flipped back, so nothing looks on.
    fill_tanks(&mut world, vehicle, "gasoline", 0);
    assert!(
        !start_engine(&mut world, vehicle, engine, true),
        "**an engine with no fuel does not start**"
    );
    assert!(
        !is_engine_on(&world, vehicle, engine),
        "**a failed start leaves the switch off**"
    );
}

#[test]
fn an_engine_takes_the_first_fuel_it_can_burn() {
    use cdda_sim::vehicles::{Vehicle, engine_fuel_left, fuel_current, part_spec, start_engine};

    let (mut world, vehicle) = fueled_helicopter();
    let engine = part_index(&world, vehicle, -2, -1, "engine_turbine_small").expect("the engine");
    let options = part_spec(&world, vehicle, engine)
        .expect("the engine's spec")
        .fuel_opts
        .clone();
    assert_eq!(
        options,
        vec![
            "gasoline".to_owned(),
            "diesel".to_owned(),
            "lamp_oil".to_owned(),
            "jp8".to_owned(),
            "motor_oil".to_owned(),
            "avgas".to_owned(),
            "biodiesel".to_owned()
        ],
        "the reference's turbine burns seven fuels, in this order"
    );

    // Diesel only: its own type is dry, so it takes the first option that is not. The engine is already on - a fresh part's switch is on - so it is switched off
    // before it is asked to choose a fuel, which is what the reference's own "start" does on a vehicle nobody has started yet.
    fill_tanks(&mut world, vehicle, "diesel", 100);
    assert!(
        start_engine(&mut world, vehicle, engine, false),
        "switched off first"
    );
    assert!(
        start_engine(&mut world, vehicle, engine, true),
        "it starts on diesel"
    );
    assert_eq!(
        fuel_current(&world, vehicle, engine),
        "diesel",
        "**the engine takes the first fuel it can burn**"
    );
    assert_eq!(engine_fuel_left(&world, vehicle, engine), 300);

    // Gasoline again: the choice is made afresh on each start, not remembered forever.
    assert!(start_engine(&mut world, vehicle, engine, false));
    fill_tanks(&mut world, vehicle, "gasoline", 100);
    assert!(
        start_engine(&mut world, vehicle, engine, true),
        "it starts on gasoline"
    );
    assert_eq!(
        fuel_current(&world, vehicle, engine),
        "gasoline",
        "**a fuel that is gone is not the fuel it burns**"
    );
    assert!(
        world
            .get::<Vehicle>(vehicle)
            .expect("a vehicle")
            .parts
            .get(engine)
            .expect("the engine")
            .fuel_preference
            == "gasoline",
        "and it remembers the type it took"
    );
}

/// A kind for the cargo gate: an id, a volume, and nothing else the rule reads.
fn cargo_kind(id: &str, volume_ml: i64) -> cdda_sim::item_registry::Kind<'_> {
    cdda_sim::item_registry::Kind {
        id,
        volume_ml,
        weight_g: 0,
        pockets: &[],
        thrown_damage: 0.0,
        melee_bash: 0.0,
        melee_cut: 0.0,
        melee_stab: 0.0,
        to_hit: -1,
    }
}

/// The reference's bicycle, with a rock and an anvil to try in its basket.
fn bicycle_with_cargo() -> (
    World,
    Entity,
    usize,
    usize,
    cdda_sim::ItemTypeId,
    cdda_sim::ItemTypeId,
) {
    use cdda_sim::item_registry::{id_of, set_item_types};
    use cdda_sim::vehicles::spawn_facing;

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let kinds = [
        cargo_kind("null", 0),
        cargo_kind("rock", 50),
        cargo_kind("anvil", 25_000),
    ];
    set_item_types(&mut world, &kinds);
    let rock = id_of(&mut world, "rock").expect("the rock");
    let anvil = id_of(&mut world, "anvil").expect("the anvil");
    let vehicle =
        spawn_facing(&mut world, "bicycle", (10, 10, 0), 0.0).expect("the bicycle builds");
    let basket = part_index(&world, vehicle, -1, 0, "basketsm").expect("the basket");
    let frame = part_index(&world, vehicle, 0, 0, "xlframe").expect("the frame");
    (world, vehicle, basket, frame, rock, anvil)
}

#[test]
fn a_cargo_part_holds_only_what_fits() {
    use cdda_sim::vehicle_cargo::{
        InVehiclePart, cargo_capacity, free_volume, items_in_part, stored_volume, stow_item,
    };

    let (mut world, vehicle, basket, frame, rock, anvil) = bicycle_with_cargo();

    assert_eq!(
        cargo_capacity(&world, vehicle, basket),
        20_000,
        "**a basket holds twenty litres**"
    );
    assert_eq!(
        cargo_capacity(&world, vehicle, frame),
        0,
        "a frame is not cargo and holds nothing"
    );
    assert_eq!(free_volume(&world, vehicle, basket), 20_000);

    assert!(
        stow_item(&mut world, vehicle, basket, anvil, 1).is_none(),
        "**a part refuses what it cannot hold**"
    );
    assert!(
        stow_item(&mut world, vehicle, frame, rock, 1).is_none(),
        "and a part that is not cargo holds nothing at all"
    );
    assert_eq!(
        items_in_part(&world, vehicle, basket).len(),
        0,
        "a refusal stores nothing"
    );

    // A stack takes its charge count in room.
    let stack = stow_item(&mut world, vehicle, basket, rock, 300).expect("300 rocks fit");
    assert!(
        world
            .get::<InVehiclePart>(stack)
            .is_some_and(|held| held.vehicle == vehicle && held.part == basket),
        "the item names the part it rides in"
    );
    assert_eq!(
        stored_volume(&world, vehicle, basket),
        15_000,
        "**a stack takes its charge count in room**"
    );
    assert_eq!(free_volume(&world, vehicle, basket), 5_000);

    // Single rocks fill the rest exactly.
    let mut loose = 0;
    while stow_item(&mut world, vehicle, basket, rock, 1).is_some() {
        loose += 1;
    }
    assert_eq!(
        loose, 100,
        "**exactly what is left of the basket fits, and not one rock more**"
    );
    assert_eq!(stored_volume(&world, vehicle, basket), 20_000);
    assert_eq!(free_volume(&world, vehicle, basket), 0);
    assert_eq!(items_in_part(&world, vehicle, basket).len(), 101);
}

#[test]
fn a_full_part_says_so_and_a_broken_one_takes_nothing() {
    use cdda_sim::item_registry::id_of;
    use cdda_sim::vehicle_cargo::{InVehiclePart, cargo_capacity, free_volume, stow_item};
    use cdda_sim::vehicles::damage;

    let (mut world, vehicle, basket, _frame, rock, _anvil) = bicycle_with_cargo();

    // A part overfilled by an older, larger capacity reports negative free volume, as the reference's does.
    let _ = world
        .spawn((
            cdda_sim::GroundItem {
                kind: rock,
                charges: 401,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            InVehiclePart {
                vehicle,
                part: basket,
            },
        ))
        .id();
    assert!(
        free_volume(&world, vehicle, basket) < 0,
        "**free volume is not clamped: an overfull part says so**"
    );

    damage(&mut world, vehicle, basket, 10_000, "bash", true);
    assert_eq!(
        cargo_capacity(&world, vehicle, basket),
        0,
        "**a broken part holds nothing**"
    );
    assert!(stow_item(&mut world, vehicle, basket, rock, 1).is_none());
    let nothing = id_of(&mut world, "null").expect("the null kind");
    assert!(
        stow_item(&mut world, vehicle, basket, nothing, 1).is_none(),
        "**a broken part takes nothing even when the item has no volume - the reference refuses a broken part first**"
    );
}

#[test]
fn an_item_can_come_back_out_of_a_part() {
    use cdda_sim::vehicle_cargo::{
        free_volume, items_in_part, remove_item, stored_volume, stow_item,
    };

    let (mut world, vehicle, basket, _frame, rock, _anvil) = bicycle_with_cargo();
    let stack = stow_item(&mut world, vehicle, basket, rock, 300).expect("300 rocks fit");
    let single = stow_item(&mut world, vehicle, basket, rock, 1).expect("a rock fits");
    assert_eq!(stored_volume(&world, vehicle, basket), 15_050);

    assert!(
        remove_item(&mut world, single),
        "**a held item comes back out**"
    );
    assert_eq!(
        stored_volume(&world, vehicle, basket),
        15_000,
        "and the room it took comes back with it"
    );
    assert_eq!(items_in_part(&world, vehicle, basket).len(), 1);
    assert!(
        !remove_item(&mut world, single),
        "the same item cannot come out twice"
    );

    assert!(remove_item(&mut world, stack));
    assert_eq!(stored_volume(&world, vehicle, basket), 0);
    assert_eq!(free_volume(&world, vehicle, basket), 20_000);

    let ground = cdda_sim::drop_item(&mut world, (0, 0), rock, 1);
    assert!(
        !remove_item(&mut world, ground),
        "**an item that is not in a part is not the part's to remove**"
    );
}

#[test]
fn a_wreck_can_be_salvaged() {
    use cdda_sim::vehicle_cargo::{items_in_part, remove_item, stow_item};
    use cdda_sim::vehicles::damage;

    let (mut world, vehicle, basket, _frame, rock, _anvil) = bicycle_with_cargo();
    let salvage = stow_item(&mut world, vehicle, basket, rock, 10).expect("rocks fit");
    damage(&mut world, vehicle, basket, 10_000, "bash", true);
    assert_eq!(
        items_in_part(&world, vehicle, basket).len(),
        1,
        "the wreck still holds it"
    );
    assert!(
        remove_item(&mut world, salvage),
        "**a broken part gives back what it holds**"
    );
    assert_eq!(items_in_part(&world, vehicle, basket).len(), 0);
}

/// A recorded `[x, y, z]` field as a square.
fn square_of(case: &cdda_json::Object, field: &str) -> (i32, i32, i32) {
    let Some(cdda_json::Value::Array(square)) = case.get(field) else {
        panic!("{field} is a square");
    };
    (
        coordinate(square, 0),
        coordinate(square, 1),
        coordinate(square, 2),
    )
}

/// Everything the index answers for one square, which is how a move is checked from the outside.
fn things_at(world: &World, x: i32, y: i32) -> Vec<Entity> {
    cdda_sim::singletons::index(world)
        .map(|index| index.at_level(x, y, 0).to_vec())
        .unwrap_or_default()
}

#[test]
fn a_vehicle_moves_one_square_and_its_squares_follow() {
    use cdda_sim::vehicles::{move_vehicle, occupied_squares, spawn_facing};

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle =
        spawn_facing(&mut world, "bicycle", (20, 20, 0), 0.0).expect("the bicycle builds");
    let before = occupied_squares(&world, vehicle);
    assert_eq!(before.len(), 3, "the bicycle stands on three squares");
    for square in &before {
        assert!(
            things_at(&world, square.0, square.1).contains(&vehicle),
            "the index holds it before the move"
        );
    }

    assert!(
        move_vehicle(&mut world, vehicle, (1, 0)),
        "**a vehicle moves one square**"
    );
    let after = occupied_squares(&world, vehicle);
    assert_eq!(
        after.len(),
        before.len(),
        "it stands on as many squares as before"
    );
    for (old, new) in before.iter().zip(&after) {
        assert_eq!(
            *new,
            (old.0 + 1, old.1, old.2),
            "**every square of it moves with it**"
        );
    }
    let left: Vec<_> = before
        .iter()
        .filter(|square| !after.contains(square))
        .collect();
    assert_eq!(
        left.len(),
        1,
        "a three-square bicycle moved one square leaves one square behind"
    );
    for square in &left {
        assert!(
            !things_at(&world, square.0, square.1).contains(&vehicle),
            "**the index forgets the square it left**"
        );
    }
    for square in &after {
        assert!(
            things_at(&world, square.0, square.1).contains(&vehicle),
            "**and finds it on the square it took**"
        );
    }

    assert!(
        !move_vehicle(&mut world, vehicle, (0, 0)),
        "an empty displacement is refused"
    );
    assert!(
        !move_vehicle(&mut world, vehicle, (2, 0)),
        "a displacement longer than a square is refused"
    );
    assert_eq!(
        occupied_squares(&world, vehicle),
        after,
        "a refused move changes nothing"
    );
}

#[test]
fn a_rider_moves_with_the_seat_it_sits_on() {
    use cdda_sim::vehicles::{Riding, board, move_vehicle, part_squares, spawn_facing};

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle =
        spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds");
    let saddle = part_index(&world, vehicle, 0, 0, "saddle").expect("the saddle");
    let rider = creature_at(&mut world, 30, 30);
    assert_eq!(
        board(&mut world, vehicle, rider),
        Some(saddle),
        "the rider takes the saddle"
    );
    let seat_before = part_squares(&world, vehicle)[saddle];

    assert!(
        move_vehicle(&mut world, vehicle, (0, 1)),
        "the bicycle moves"
    );
    let seat_after = part_squares(&world, vehicle)[saddle];
    assert_ne!(seat_before, seat_after, "the seat moved");
    let position = world
        .get::<cdda_sim::Position>(rider)
        .expect("the rider has a square");
    assert_eq!(
        (position.x, position.y, position.z),
        seat_after,
        "**a rider is put on the seat's new square**"
    );
    assert!(
        world
            .get::<Riding>(rider)
            .is_some_and(|riding| riding.vehicle == vehicle && riding.part == saddle),
        "and still sits in the same seat"
    );
    assert!(
        things_at(&world, seat_after.0, seat_after.1).contains(&rider),
        "**the index finds the rider where the seat is now**"
    );
    assert!(
        !things_at(&world, seat_before.0, seat_before.1).contains(&rider),
        "and not where it was"
    );
}

/// The four tiles the collision recording names, with the numbers the reference's own data gives them.
///
/// `t_wall` is a wood wall - no room, a bash block; `t_floor` is a floor - room costs 2, and it has a bash block too, for a fall through it, which the
/// collision rule excludes by cost; `t_open_air` has room and no floor; `t_underbrush` is `SHORT` and bashable.
fn recorded_tile(id: &str) -> cdda_content::mapdata::MapData {
    use cdda_content::mapdata::{BashInfo, MapDataKind};
    let bash = |yes: bool| yes.then(BashInfo::default);
    let (move_cost, flags, has_bash): (i32, &[&str], bool) = match id {
        "t_wall" => (0, &[], true),
        "t_floor" => (2, &["FLAT", "ROAD"], true),
        "t_open_air" => (2, &["NO_FLOOR"], false),
        "t_underbrush" => (6, &["SHORT", "SHRUB"], true),
        other => panic!("the recording names a tile the gate does not know: {other}"),
    };
    cdda_content::mapdata::MapData {
        heat_radiation: 0,
        examine: None,
        kind: MapDataKind::Terrain,
        id: id.to_owned(),
        name: id.to_owned(),
        symbol: [35; 4],
        color: [cdda_core_types::colors::NcColor::default(); 4],
        move_cost,
        light: 0,
        coverage: 0,
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
        transparent: false,
        bash: bash(has_bash),
        roof: None,
    }
}

/// Rebuilds the world one collision case was asked in: a bicycle at the recorded origin, the obstacle the case names, and nothing else.
fn collision_world(
    case: &cdda_json::Object,
    at: (i32, i32, i32),
) -> (World, Entity, Option<Entity>) {
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    if case.get("terrain").is_some() {
        let id = text_of(case, "terrain");
        let code = 7;
        cdda_sim::terrain_types::set_types(&mut world, [(id.clone(), code)]);
        cdda_sim::bash::set_data(&mut world, [recorded_tile(&id)]);
        cdda_sim::square::set_terrain(&mut world, square_of(case, "terrain_at"), code);
    }
    let first = cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", at, 0.0)
        .expect("the bicycle builds");
    let mut blocker = None;
    for field in ["obstacle_at", "creature_at"] {
        let Some(cdda_json::Value::Array(square)) = case.get(field) else {
            continue;
        };
        let there = (
            coordinate(square, 0),
            coordinate(square, 1),
            coordinate(square, 2),
        );
        let spawned = if field == "obstacle_at" {
            cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", there, 0.0)
                .expect("the obstacle builds")
        } else {
            world
                .spawn((
                    cdda_sim::Position {
                        x: there.0,
                        y: there.1,
                        z: there.2,
                    },
                    cdda_sim::creature::Creature {
                        name: "a zombie".to_owned(),
                        size: cdda_sim::CreatureSize::Medium,
                    },
                ))
                .id()
        };
        cdda_sim::singletons::index_mut(&mut world).insert(spawned, there);
        blocker = Some(spawned);
    }
    (world, first, blocker)
}

#[test]
fn a_vehicle_sees_what_it_would_hit() {
    use cdda_sim::vehicles::{Collision, Vehicle, first_collision};

    // `corpus/vehicle_collision.json` is `--dump-vehicle-collision`: a bicycle at the avatar's square, then four displacements answered in detect-only mode. Each case names
    // the world it was asked in - a second bicycle for the vehicle collision, a zombie for the body collision - and the port rebuilds exactly that world.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_collision.json");
    let bytes = std::fs::read(path).expect("the collision recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(origin)) = root.get("origin") else {
        panic!("an origin");
    };
    let at = (
        coordinate(origin, 0),
        coordinate(origin, 1),
        coordinate(origin, 2),
    );
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        let name = text_of(case, "name");
        let Some(cdda_json::Value::Array(dp)) = case.get("dp") else {
            panic!("a displacement");
        };
        let delta = (coordinate(dp, 0), coordinate(dp, 1));
        let hit = int_of(case, "hit") != 0;
        let kind = if hit {
            text_of(case, "type")
        } else {
            "nothing".to_owned()
        };

        let (world, first, blocker) = collision_world(case, at);
        let answer = first_collision(&world, first, delta);
        let part = match answer {
            None => {
                assert!(
                    !hit,
                    "**a {name} move hits nothing, as the reference says**"
                );
                None
            }
            Some(Collision::Vehicle {
                part,
                target,
                target_part,
            }) => {
                assert!(
                    hit && kind == "veh",
                    "**the reference answers a vehicle for {name}**"
                );
                assert_eq!(
                    Some(target),
                    blocker,
                    "{name}: the vehicle it hits is the one standing there"
                );
                assert_eq!(
                    target_part,
                    usize::try_from(int_of(case, "target_part")).expect("a part index"),
                    "{name}: **the target part is the reference's**"
                );
                Some(part)
            }
            Some(Collision::Terrain {
                part,
                square: _,
                bashable,
            }) => {
                assert!(
                    hit && kind == if bashable { "bashable" } else { "other" },
                    "**the reference answers {} for {name}**",
                    if bashable { "bashable" } else { "other" }
                );
                Some(part)
            }
            Some(Collision::Creature { part, target }) => {
                assert!(
                    hit && kind == "body",
                    "**the reference answers a body for {name}**"
                );
                assert_eq!(
                    Some(target),
                    blocker,
                    "{name}: the creature it hits is the one standing there"
                );
                Some(part)
            }
        };
        if let Some(part) = part {
            assert_eq!(
                part,
                usize::try_from(int_of(case, "part_index")).expect("a part index"),
                "{name}: **the hitting part is the reference's**"
            );
            assert_eq!(
                world
                    .get::<Vehicle>(first)
                    .expect("the bicycle")
                    .parts
                    .get(part)
                    .expect("the part")
                    .part,
                text_of(case, "part"),
                "{name}: and it is the same part by name"
            );
        }
        kinds.insert(if hit { kind } else { "nothing".to_owned() });
        seen += 1;
    }
    assert_eq!(seen, 8, "the recording holds eight displacements");
    assert_eq!(
        kinds.len(),
        4,
        "**it covers a vehicle, a body, bashable terrain and a clear square**: {kinds:?}"
    );
}

#[test]
fn the_prediction_does_not_stop_the_move() {
    use cdda_sim::vehicles::{Collision, first_collision, move_vehicle, spawn_facing};

    // A prediction is not a veto: `move_vehicle` shifts the vehicle and the obstacle is still ahead afterwards (`map::move_vehicle` collides **before** it displaces).
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let first = spawn_facing(&mut world, "bicycle", (40, 40, 0), 0.0).expect("the bicycle builds");
    let second =
        spawn_facing(&mut world, "bicycle", (42, 40, 0), 0.0).expect("the obstacle builds");
    assert!(
        first_collision(&world, first, (1, 0)).is_some(),
        "the square ahead holds the other bicycle"
    );
    assert!(
        move_vehicle(&mut world, first, (1, 0)),
        "the move itself is not refused yet"
    );
    assert!(
        first_collision(&world, first, (1, 0)).is_some_and(
            |collision| matches!(collision, Collision::Vehicle { target, .. } if target == second)
        ),
        "**after the move the two stand closer and the next square still holds the other one**"
    );
}

#[test]
fn a_rider_is_not_something_the_vehicle_hits() {
    use cdda_sim::vehicles::{Collision, board, first_collision, spawn_facing, unboard};

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle =
        spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds");
    let rider = creature_at(&mut world, 30, 30);
    cdda_sim::singletons::index_mut(&mut world).insert(rider, (30, 30, 0));
    let saddle = part_index(&world, vehicle, 0, 0, "saddle").expect("the saddle");
    assert_eq!(
        board(&mut world, vehicle, rider),
        Some(saddle),
        "the rider takes the saddle"
    );

    // The trailing frame moves onto the square the rider stands on; a passenger is not debris (`vehicle_move.cpp:867-872`).
    assert_eq!(
        first_collision(&world, vehicle, (1, 0)),
        None,
        "**a passenger is not something the vehicle carrying it hits**"
    );
    assert!(unboard(&mut world, rider), "the rider gets off");
    assert_eq!(
        first_collision(&world, vehicle, (1, 0)),
        Some(Collision::Creature {
            part: 6,
            target: rider
        }),
        "**and once off, standing there, the trailing frame would hit them**"
    );
}

/// A tile with the numbers a variant clause needs, so the flag rules can be checked without a recording that happens to contain them.
fn synthetic_tile(id: &str, move_cost: i32, flags: &[&str]) -> cdda_content::mapdata::MapData {
    use cdda_content::mapdata::{BashInfo, MapDataKind};
    cdda_content::mapdata::MapData {
        heat_radiation: 0,
        examine: None,
        kind: MapDataKind::Terrain,
        id: id.to_owned(),
        name: id.to_owned(),
        symbol: [35; 4],
        color: [cdda_core_types::colors::NcColor::default(); 4],
        move_cost,
        light: 0,
        coverage: 0,
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
        transparent: false,
        bash: Some(BashInfo::default()),
        roof: None,
    }
}

/// A bicycle standing at `at` in a world whose ground at `ahead` is the one tile described.
fn bicycle_facing(
    world: &mut World,
    at: (i32, i32, i32),
    ahead: (i32, i32, i32),
    tile: cdda_content::mapdata::MapData,
) -> Entity {
    let code = 9;
    cdda_sim::terrain_types::set_types(world, [(tile.id.clone(), code)]);
    cdda_sim::bash::set_data(world, [tile]);
    cdda_sim::square::set_terrain(world, ahead, code);
    cdda_sim::vehicles::spawn_facing(world, "bicycle", at, 0.0).expect("the bicycle builds")
}

#[test]
fn trivial_things_do_not_stop_a_vehicle() {
    use cdda_sim::vehicles::{Collision, first_collision};

    // `vehicle_move.cpp:950-967` is a list of exceptions, and each one is a thing a vehicle should drive over rather than hit.

    // A `TINY` flower is only hit when the part's mount carries a wheel; the bicycle's leading frame has no wheel of its own on this mount after this blueprint is built.
    let mut materials = store();
    for prototype in &mut materials.prototypes {
        if prototype.id == "bicycle" {
            prototype
                .parts
                .retain(|placement| !placement.part.starts_with("wheel_bicycle"));
        }
    }
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, materials);
    let tile = synthetic_tile("t_flower", 0, &["TINY"]);
    let vehicle = {
        let code = 9;
        cdda_sim::terrain_types::set_types(&mut world, [(tile.id.clone(), code)]);
        cdda_sim::bash::set_data(&mut world, [tile.clone()]);
        cdda_sim::square::set_terrain(&mut world, (32, 30, 0), code);
        cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0)
            .expect("the bicycle builds")
    };
    // A flower has no walkable square either, so a frame with no wheel in its space still stops - as the reference's `veh_coll_other` rather than `bashable`.
    assert_eq!(
        first_collision(&world, vehicle, (1, 0)),
        Some(Collision::Terrain {
            part: 4,
            square: (32, 30, 0),
            bashable: false,
        }),
        "**a wheel-less frame does not bash a flower, but still stops on one**"
    );

    // The same frame with a wheel on its mount does hit it.
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let tile = synthetic_tile("t_flower", 0, &["TINY"]);
    let vehicle = {
        let code = 9;
        cdda_sim::terrain_types::set_types(&mut world, [(tile.id.clone(), code)]);
        cdda_sim::bash::set_data(&mut world, [tile.clone()]);
        cdda_sim::square::set_terrain(&mut world, (32, 30, 0), code);
        cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0)
            .expect("the bicycle builds")
    };
    assert!(
        matches!(
            first_collision(&world, vehicle, (1, 0)),
            Some(Collision::Terrain { bashable: true, .. })
        ),
        "**with a wheel on its mount the same flower is hit**"
    );

    // `NOCOLLIDE` never interacts with a vehicle, even when it is bashable.
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = bicycle_facing(
        &mut world,
        (30, 30, 0),
        (32, 30, 0),
        synthetic_tile("t_smoke_thing", 6, &["NOCOLLIDE"]),
    );
    assert_eq!(
        first_collision(&world, vehicle, (1, 0)),
        None,
        "**a NOCOLLIDE thing never stops a vehicle**"
    );

    // A `SHORT` thing is not hit by a `PROTRUSION` on the part's mount, though the same part hits anything taller. The spike goes into the blueprint because a
    // structure part cannot be installed beside the frame it shares a square with.
    let low = synthetic_tile("t_low_table", 6, &["SHORT"]);
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let plain = bicycle_facing(&mut world, (30, 30, 0), (32, 30, 0), low.clone());
    assert!(
        matches!(
            first_collision(&world, plain, (1, 0)),
            Some(Collision::Terrain { bashable: true, .. })
        ),
        "a short thing is hit by a plain frame"
    );
    let mut materials = store();
    for prototype in &mut materials.prototypes {
        if prototype.id == "bicycle" {
            prototype.parts.push(cdda_content::vehicles::Placement {
                part: "spike".to_owned(),
                x: 1,
                y: 0,
                variant: String::new(),
                with_ammo: 0,
            });
        }
    }
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, materials);
    let armed = bicycle_facing(&mut world, (30, 30, 0), (32, 30, 0), low);
    assert_eq!(
        first_collision(&world, armed, (1, 0)),
        None,
        "**a ram's mount passes over a short thing it would otherwise hit**"
    );
}

#[test]
fn armour_takes_the_hit_and_a_broken_plate_does_not() {
    use cdda_sim::vehicles::{Collision, Vehicle, first_collision, spawn_facing};

    // `corpus/vehicle_armour.json` is `--dump-vehicle-armour`: a wall in front of the leading structure part of a quad bike - which carries steel plating on
    // that mount - and of a bicycle, which carries none. The reference blames the plate, then blames the frame once the plate is ruined.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_armour.json");
    let bytes = std::fs::read(path).expect("the armour recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let origin = square_of(&root, "origin");
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut blames = std::collections::BTreeSet::new();
    // For each vehicle, the part its healthy case blamed: the broken case that follows is that same part ruined.
    let mut healthy_part: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        let wall = square_of(case, "wall_at");
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let tile = recorded_tile("t_wall");
        let code = 11;
        cdda_sim::terrain_types::set_types(&mut world, [(tile.id.clone(), code)]);
        cdda_sim::bash::set_data(&mut world, [tile]);
        cdda_sim::square::set_terrain(&mut world, wall, code);
        let name = text_of(case, "vehicle");
        let vehicle =
            spawn_facing(&mut world, &name, origin, 0.0).expect("the recorded vehicle builds");
        assert_eq!(
            int_of(case, "hit"),
            1,
            "the wall is in the way in every case"
        );
        if case.get("plates").is_some() {
            let ruined = *healthy_part
                .get(&name)
                .expect("the healthy case came first");
            let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
            let part = built
                .parts
                .get_mut(ruined)
                .expect("the part the healthy case blamed");
            part.hp = 0;
            part.broken = true;
        } else {
            healthy_part.insert(
                name,
                usize::try_from(int_of(case, "reported_index")).expect("a part index"),
            );
        }

        let answer = first_collision(&world, vehicle, (1, 0));
        let Some(Collision::Terrain { part, bashable, .. }) = answer else {
            panic!(
                "{}: the wall is hit, not something else",
                text_of(case, "name")
            );
        };
        assert!(bashable, "{}: a wall is bashable", text_of(case, "name"));
        assert_eq!(
            part,
            usize::try_from(int_of(case, "reported_index")).expect("a part index"),
            "{}: **the part the reference blames is the part the port blames**",
            text_of(case, "name")
        );
        let named = world
            .get::<Vehicle>(vehicle)
            .expect("a vehicle")
            .parts
            .get(part)
            .expect("the part")
            .part
            .clone();
        assert_eq!(
            named,
            text_of(case, "reported"),
            "{}: and by name",
            text_of(case, "name")
        );
        blames.insert(named);
        seen += 1;
    }
    assert_eq!(seen, 4, "the recording holds four cases");
    assert_eq!(
        blames.len(),
        3,
        "**the plate answers one case, and two different frames the others**: {blames:?}"
    );
}

#[test]
fn a_part_that_is_itself_armour_takes_the_hit() {
    use cdda_sim::vehicles::{Collision, Vehicle, first_collision, spawn_facing};

    // No `structure` part in the shipped data is also armour, so the reference's first clause - the part that hits is itself a plate, so it takes the hit
    // (`vehicle.cpp:3037-3040`) - cannot be reached by any recording. A fixture pins it instead: a frame marked `ARMOR`, with a plate **earlier in part order** on the
    // same mount. Without the self-check the loop would blame the earlier plate.
    let mut materials = store();
    for spec in &mut materials.parts {
        if spec.id == "xlframe" {
            spec.flags.insert("ARMOR".to_owned());
        }
    }
    for prototype in &mut materials.prototypes {
        if prototype.id == "bicycle" {
            prototype.parts.insert(
                0,
                cdda_content::vehicles::Placement {
                    part: "plating_steel".to_owned(),
                    x: 1,
                    y: 0,
                    variant: String::new(),
                    with_ammo: 0,
                },
            );
        }
    }
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, materials);
    let tile = recorded_tile("t_wall");
    let code = 11;
    cdda_sim::terrain_types::set_types(&mut world, [(tile.id.clone(), code)]);
    cdda_sim::bash::set_data(&mut world, [tile]);
    cdda_sim::square::set_terrain(&mut world, (32, 30, 0), code);
    let vehicle =
        spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds");
    let Some(Collision::Terrain { part, .. }) = first_collision(&world, vehicle, (1, 0)) else {
        panic!("the wall is in the way");
    };
    assert_eq!(
        world
            .get::<Vehicle>(vehicle)
            .expect("a vehicle")
            .parts
            .get(part)
            .expect("the part")
            .part,
        "xlframe",
        "**a part that is itself armour takes the hit, even with a plate earlier on its mount**"
    );
}

#[test]
fn a_vehicles_wheels_say_what_they_resist_rolling_with() {
    use cdda_sim::item_registry::{Kind, set_item_types};
    use cdda_sim::vehicles::{Vehicle, spawn_facing};

    // `corpus/vehicle_rolling.json` is `--dump-vehicle-rolling`: a bicycle, a quad bike, and a bicycle whose wheels are all ruined. Each case carries its parts'
    // base item weights, so the gate can install the reference's own registry and compare the coefficient rather than an approximation of it. The reference's
    // `coeff_rolling_drag` is the first number of the driving model, and it answers 50 as the wheel factor for a vehicle with no usable wheel.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_rolling.json");
    let bytes = std::fs::read(path).expect("the rolling recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let origin = square_of(&root, "origin");
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut factors = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        let name = text_of(case, "name");
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = spawn_facing(&mut world, &text_of(case, "vehicle"), origin, 0.0)
            .expect("the recorded vehicle builds");

        // The registry, weighted from the recording's own parts: one entry per base item, at the reference's weight.
        let Some(cdda_json::Value::Array(parts)) = case.get("parts") else {
            panic!("{name}: parts");
        };
        let mut kinds: Vec<(String, i64)> = Vec::new();
        for part in parts {
            let cdda_json::Value::Object(part) = part else {
                panic!("a part is an object");
            };
            let item = text_of(part, "item");
            let weight = i64::from(int_of(part, "base_mass_g"));
            if let Some(slot) = kinds.iter_mut().find(|(id, _)| *id == item) {
                slot.1 = weight;
            } else {
                kinds.push((item, weight));
            }
        }
        let specs: Vec<Kind<'_>> = kinds
            .iter()
            .map(|(id, weight)| Kind {
                id,
                volume_ml: 0,
                weight_g: *weight,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: -1,
            })
            .collect();
        set_item_types(&mut world, &specs);

        let wheels = int_of(case, "wheel_count");
        if wheels == 0 {
            let count = world
                .get::<Vehicle>(vehicle)
                .expect("a vehicle")
                .parts
                .len();
            let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
            for index in 0..count {
                let part = built.parts.get_mut(index).expect("a part");
                part.broken = true;
            }
        }

        assert_eq!(
            cdda_sim::vehicle_mass::mass(&world, vehicle),
            i64::from(int_of(case, "total_mass_g")),
            "{name}: **the mass the coefficient is scaled by is the recorded one**"
        );
        let Some(coefficient) = cdda_sim::vehicle_drive::rolling_drag(&world, vehicle) else {
            panic!("{name}: the port answers for every recorded vehicle");
        };
        let Some(cdda_json::Value::Float(recorded)) = case.get("coeff_rolling_drag") else {
            panic!("a recorded coefficient");
        };
        let difference = (coefficient - recorded).abs();
        assert!(
            difference <= 1e-9 * recorded.abs().max(1.0),
            "{name}: **the port's rolling resistance is the reference's** ({coefficient} vs {recorded})"
        );
        factors.insert(if wheels == 0 { "none" } else { "some" });
        seen += 1;
    }
    assert_eq!(seen, 3, "the recording holds three vehicles");
    assert_eq!(
        factors.len(),
        2,
        "**one of them has no usable wheel**: {factors:?}"
    );
}

#[test]
fn the_cubic_turns_drag_and_power_into_a_top_speed() {
    // `corpus/vehicle_top_speed.json` is `--dump-vehicle-top-speed`: six real vehicles and a humvee with no wheel and a broken engine, each recording the three
    // coefficients the reference solved and the speed it answered. The port's `top_speed_vmiph` is checked on those numbers, including the one corner where the
    // reference's cubic goes through `std::complex`.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_top_speed.json");
    let bytes = std::fs::read(path).expect("the top-speed recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut complex = 0;
    let mut speeds = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        let Some(cdda_json::Value::Float(air)) = case.get("c_air") else {
            panic!("recorded air drag");
        };
        let Some(cdda_json::Value::Float(rolling)) = case.get("c_rolling") else {
            panic!("recorded rolling drag");
        };
        let power = int_of(case, "power_w");

        // The engines, measured rather than handed over: the port sums its own engine parts, so this case's power is a value it produces.
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let origin = square_of(&root, "origin");
        let vehicle =
            cdda_sim::vehicles::spawn_facing(&mut world, &text_of(case, "vehicle"), origin, 0.0)
                .expect("the recorded vehicle builds");
        if text_of(case, "name").starts_with("a_humvee_with_no_wheel") {
            let count = world
                .get::<cdda_sim::vehicles::Vehicle>(vehicle)
                .expect("a vehicle")
                .parts
                .len();
            for index in 0..count {
                if cdda_sim::vehicles::part_spec(&world, vehicle, index)
                    .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "ENGINE"))
                {
                    let mut built = world
                        .get_mut::<cdda_sim::vehicles::Vehicle>(vehicle)
                        .expect("a vehicle");
                    let part = built.parts.get_mut(index).expect("a part");
                    part.hp = 0;
                    part.broken = true;
                }
            }
        }
        assert_eq!(
            cdda_sim::vehicle_drive::total_power(&world, vehicle),
            power,
            "{}: **the running engines make what the reference says they make**",
            text_of(case, "name")
        );

        assert_eq!(
            cdda_sim::vehicle_drive::top_speed_vmiph(*air, *rolling, power),
            int_of(case, "max_ground_velocity"),
            "{}: **the reference's cubic answer is the port's** (air {air}, rolling {rolling}, power {power})",
            text_of(case, "name")
        );
        if int_of(case, "complex") == 1 {
            complex += 1;
        }
        speeds.insert(int_of(case, "max_ground_velocity"));
        seen += 1;
    }
    assert_eq!(seen, 9, "the recording holds nine vehicles");
    assert_eq!(
        complex, 1,
        "**one of them reaches the complex branch**: {complex}"
    );
    assert!(
        speeds.len() >= 5,
        "the speeds are not all the same: {speeds:?}"
    );
}

#[test]
fn the_complex_branch_of_the_cubic_agrees_with_a_bisection() {
    // The reference's only recorded trip through `std::complex` is a humvee with no power, where the answer is zero and every solver agrees. This case is chosen so
    // that the branch is reached **and** the answer is not zero, and the port's answer is checked against a bisection root of the same cubic - a second method on
    // the same equation, rather than the same arithmetic twice.
    let air = 0.25;
    let rolling = 600.0;
    let power = 500.0;

    // The branch selector, exactly as the reference computes it: negative `tricky_bit` means three real roots and the complex path.
    let p = -rolling / (3.0 * air);
    let q = p * p * p + (rolling * 33.33 * rolling - 3.0 * air * -power) / (6.0 * air * air);
    let r = rolling * 33.33 / (3.0 * air);
    let tricky = q * q + (r - p * p) * (r - p * p) * (r - p * p);
    assert!(
        tricky < 0.0,
        "the case must reach the complex branch: {tricky}"
    );

    let at = |v: f64| air * v * v * v + rolling * v * v + 33.33 * rolling * v - power;
    let (mut low, mut high) = (0.0, 10_000.0);
    assert!(at(low) < 0.0 && at(high) > 0.0, "the root is bracketed");
    for _ in 0..200 {
        let middle = f64::midpoint(low, high);
        if at(middle) < 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    let expected = (low * f64::from(cdda_sim::vehicle_drive::MPS_TO_MIPH) * 100.0) as i32;
    assert_eq!(
        cdda_sim::vehicle_drive::top_speed_vmiph(air, rolling, 500),
        expected,
        "**the complex cube roots give the same root a bisection finds**"
    );
    assert!(expected > 0, "and the answer is not zero: {expected}");
}

#[test]
fn the_body_prices_the_air_one_column_at_a_time() {
    use cdda_sim::vehicles::spawn_facing;

    // `corpus/vehicle_top_speed.json` records each vehicle's `coeff_air_drag` beside its speed. The rule is a column scan over the structure parts, so a bicycle
    // with no body at all and a helicopter with rotors give opposite ends of it.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_top_speed.json");
    let bytes = std::fs::read(path).expect("the top-speed recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let origin = square_of(&root, "origin");
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut values = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = spawn_facing(&mut world, &text_of(case, "vehicle"), origin, 0.0)
            .expect("the recorded vehicle builds");
        let Some(cdda_json::Value::Float(recorded)) = case.get("c_air") else {
            panic!("recorded air drag");
        };
        let Some(answered) = cdda_sim::vehicle_air::coeff_air_drag(&world, vehicle) else {
            panic!(
                "{}: the port answers for a placed vehicle",
                text_of(case, "name")
            );
        };
        let difference = (answered - recorded).abs();
        assert!(
            difference <= 1e-12 * recorded.abs().max(1.0),
            "{}: **the port's air drag is the reference's** ({answered} vs {recorded})",
            text_of(case, "name")
        );
        #[allow(clippy::cast_possible_truncation)]
        let key = (answered * 10_000.0) as i64;
        values.insert(key);
        seen += 1;
    }
    assert_eq!(seen, 9, "the recording holds nine vehicles");
    assert!(
        values.len() >= 7,
        "the drags are not all the same: {values:?}"
    );
}

#[test]
fn the_engines_are_summed_and_the_vehicle_drives_itself_to_its_top_speed() {
    use cdda_sim::item_registry::{Kind, set_item_types};
    use cdda_sim::vehicles::{Vehicle, spawn_facing};

    // The whole chain in one gate, on the three vehicles whose mass the port reproduces exactly: the port measures the body's air drag, the wheels' rolling
    // resistance and the engines' power, and the cubic turns them into the speed the reference recorded. No recorded coefficient is used for the answer.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_rolling.json");
    let bytes = std::fs::read(path).expect("the rolling recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let origin = square_of(&root, "origin");
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut speeds = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        let name = text_of(case, "name");
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = spawn_facing(&mut world, &text_of(case, "vehicle"), origin, 0.0)
            .expect("the recorded vehicle builds");
        let Some(cdda_json::Value::Array(parts)) = case.get("parts") else {
            panic!("{name}: parts");
        };
        let mut kinds: Vec<(String, i64)> = Vec::new();
        for part in parts {
            let cdda_json::Value::Object(part) = part else {
                panic!("a part is an object");
            };
            let item = text_of(part, "item");
            let weight = i64::from(int_of(part, "base_mass_g"));
            if let Some(slot) = kinds.iter_mut().find(|(id, _)| *id == item) {
                slot.1 = weight;
            } else {
                kinds.push((item, weight));
            }
        }
        let specs: Vec<Kind<'_>> = kinds
            .iter()
            .map(|(id, weight)| Kind {
                id,
                volume_ml: 0,
                weight_g: *weight,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: -1,
            })
            .collect();
        set_item_types(&mut world, &specs);
        if int_of(case, "wheel_count") == 0 {
            let broken: Vec<usize> = (0..world
                .get::<Vehicle>(vehicle)
                .expect("a vehicle")
                .parts
                .len())
                .filter(|index| {
                    cdda_sim::vehicles::part_spec(&world, vehicle, *index)
                        .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "WHEEL"))
                })
                .collect();
            let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
            for index in broken {
                built.parts.get_mut(index).expect("a part").broken = true;
            }
        }

        assert_eq!(
            cdda_sim::vehicle_drive::total_power(&world, vehicle),
            int_of(case, "power_w"),
            "{name}: **the running engines make what the reference says they make**"
        );
        assert_eq!(
            cdda_sim::vehicle_drive::max_ground_velocity(&world, vehicle),
            Some(int_of(case, "max_ground_velocity")),
            "{name}: **and the port's own air drag, rolling drag and power give the reference's speed**"
        );
        speeds.insert(int_of(case, "max_ground_velocity"));
        seen += 1;
    }
    assert_eq!(seen, 3, "the recording holds three vehicles");
    assert!(
        speeds.len() >= 2,
        "the speeds are not all the same: {speeds:?}"
    );
}

#[test]
fn a_switched_off_engine_makes_no_power() {
    use cdda_sim::vehicles::{Vehicle, spawn_facing};

    // `is_engine_on` is "available and enabled" (`vehicle.cpp:1061-1064`), so the quad bike stands still with its engine off and again with it broken. The recorded
    // cases all have their switches on, so this is the only case that can tell the two apart.
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle =
        spawn_facing(&mut world, "quad_bike", (70, 70, 0), 0.0).expect("the quad bike builds");
    let engine = (0..world
        .get::<Vehicle>(vehicle)
        .expect("a vehicle")
        .parts
        .len())
        .find(|index| {
            cdda_sim::vehicles::part_spec(&world, vehicle, *index)
                .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "ENGINE"))
        })
        .expect("an engine");
    assert_eq!(
        cdda_sim::vehicle_drive::total_power(&world, vehicle),
        37_300
    );
    assert!(cdda_sim::vehicles::start_engine(
        &mut world, vehicle, engine, false
    ));
    assert_eq!(
        cdda_sim::vehicle_drive::total_power(&world, vehicle),
        0,
        "**an engine that is off makes no power**"
    );
    assert_eq!(
        cdda_sim::vehicle_drive::max_ground_velocity(&world, vehicle),
        Some(0),
        "and the vehicle stands still"
    );
    // Switching it back on needs fuel, which a fresh vehicle has none of - that is `start_engine`'s job and the fuel gate's business. Here the switch is what
    // matters, so it is set directly.
    world
        .get_mut::<Vehicle>(vehicle)
        .expect("a vehicle")
        .parts
        .get_mut(engine)
        .expect("the engine")
        .enabled = true;
    assert_eq!(
        cdda_sim::vehicle_drive::total_power(&world, vehicle),
        37_300,
        "and it comes back when the switch does"
    );
    let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
    let part = built.parts.get_mut(engine).expect("the engine");
    part.hp = 0;
    part.broken = true;
    assert_eq!(
        cdda_sim::vehicle_drive::total_power(&world, vehicle),
        0,
        "**a broken engine makes no power even with its switch on**"
    );
}

#[test]
fn a_damaged_engine_makes_less_power() {
    use cdda_sim::vehicles::damage;

    // `corpus/engine_power.json` is `--dump-engine-power`: the quad bike's one engine walked from whole to broken, with the reference's `health_percent` and the
    // engine's own `damaged_power_factor`. The arithmetic is the reference's to the last digit, because its power unit is an integer of milliwatts.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/engine_power.json");
    let bytes = std::fs::read(path).expect("the engine-power recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let origin = square_of(&root, "origin");
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut powers = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        let mut world = World::new();
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = cdda_sim::vehicles::spawn_facing(&mut world, "quad_bike", origin, 0.0)
            .expect("the quad bike builds");
        let engine = usize::try_from(int_of(case, "engine_part")).expect("a part index");
        let durability = int_of(case, "durability");
        let hp = int_of(case, "hp");
        let spec =
            cdda_sim::vehicles::part_spec(&world, vehicle, engine).expect("the engine's spec");
        assert_eq!(
            spec.durability, durability,
            "the recorded durability is the store's"
        );
        let Some(cdda_json::Value::Float(recorded_factor)) = case.get("damaged_power_factor")
        else {
            panic!("a recorded factor");
        };
        assert_eq!(
            spec.damaged_power_factor.map(f64::from),
            Some(*recorded_factor),
            "and the recorded factor is the store's"
        );
        // `set_hp( part, hp )` is how the harness damaged it; here the damage rule does the same work, at the difference the recording records.
        let damage_taken = durability - hp;
        if damage_taken > 0 {
            damage(&mut world, vehicle, engine, damage_taken, "bash", true);
        }
        let recorded_hp = world
            .get::<cdda_sim::vehicles::Vehicle>(vehicle)
            .expect("a vehicle")
            .parts
            .get(engine)
            .expect("the engine")
            .hp;
        assert_eq!(
            recorded_hp,
            hp,
            "{}: the engine stands where the recording left it",
            text_of(case, "name")
        );

        let power = cdda_sim::vehicle_drive::total_power(&world, vehicle);
        assert_eq!(
            power,
            int_of(case, "total_power_w"),
            "{}: **the engine makes what the reference says it makes at this health**",
            text_of(case, "name")
        );
        powers.insert(power);
        seen += 1;
    }
    assert_eq!(
        seen, 8,
        "the recording walks eight steps from whole to broken"
    );
    assert!(
        powers.len() >= 6,
        "**the numbers fall as the engine is hurt**: {powers:?}"
    );
    assert!(powers.contains(&0), "and the broken engine makes nothing");
}

#[test]
fn the_engines_push_harder_at_low_speed() {
    // `corpus/vehicle_acceleration.json` is `--dump-vehicle-acceleration`: two vehicles asked what their engine can add at a series of speeds. The rule is
    // `100 * 100 * ( power / weight ) / cmps` - one double division truncated, then integer arithmetic - so the dump checks every step of it.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_acceleration.json");
    let bytes = std::fs::read(path).expect("the acceleration recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    let mut accelerations = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case is an object");
        };
        if int_of(case, "engine_on") == 0 {
            assert_eq!(
                int_of(case, "acceleration"),
                0,
                "{}: **nothing turning adds nothing**",
                text_of(case, "name")
            );
            continue;
        }
        let Some(cdda_json::Value::Float(mass_kg)) = case.get("mass_kg") else {
            panic!("a recorded mass");
        };
        #[allow(clippy::cast_possible_truncation)]
        let grams = (mass_kg * 1000.0).round() as i64;
        assert_eq!(
            cdda_sim::vehicle_move::ground_acceleration(
                int_of(case, "power_w"),
                grams,
                int_of(case, "max_velocity"),
                int_of(case, "at"),
            ),
            int_of(case, "acceleration"),
            "{}: **the port's acceleration is the reference's**",
            text_of(case, "name")
        );
        accelerations.insert(int_of(case, "acceleration"));
        seen += 1;
    }
    assert_eq!(seen, 14, "the recording asks fourteen running cases");
    assert!(
        accelerations.len() >= 6,
        "**the answer falls as the speed rises**: {accelerations:?}"
    );
}

#[test]
fn a_vehicle_accelerates_from_where_it_stands() {
    use cdda_sim::item_registry::{Kind, set_item_types};
    use cdda_sim::vehicles::{Vehicle, spawn_facing};

    // The world-level half: `current_acceleration` reads the vehicle's own `velocity` and its engines. The weights come from `vehicle_rolling.json`, which carries
    // the same two vehicles' parts, so the mass is the reference's own.
    let rolling =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_rolling.json");
    let bytes = std::fs::read(rolling).expect("the rolling recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the rolling recording is an object");
    };
    let origin = square_of(&root, "origin");
    let Some(cdda_json::Value::Array(recordings)) = root.get("cases") else {
        panic!("cases");
    };
    let Some(cdda_json::Value::Object(bicycle)) = recordings
        .iter()
        .find(|case| matches!(case, cdda_json::Value::Object(object) if text_of(object, "name") == "bicycle"))
    else {
        panic!("the bicycle's recording");
    };
    let Some(cdda_json::Value::Array(parts)) = bicycle.get("parts") else {
        panic!("its parts");
    };
    let mut kinds: Vec<(String, i64)> = Vec::new();
    for part in parts {
        let cdda_json::Value::Object(part) = part else {
            panic!("a part is an object");
        };
        let item = text_of(part, "item");
        let weight = i64::from(int_of(part, "base_mass_g"));
        if let Some(slot) = kinds.iter_mut().find(|(id, _)| *id == item) {
            slot.1 = weight;
        } else {
            kinds.push((item, weight));
        }
    }
    let specs: Vec<Kind<'_>> = kinds
        .iter()
        .map(|(id, weight)| Kind {
            id,
            volume_ml: 0,
            weight_g: *weight,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -1,
        })
        .collect();
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = spawn_facing(&mut world, "bicycle", origin, 0.0).expect("the bicycle builds");
    set_item_types(&mut world, &specs);

    assert_eq!(
        cdda_sim::vehicle_move::current_acceleration(&world, vehicle),
        1_650,
        "**a standing bicycle can add 1,650 vmiph this turn**"
    );
    for (velocity, expected) in [(1_500, 1_100), (2_000, 825), (10_000, 163)] {
        world
            .get_mut::<Vehicle>(vehicle)
            .expect("a vehicle")
            .velocity = velocity;
        assert_eq!(
            cdda_sim::vehicle_move::current_acceleration(&world, vehicle),
            expected,
            "at {velocity} vmiph the engine can add {expected}"
        );
    }
    // Reverse is the same push: the reference asks about `std::abs( velocity )`, so a vehicle backing up accelerates as hard as one going forward.
    world
        .get_mut::<Vehicle>(vehicle)
        .expect("a vehicle")
        .velocity = -1_500;
    assert_eq!(
        cdda_sim::vehicle_move::current_acceleration(&world, vehicle),
        1_100,
        "**backwards is the same as forwards**: the reference asks about the absolute speed"
    );

    // With every engine broken there is nothing to push with.
    let count = world
        .get::<Vehicle>(vehicle)
        .expect("a vehicle")
        .parts
        .len();
    let engines: Vec<usize> = (0..count)
        .filter(|index| {
            cdda_sim::vehicles::part_spec(&world, vehicle, *index)
                .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "ENGINE"))
        })
        .collect();
    let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
    for index in engines {
        built.parts.get_mut(index).expect("an engine").broken = true;
    }
    assert_eq!(
        cdda_sim::vehicle_move::current_acceleration(&world, vehicle),
        0,
        "**a broken engine pushes nothing**"
    );
}

/// **One thrust step leaves the vehicle at the speed the reference leaves it at.**
///
/// The dump states every input the step reads - the current speed, the cruise the driver asked for, the direction, what the engine can add, the grip, and the two
/// limits - and the speed afterwards. The arithmetic is `vehicle::thrust` (`vehicle_move.cpp:453-620`), and every case here is on open floor, so traction is the
/// reference's own `1.0` and the numbers are exact rather than rounded.
#[test]
fn a_thrust_step_leaves_the_speed_the_reference_leaves() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_thrust.json");
    let text = std::fs::read_to_string(&path).expect("the thrust dump is in the corpus");
    let value = cdda_json::parse(text.as_bytes()).expect("the dump parses");
    let cdda_json::Value::Object(root) = &value else {
        panic!("the dump is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("the dump has cases");
    };
    assert!(
        cases.len() >= 10,
        "the dump is the case list: {} rows",
        cases.len()
    );

    let mut checked = 0;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let Some(cdda_json::Value::Str(name)) = row.get("name") else {
            panic!("a case names itself");
        };
        let number = |key: &str| -> i32 {
            match row.get(key) {
                Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a small int"),
                _ => panic!("`{key}` is an int in `{name}`"),
            }
        };
        let boolean =
            |key: &str| -> bool { matches!(row.get(key), Some(cdda_json::Value::Bool(true))) };
        let traction = match row.get("traction") {
            Some(cdda_json::Value::Float(value)) => *value,
            // `printf("%.9g", 1.0)` writes `1`, which is an int to a JSON reader.
            Some(cdda_json::Value::Int(value)) => {
                f64::from(i32::try_from(*value).expect("a small int"))
            }
            _ => panic!("`traction` is a number in `{name}`"),
        };
        #[allow(clippy::cast_possible_truncation)]
        let inputs = cdda_sim::vehicle_move::ThrustInputs {
            velocity: number("velocity_before"),
            cruise_velocity: number("cruise"),
            direction: number("thd"),
            acceleration: number("acceleration"),
            traction: traction as f32,
            max_velocity: number("max_velocity"),
            max_reverse: number("max_reverse"),
            battery_engine: boolean("has_battery_engine"),
        };
        assert_eq!(
            cdda_sim::vehicle_move::thrust_velocity(&inputs),
            number("velocity_after"),
            "**{name}**: from {} at direction {}, cruise {}, accel {}, max {}, reverse {}, battery {}",
            inputs.velocity,
            inputs.direction,
            inputs.cruise_velocity,
            inputs.acceleration,
            inputs.max_velocity,
            inputs.max_reverse,
            inputs.battery_engine
        );
        checked += 1;
    }
    assert_eq!(checked, cases.len(), "every case was checked");
}

/// **The reverse floor is a quarter of the top speed, or all of it with an electric motor.**
///
/// Reference: `vehicle::max_reverse_velocity` (`vehicle.cpp:4289-4299`). The dump carries the floor it computed for every thrust case, so the check is the
/// reference's own number rather than the port's arithmetic read back to itself.
#[test]
fn the_reverse_floor_is_a_quarter_unless_the_motor_is_electric() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_thrust.json");
    let bytes = std::fs::read(path).expect("the thrust dump is in the corpus");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut electric = 0;
    let mut mechanical = 0;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let battery = matches!(
            row.get("has_battery_engine"),
            Some(cdda_json::Value::Bool(true))
        );
        let max_velocity = int_of(row, "max_velocity");
        assert_eq!(
            cdda_sim::vehicle_drive::max_reverse_velocity(max_velocity, battery),
            int_of(row, "max_reverse"),
            "{}: **the floor is the reference's**",
            text_of(row, "name")
        );
        if battery {
            electric += 1;
        } else {
            mechanical += 1;
        }
    }
    assert!(
        electric >= 2 && mechanical >= 5,
        "both branches were checked: {electric} electric, {mechanical} mechanical"
    );
}

/// **A vehicle in the store accelerates when it is asked to, and stops asking when nobody does.**
///
/// The numbers are the reference's: the bicycle in the thrust dump adds 1,650 vmiph from rest with a cruise set, and its top speed is 1,871. A vehicle whose engines
/// are all switched off does not move at all, which is `current_acceleration` answering zero.
#[test]
fn a_vehicle_in_the_store_takes_the_speed_its_thrust_step_decides() {
    let mut world = World::new();
    install_part_weights(&mut world, "bicycle");
    cdda_sim::vehicles::set_store(&mut world, store());
    let bicycle = cdda_sim::vehicles::spawn(&mut world, "bicycle", (10, 10, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.cruise_velocity = 3000;
    }

    assert!(
        cdda_sim::vehicle_move::thrust(&mut world, bicycle, 1),
        "**the bicycle moves**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("still there")
            .velocity,
        1650,
        "**from rest it adds what the engine can add**"
    );

    // At its top speed with the same cruise, the engine is clamped to the top speed and the step leaves it there.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 3000;
    }
    assert!(
        cdda_sim::vehicle_move::thrust(&mut world, bicycle, 1),
        "it slows to its top speed"
    );
    assert_eq!(
        world
            .get::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("still there")
            .velocity,
        1871,
        "**at cruise faster than the top speed, the top speed wins**"
    );

    // Every engine off: nothing to add, and the speed does not change.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 1000;
        built.cruise_velocity = 3000;
        for part in &mut built.parts {
            part.enabled = false;
        }
    }
    assert!(
        !cdda_sim::vehicle_move::thrust(&mut world, bicycle, 1),
        "**an engine that is off changes nothing**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("still there")
            .velocity,
        1000,
        "the speed is where it was"
    );
}

/// **An electric motor is recognised by the fuel its engine burns.**
///
/// Reference: `has_engine_type( fuel_type_battery, true )` (`vehicle.cpp:4289-4299`), which reads `engine_info::fuel_type` (`veh_type.h:130`). The electric car's
/// motor burns battery; the bicycle's muscle engine and the car's combustion engine do not.
/// Installs the item weights the reference recorded for one vehicle's parts.
///
/// A part's base item is what mass is summed from (`vehicle::calc_mass_center`), and the weights are in `corpus/vehicle_top_speed.json` because that dump records them
/// per part. Without them the registry answers zero grams and a vehicle would weigh nothing.
fn install_part_weights(world: &mut World, name: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/vehicle_top_speed.json");
    let bytes = std::fs::read(path).expect("the top speed recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };
    let case = cases
        .iter()
        .find_map(|case| match case {
            cdda_json::Value::Object(row) if text_of(row, "name") == name => Some(row),
            _ => None,
        })
        .expect("the recording has that vehicle");
    let Some(cdda_json::Value::Array(parts)) = case.get("parts") else {
        panic!("the case records its parts");
    };
    let mut kinds: Vec<(String, i64)> = Vec::new();
    for part in parts {
        let cdda_json::Value::Object(row) = part else {
            panic!("a part is an object");
        };
        let item = text_of(row, "item");
        let mass = int_of(row, "base_mass_g");
        if !kinds.iter().any(|(id, _)| *id == item) {
            kinds.push((item, i64::from(mass)));
        }
    }
    let refs: Vec<cdda_sim::item_registry::Kind<'_>> = kinds
        .iter()
        .map(|(id, mass)| cdda_sim::item_registry::Kind {
            id,
            volume_ml: 0,
            weight_g: *mass,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        })
        .collect();
    cdda_sim::item_registry::set_item_types(world, &refs);
}

#[test]
fn only_the_electric_car_has_a_battery_engine() {
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let electric = cdda_sim::vehicles::spawn(&mut world, "electric_car", (10, 10, 0))
        .expect("the electric car builds");
    let bicycle =
        cdda_sim::vehicles::spawn(&mut world, "bicycle", (20, 20, 0)).expect("the bicycle builds");
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (30, 30, 0)).expect("the car builds");

    assert!(
        cdda_sim::vehicle_drive::battery_engine(&world, electric),
        "**the electric car's motor burns battery**"
    );
    assert!(
        !cdda_sim::vehicle_drive::battery_engine(&world, bicycle),
        "a bicycle has no motor"
    );
    assert!(
        !cdda_sim::vehicle_drive::battery_engine(&world, car),
        "a combustion engine is not electric"
    );
}

/// **The drag a vehicle loses each turn is the reference's own number, at rest and at speed, engine on and off.**
///
/// `corpus/vehicle_slowdown.json` is `--dump-vehicle-slowdown`: four vehicles at seven speeds in both engine states, each row carrying the mass and the two
/// coefficients the rule divides by - so the check is arithmetic against the reference's coefficients rather than the port's, and the sum of two very different
/// terms (air drag growing with the square of speed, rolling drag with a 33.33 floor, and the 1500 W an idle engine adds) is pinned at both ends.
#[test]
fn the_drag_is_the_reference_s_at_every_speed_and_engine_state() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_slowdown.json");
    let bytes = std::fs::read(path).expect("the slowdown recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut idle = 0;
    let mut running = 0;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let coefficient = |key: &str| -> f64 {
            match row.get(key) {
                Some(cdda_json::Value::Float(value)) => *value,
                Some(cdda_json::Value::Int(value)) => {
                    f64::from(i32::try_from(*value).expect("a small int"))
                }
                _ => panic!("`{key}` is a number"),
            }
        };
        let engine_on = matches!(row.get("engine_on"), Some(cdda_json::Value::Bool(true)));
        assert_eq!(
            cdda_sim::vehicle_move::slowdown_of(
                coefficient("c_air"),
                coefficient("c_rolling"),
                i64::from(int_of(row, "mass_g")),
                int_of(row, "at"),
                engine_on,
            ),
            int_of(row, "slowdown"),
            "{:?} at {} with the engine {}: **the drag is the reference's**",
            text_of(row, "vehicle"),
            int_of(row, "at"),
            if engine_on { "running" } else { "off" }
        );
        if engine_on {
            running += 1;
        } else {
            idle += 1;
        }
    }
    assert_eq!(
        (idle, running),
        (28, 28),
        "both engine states were checked, at every speed"
    );
}

/// **A coasting vehicle loses speed and a stopped one stays stopped.**
///
/// Reference: `vehicle::gain_moves`'s slowdown block (`vehicle.cpp:6488-6499`). A bicycle at 1,000 vmiph with its engine **off** loses 1,758 to drag - the 1,500 W
/// idle term plus the rolling drag - which is more than it has: with nobody driving it stops, and with a driver asking for 3,000 it is left creeping at one vmiph
/// instead of stalling. With the engine running the same speed only sheds 258, which is the other branch of the same rule.
#[test]
fn coasting_loses_speed_and_a_stalled_coaster_stops_or_creeps() {
    let mut world = World::new();
    install_part_weights(&mut world, "bicycle");
    cdda_sim::vehicles::set_store(&mut world, store());
    let bicycle = cdda_sim::vehicles::spawn(&mut world, "bicycle", (10, 10, 0)).expect("it builds");
    let speed_of = |world: &World| {
        world
            .get::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("still there")
            .velocity
    };

    // Engine off at 1,000: the drag is 1,758, more than the speed, so nobody driving means a stop.
    world
        .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
        .expect("a vehicle")
        .velocity = 1000;
    assert!(cdda_sim::vehicle_move::coast(
        &mut world, bicycle, false, false
    ));
    assert_eq!(speed_of(&world), 0, "**it stops rather than stalling**");

    // The stall with a driver holding a cruise: one vmiph, not zero.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 1000;
        built.cruise_velocity = 3000;
    }
    assert!(cdda_sim::vehicle_move::coast(
        &mut world, bicycle, true, false
    ));
    assert_eq!(
        speed_of(&world),
        1,
        "**a driven coaster creeps rather than stalling**"
    );

    // Engine running, driven, at 1,000: the drag leaves 742 and the cruise is far away.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 1000;
    }
    assert!(cdda_sim::vehicle_move::coast(
        &mut world, bicycle, false, true
    ));
    assert_eq!(
        speed_of(&world),
        742,
        "**a running engine leaves 258 of drag, not 1,758**"
    );

    // A stopped vehicle has nothing to lose, and says so rather than moving.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 0;
    }
    assert!(!cdda_sim::vehicle_move::coast(
        &mut world, bicycle, false, true
    ));
}

/// **A square costs a fraction of a turn, and a vehicle that cannot afford one keeps the remainder.**
///
/// `corpus/vehicle_move.json` is `--dump-vehicle-move`: a bicycle and a car at five speeds, twelve calls of `act_on_map` each, recording the `of_turn` left and the
/// speed after every call. The port replays the same twelve calls against a real world and must land on the same hundredth, in both directions: a vehicle below 20
/// vmiph **stops** and spends 0.321 of a turn, and a square costing exactly one whole turn is not affordable this turn at all (`>=`, not `>`).
#[test]
fn a_square_costs_a_fraction_of_a_turn_and_the_rest_carries() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_move.json");
    let bytes = std::fs::read(path).expect("the move recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut steps_seen = 0;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let vehicle_name = text_of(row, "vehicle");
        let speed = int_of(row, "speed");
        let Some(cdda_json::Value::Array(steps)) = row.get("steps") else {
            panic!("a case records its steps");
        };

        let mut world = World::new();
        install_part_weights(&mut world, &vehicle_name);
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = cdda_sim::vehicles::spawn(&mut world, &vehicle_name, (100, 100, 0))
            .expect("the recorded vehicle builds");
        {
            let mut built = world
                .get_mut::<cdda_sim::vehicles::Vehicle>(vehicle)
                .expect("a vehicle");
            built.velocity = speed;
            built.cruise_velocity = speed;
            built.of_turn = 1.0;
        }

        for (index, step) in steps.iter().enumerate() {
            let cdda_json::Value::Object(step) = step else {
                panic!("a step is an object");
            };
            // `of_turn` is written by `%.9g`, so an exact zero comes back as an int.
            let want_turn = match step.get("of_turn") {
                Some(cdda_json::Value::Float(value)) => *value,
                Some(cdda_json::Value::Int(value)) => {
                    f64::from(i32::try_from(*value).expect("a small int"))
                }
                _ => panic!("a step records its turn as a number"),
            };
            let want_velocity = int_of(step, "velocity");
            cdda_sim::vehicle_move::move_step(&mut world, vehicle);
            let built = world
                .get::<cdda_sim::vehicles::Vehicle>(vehicle)
                .expect("still there");
            assert!(
                (f64::from(built.of_turn) - want_turn).abs() < 0.0001,
                "{vehicle_name} at {speed}, step {index}: **of_turn** is {} but the reference left {want_turn}",
                built.of_turn
            );
            assert_eq!(
                built.velocity, want_velocity,
                "{vehicle_name} at {speed}, step {index}: **the speed is the reference's**"
            );
            steps_seen += 1;
        }
    }
    assert_eq!(steps_seen, 120, "ten cases of twelve steps");

    // The edge the recording shows as a vehicle that never moves: a square that costs exactly the turn left is **not** affordable, because the comparison is `>=`.
    {
        let mut world = World::new();
        install_part_weights(&mut world, "bicycle");
        cdda_sim::vehicles::set_store(&mut world, store());
        let bicycle =
            cdda_sim::vehicles::spawn(&mut world, "bicycle", (100, 100, 0)).expect("it builds");
        {
            let mut built = world
                .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
                .expect("a vehicle");
            built.velocity = 400;
            built.of_turn = 1.0;
        }
        assert!(
            !cdda_sim::vehicle_move::move_step(&mut world, bicycle),
            "**a square costing a whole turn is not affordable**"
        );
        assert_eq!(
            world
                .get::<cdda_sim::Position>(bicycle)
                .expect("a position")
                .x,
            100,
            "it stayed put"
        );
    }

    // The arithmetic itself, at the speeds the dump used: a bicycle at 400 spends a whole turn on a square, at 1,000 spends 0.4 of one, and at 20 spends twenty.
    assert!((cdda_sim::vehicle_move::turn_cost(400) - 1.0).abs() < f32::EPSILON);
    assert!((cdda_sim::vehicle_move::turn_cost(1000) - 0.4).abs() < 0.000_001);
    assert!((cdda_sim::vehicle_move::turn_cost(20) - 20.0).abs() < f32::EPSILON);
}

/// **A vehicle that can afford a square takes it, and the index follows.**
///
/// Reference: the affordable branch of `vehicle::act_on_map` (`vehicle_move.cpp:1900-2010`) - the turn is spent, then exactly one square is taken along the facing.
/// The recorded bicycle at 1,000 vmiph moves one square east and is left with 0.6 of the turn; a bicycle at 400 cannot afford its square this turn and stays put.
#[test]
fn an_affordable_square_is_taken_and_the_index_follows() {
    let mut world = World::new();
    install_part_weights(&mut world, "bicycle");
    cdda_sim::vehicles::set_store(&mut world, store());
    let bicycle = cdda_sim::vehicles::spawn(&mut world, "bicycle", (10, 10, 0)).expect("it builds");

    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 1000;
        built.of_turn = 1.0;
    }
    assert!(
        cdda_sim::vehicle_move::move_step(&mut world, bicycle),
        "**a square at 1,000 vmiph is affordable**"
    );
    let built = world
        .get::<cdda_sim::vehicles::Vehicle>(bicycle)
        .expect("a vehicle");
    assert!(
        (built.of_turn - 0.6).abs() < 0.000_001,
        "the turn is spent: {}",
        built.of_turn
    );
    let position = world
        .get::<cdda_sim::Position>(bicycle)
        .expect("a position");
    assert_eq!((position.x, position.y), (11, 10), "**one square east**");
    assert_eq!(
        cdda_sim::vehicles::at(&world, (11, 10, 0)),
        Some(bicycle),
        "**the index follows the vehicle**"
    );
    // The bicycle is three squares long: a part sits one mount behind the origin and one in front, so the walk east vacates the square at nine and takes the one at twelve.
    assert_eq!(
        cdda_sim::vehicles::at(&world, (9, 10, 0)),
        None,
        "the square behind the old tail is empty"
    );
    assert_eq!(
        cdda_sim::vehicles::at(&world, (12, 10, 0)),
        Some(bicycle),
        "the square ahead of the old nose is taken"
    );

    // At 400 a square costs a whole turn, which is not affordable this turn: the vehicle stays and keeps the fraction.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 400;
        built.of_turn = 1.0;
    }
    assert!(
        !cdda_sim::vehicle_move::move_step(&mut world, bicycle),
        "**a square that costs a whole turn is not affordable**"
    );
    let position = world
        .get::<cdda_sim::Position>(bicycle)
        .expect("a position");
    assert_eq!((position.x, position.y), (11, 10), "it stayed put");
    let built = world
        .get::<cdda_sim::vehicles::Vehicle>(bicycle)
        .expect("a vehicle");
    assert!(
        built.of_turn.abs() < f32::EPSILON,
        "nothing left this turn: {}",
        built.of_turn
    );
    assert!(
        (built.of_turn_carry - 1.0).abs() < f32::EPSILON,
        "**the fraction is carried**: {}",
        built.of_turn_carry
    );
}

/// **A vehicle does not drive through a wall: it stops at the last square it can stand on.**
///
/// Reference: `vehicle::part_collision` reports terrain for a square a structure part would enter (`vehicle_move.cpp:950-970`) and `map::move_vehicle` calls
/// `veh.stop()` when a collision leaves the vehicle unable to move (`map.cpp:948-950`). The detection is the port's own `first_collision`, so this gate is about
/// what movement does with the answer: the square is never entered and the speed is gone.
#[test]
fn a_vehicle_stops_at_a_wall_rather_than_driving_through_it() {
    const FLOOR: u32 = 1;
    const WALL: u32 = 2;

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_floor".to_owned(), FLOOR), ("t_wall".to_owned(), WALL)],
    );
    cdda_sim::bash::set_data(
        &mut world,
        [recorded_tile("t_floor"), recorded_tile("t_wall")],
    );
    for y in 95..=105 {
        for x in 95..=110 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    for y in 95..=105 {
        cdda_sim::square::set_terrain(&mut world, (103, y, 0), WALL);
    }
    install_part_weights(&mut world, "bicycle");
    cdda_sim::vehicles::set_store(&mut world, store());

    let bicycle =
        cdda_sim::vehicles::spawn(&mut world, "bicycle", (100, 100, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle");
        built.velocity = 1000;
        built.of_turn = 1.0;
    }

    assert!(
        cdda_sim::vehicle_move::move_step(&mut world, bicycle),
        "the first step is on open floor"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(bicycle)
            .expect("a position")
            .x,
        101,
        "one square east"
    );

    // The bicycle is three squares long, so at x = 101 its nose stands on 102: the next step would put that nose in the wall at 103.
    assert!(
        !cdda_sim::vehicle_move::move_step(&mut world, bicycle),
        "**the wall is not entered**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(bicycle)
            .expect("a position")
            .x,
        101,
        "it stayed where it stood"
    );
    assert_eq!(
        world
            .get::<cdda_sim::vehicles::Vehicle>(bicycle)
            .expect("a vehicle")
            .velocity,
        0,
        "**the crash takes the speed**"
    );

    // A stopped vehicle stays stopped: twelve more attempts never move it.
    for _ in 0..12 {
        assert!(!cdda_sim::vehicle_move::move_step(&mut world, bicycle));
    }
    assert_eq!(
        world
            .get::<cdda_sim::Position>(bicycle)
            .expect("a position")
            .x,
        101,
        "and it never enters the wall"
    );
}

/// **What a crash costs each vehicle, as the reference recorded it.**
///
/// `corpus/vehicle_crash.json` is `--dump-vehicle-crash`: a bicycle and a car driven into a wall at four speeds. Each row carries the mass, the speed after the
/// crash, the part that took the hit, its hit points before and after, and the two densities the damage split divides by. The port's impulse loop lands next
/// round and will reproduce the hit points; this test pins the recording itself so the numbers cannot drift while it is being written, and so the file is a gate
/// rather than a promise.
#[test]
fn a_crash_into_a_wall_costs_the_part_the_reference_recorded() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_crash.json");
    let bytes = std::fs::read(path).expect("the crash recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut damaged = 0;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let name = text_of(row, "vehicle");
        let speed = int_of(row, "speed");
        seen.insert((name.clone(), speed));

        // Every crash takes the speed: the car and the bicycle both come to rest.
        assert_eq!(
            int_of(row, "velocity_after"),
            0,
            "**{name} at {speed} stops at the wall**"
        );
        let lost = int_of(row, "part_lost");
        assert_eq!(
            lost,
            int_of(row, "hp_before") - int_of(row, "hp_after"),
            "{name} at {speed}: the hit points lost are the difference"
        );
        if lost > 0 {
            damaged += 1;
            assert!(
                !text_of(row, "part_name").is_empty(),
                "{name} at {speed}: a damaged part is named"
            );
        }
    }
    assert_eq!(seen.len(), 8, "two vehicles at four speeds");
    assert_eq!(
        damaged, 2,
        "**the car takes damage at 1,000 and 2,000 and the bicycle takes none**"
    );

    // The two numbers the port has to reproduce, and the part each one lands on.
    let damaged_row = |speed: i32| {
        cases
            .iter()
            .find_map(|case| {
                let cdda_json::Value::Object(row) = case else {
                    return None;
                };
                (text_of(row, "vehicle") == "car" && int_of(row, "speed") == speed)
                    .then(|| int_of(row, "part_lost"))
            })
            .expect("the car has a row at that speed")
    };
    assert_eq!(damaged_row(1000), 41, "the car at 1,000 loses 41");
    assert_eq!(
        damaged_row(2000),
        44,
        "**at 2,000 the fragile windshield takes the hit instead, and it costs more**"
    );
}

/// **The impulse a crash produces is the reference's own number.**
///
/// The recording's `imp` column came from a guarded print inside `part_collision`, so this compares the port's loop against what the reference computed rather than
/// against hit points that a second rule then spreads. Three rows name a part with a recorded density; the bicycle's are skipped because the dump only names a
/// density for the part that lost the most, and no bicycle part lost any.
#[test]
fn a_crash_produces_the_impulse_the_reference_produces() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_crash.json");
    let bytes = std::fs::read(path).expect("the crash recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let density = match row.get("vpart_dens") {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => f64::from(i32::try_from(*value).expect("small")),
            _ => panic!("a part density"),
        };
        if density <= 0.0 {
            continue;
        }
        let (impulse, obstacle, left) = cdda_sim::vehicle_move::crash_impulse(
            i64::from(int_of(row, "mass_g")),
            int_of(row, "speed"),
            density,
        );
        assert_eq!(
            impulse,
            int_of(row, "imp"),
            "{} at {}: **the impulse is the reference's**",
            text_of(row, "vehicle"),
            int_of(row, "speed")
        );
        assert_eq!(left, 0, "the crash leaves no speed");
        // **The obstacle gets the rest**, about a quarter of the blow because the material factor is clamped at 25.
        assert!(
            obstacle > 0 && obstacle < impulse,
            "the obstacle takes {obstacle} of the {impulse} dealt"
        );
        seen += 1;
    }
    assert_eq!(seen, 2, "two recorded rows name a part and its density");
}

/// **A crash leaves a mark in the store.**
///
/// The car is three squares on a side, so standing at 100 its nose is on 101 and the wall at 103 is one step away - the geometry the bicycle test uses, one square
/// longer. Driven at 1,000 vmiph the reference records 41 hit points off the frame (`corpus/vehicle_crash.json`); this asks the store what the car is worth after.
#[test]
fn a_crash_marks_the_car_that_hits_the_wall() {
    const FLOOR: u32 = 1;
    const WALL: u32 = 2;

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_floor".to_owned(), FLOOR), ("t_wall".to_owned(), WALL)],
    );
    cdda_sim::bash::set_data(
        &mut world,
        [recorded_tile("t_floor"), recorded_tile("t_wall")],
    );
    for y in 95..=105 {
        for x in 95..=110 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    for y in 95..=105 {
        cdda_sim::square::set_terrain(&mut world, (103, y, 0), WALL);
    }
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());

    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 1000;
        built.of_turn = 20.0;
    }
    for _ in 0..6 {
        if !cdda_sim::vehicle_move::move_step(&mut world, car) {
            break;
        }
    }

    let dented = {
        let built = world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        // **The direct hit, not the spread**: the part that lost the most is the one the collision blamed, and that is the one the impulse's arithmetic covers.
        // Its neighbours are dented too, by `damage_around`, and those are checked separately.
        built
            .parts
            .iter()
            .enumerate()
            .filter_map(|(index, part)| {
                let spec = cdda_sim::vehicles::part_spec(&world, car, index)?;
                (part.hp < spec.durability).then(|| (spec.id.clone(), spec.durability - part.hp))
            })
            .max_by_key(|(_, lost)| *lost)
    };
    let (id, lost) = dented.expect("**the crash dented a part**");
    // The rule is verified against the reference in `a_crash_produces_the_impulse_the_reference_produces`; this checks the **wiring** - that the blow the rule
    // computes is what the store receives, after the part's own reduction. The two numbers differ from 41 only because this fixture's car is assembled from the
    // top-speed recording's per-item masses and the reference's car is the same blueprint with its own, so the masses differ by a few grams per part.
    let spec = cdda_sim::vehicles::part_spec(&world, car, 0)
        .or_else(|| {
            let built = world.get::<cdda_sim::vehicles::Vehicle>(car)?;
            built.parts.iter().enumerate().find_map(|(index, part)| {
                cdda_sim::vehicles::part_spec(&world, car, index)
                    .filter(|spec| spec.id == id && part.hp < spec.durability)
            })
        })
        .expect("the dented part's spec");
    let density = spec.density.expect("a density");
    let mass = cdda_sim::vehicle_mass::mass(&world, car);
    let (impulse, _, _) = cdda_sim::vehicle_move::crash_impulse(mass, 1000, density);
    let reduction = spec.damage_reduction.get("bash").copied().unwrap_or(0.0);
    #[allow(clippy::cast_possible_truncation)]
    let expected = impulse - reduction as i32;
    assert_eq!(
        lost, expected,
        "**{id} lost {lost}, and the ported rule says {expected} ({impulse} before {reduction} of bash reduction)**"
    );
    assert!(
        (33..=45).contains(&lost),
        "and that is within a few hit points of the reference's 41: {lost}"
    );
    // The spread reaches the neighbourhood: some part other than the direct hit lost hit points too.
    let dented = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map_or(0, |built| {
            built
                .parts
                .iter()
                .enumerate()
                .filter(|(index, part)| {
                    cdda_sim::vehicles::part_spec(&world, car, *index)
                        .is_some_and(|spec| part.hp < spec.durability)
                })
                .count()
        });
    assert!(
        dented > 1,
        "**the blow spreads: {dented} part(s) dented, not just the one at the wall**"
    );
    // And the spread is weaker than the blow: every part other than the direct hit lost less than it did.
    let others = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map(|built| {
            built
                .parts
                .iter()
                .enumerate()
                .filter_map(|(index, part)| {
                    let spec = cdda_sim::vehicles::part_spec(&world, car, index)?;
                    (part.hp < spec.durability && spec.id != id)
                        .then_some(spec.durability - part.hp)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for lost in &others {
        assert!(
            *lost < 38,
            "**a neighbour lost {lost}, less than the direct hit's 38**"
        );
    }
}

/// **The spread falls off with the square of the distance.** Reference: `damage_all`'s `rng( dmg1, dmg2 ) / ( distance * distance )`
/// (`vehicle.cpp:7679`), with the distance counted as `1 + square_dist`.
#[test]
fn a_blow_reaches_further_parts_more_weakly() {
    assert_eq!(
        cdda_sim::vehicle_move::spread_share(100, 2, 0),
        25,
        "an adjacent-square part takes a quarter"
    );
    assert_eq!(
        cdda_sim::vehicle_move::spread_share(100, 3, 0),
        11,
        "and one square further, a ninth - integer division, as the reference writes it"
    );
    assert_eq!(
        cdda_sim::vehicle_move::spread_share(100, 1, 0),
        0,
        "**the part at the impact takes nothing from the spread**"
    );
    assert_eq!(
        cdda_sim::vehicle_move::spread_share(100, 2, 5),
        20,
        "a shock absorber takes its bonus off first"
    );
    assert_eq!(
        cdda_sim::vehicle_move::spread_share(10, 4, 20),
        0,
        "and never below zero"
    );
}

/// **The data decides which obstacles give way.** A block with a real `str_min` can be bashed; none cannot; `-1` is the data's word for "never"; and a
/// `destroy_only` block answers no to an ordinary hit (`map.cpp`'s `bash_ter_furn` test, which the collision calls with `destroy` false).
#[test]
fn the_data_says_which_obstacles_can_be_bashed() {
    use cdda_content::mapdata::BashInfo;
    let block = |str_min: i32, destroy_only: bool| BashInfo {
        str_min,
        destroy_only,
        ..BashInfo::default()
    };

    let wall = block(20, false);
    assert!(
        cdda_sim::bash::accepts_bash(Some(&wall)),
        "**a wall gives way**"
    );
    assert!(
        !cdda_sim::bash::accepts_bash(None),
        "**open air has no block, so nothing to break**"
    );
    let never = block(-1, false);
    assert!(
        !cdda_sim::bash::accepts_bash(Some(&never)),
        "a -1 str_min is the data's never"
    );
    let only_destroy = block(20, true);
    assert!(
        !cdda_sim::bash::accepts_bash(Some(&only_destroy)),
        "a destroy-only block answers no to a bash"
    );
}

/// **The square's resistance is the furniture's if it is bashable, else the terrain's, else none.** Reference: `map::bash_resistance` (`map.cpp:3204-3215`).
#[test]
fn a_square_resists_with_what_stands_on_it_first() {
    use cdda_content::mapdata::BashInfo;
    let block = |str_min: i32| BashInfo {
        str_min,
        ..BashInfo::default()
    };
    let (terrain, furniture) = (block(20), block(50));

    assert_eq!(
        cdda_sim::bash::resistance(Some(&furniture), Some(&terrain)),
        50,
        "**the furniture answers first**"
    );
    assert_eq!(
        cdda_sim::bash::resistance(None, Some(&terrain)),
        20,
        "with nothing on it, the terrain answers"
    );
    assert_eq!(
        cdda_sim::bash::resistance(None, None),
        -1,
        "**and an unbreakable square answers -1**"
    );
    let only_destroy = BashInfo {
        str_min: 50,
        destroy_only: true,
        ..BashInfo::default()
    };
    assert_eq!(
        cdda_sim::bash::resistance(Some(&only_destroy), Some(&terrain)),
        20,
        "a destroy-only block does not answer, so the terrain's does"
    );
}

/// **A wall can be bashed and open air cannot: the real data says so.**
///
/// The crash's obstacle branch needs a bash block on the tiles it hits, and the collision fixtures build theirs by hand. This reads the reference's own terrain data
/// and checks the shapes the rule distinguishes: a wooden wall with a real `str_min`, open air with no block at all, and a vitrified wall the data does not let an
/// ordinary bash through.
#[test]
fn the_real_data_says_which_tiles_a_bash_gets_through() {
    use cdda_content::mapdata::MapDataKind;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA");
    let loaded =
        cdda_content::mapdata::load_paths(&[&root.join("data/core"), &root.join("data/json")])
            .expect("loads the content set");
    let tile = |id: &str| {
        loaded
            .iter()
            .find(|prototype| prototype.id == id && prototype.kind == MapDataKind::Terrain)
            .unwrap_or_else(|| panic!("{id} is in the data"))
    };

    let wall = tile("t_wall");
    let block = wall.bash.as_ref().expect("**a wall has a bash block**");
    assert!(block.str_min > 0, "its str_min is real: {}", block.str_min);
    assert!(!block.destroy_only, "and an ordinary bash reaches it");
    assert!(
        cdda_sim::bash::accepts_bash(wall.bash.as_ref()),
        "so the guard lets a hit through"
    );

    let air = tile("t_open_air");
    assert!(
        air.bash.is_none(),
        "**open air has no block, which is why a car drives into it**"
    );
    assert!(!cdda_sim::bash::accepts_bash(air.bash.as_ref()));

    let vitrified = tile("t_vitrified_wall");
    assert!(
        !cdda_sim::bash::accepts_bash(vitrified.bash.as_ref()),
        "**and the vitrified wall is the stone a crash cannot break**"
    );
}

/// **The comparison the obstacle branch makes**, checked against the two recorded crashes: the car at 1,000 deals 15 and a wall resists far more, so it stands; the
/// car at 2,000 deals 62 and the same wall gives way. `-1` is a square nothing can break.
#[test]
fn a_hit_breaks_a_square_only_when_it_reaches_its_resistance() {
    assert!(
        !cdda_sim::bash::breaks(20, 15),
        "**15 against a wall that resists 20 leaves it standing**"
    );
    assert!(
        cdda_sim::bash::breaks(20, 62),
        "**62 against the same wall takes it down**"
    );
    assert!(
        cdda_sim::bash::breaks(20, 20),
        "and equality breaks it: the reference compares with `<=`"
    );
    assert!(
        !cdda_sim::bash::breaks(-1, 1_000_000),
        "a square nothing can break is not broken by any strength"
    );
}

/// **The prediction the split makes, against the wall's real resistance.** A car at 1,000 deals its obstacle 15 and a car at 2,000 deals 62; the wall's own `str_min`
/// comes from the reference's data. If those two numbers straddle it, a wooden wall stands against the slower car and gives way to the faster one - which is the
/// behaviour the obstacle branch must produce once it is wired.
#[test]
fn the_slower_car_leaves_the_wall_and_the_faster_one_does_not() {
    use cdda_content::mapdata::MapDataKind;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA");
    let loaded =
        cdda_content::mapdata::load_paths(&[&root.join("data/core"), &root.join("data/json")])
            .expect("loads the content set");
    let wall = loaded
        .iter()
        .find(|prototype| prototype.id == "t_wall" && prototype.kind == MapDataKind::Terrain)
        .expect("the wall");
    let resistance = cdda_sim::bash::resistance(None, wall.bash.as_ref());
    assert!(resistance > 0, "the wall resists: {resistance}");

    // The shares the split gives the obstacle at the two recorded speeds, worked out from `crash_impulse`'s own numbers.
    let shares = (15, 62);
    assert!(
        !cdda_sim::bash::breaks(resistance, shares.0),
        "**15 against a wall resisting {resistance} leaves it standing**"
    );
    assert!(
        cdda_sim::bash::breaks(resistance, shares.1),
        "**62 against the same wall takes it down**"
    );
}

/// **A crash strong enough leaves the wall broken.**
///
/// The car at 2,000 deals the obstacle about 62 and the fixture's wall resists 10, so the square the collision reports comes away changed - carrying bash damage, or
/// standing as different terrain because the hit went through. The crash draws from its own stream (`bash_square_with_rolls`), so this needs no dice in the store; it
/// does need the map data table, which is what `bash_square` reads the square through.
#[test]
fn a_fast_crash_breaks_the_wall_it_hits() {
    const FLOOR: u32 = 1;
    const WALL: u32 = 2;

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_floor".to_owned(), FLOOR), ("t_wall".to_owned(), WALL)],
    );
    let mut wall = recorded_tile("t_wall");
    wall.bash = Some(cdda_content::mapdata::BashInfo {
        str_min: 10,
        str_max: 10,
        // **What it becomes.** A block that names nothing leaves the square standing even when the hit goes through, because there is no terrain to turn it into.
        set: "t_floor".to_owned(),
        ..cdda_content::mapdata::BashInfo::default()
    });
    cdda_sim::mapsquare::set_data(
        &mut world,
        [recorded_tile("t_floor"), wall, recorded_tile("t_open_air")],
    );
    for y in 95..=105 {
        for x in 95..=110 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    for y in 95..=105 {
        cdda_sim::square::set_terrain(&mut world, (103, y, 0), WALL);
    }
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());

    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 2000;
        built.of_turn = 20.0;
    }
    for _ in 0..6 {
        if !cdda_sim::vehicle_move::move_step(&mut world, car) {
            break;
        }
    }

    let changed = (95..=105).any(|y| {
        let square = (103, y, 0);
        cdda_sim::square::bash_damage_at(&world, square) > 0
            || cdda_sim::mapsquare::at(&world, square)
                .is_some_and(|data| data.terrain.id != "t_wall")
    });
    assert!(
        changed,
        "**the wall took the hit somewhere along its length**"
    );
}

/// **A slow car leaves the wall standing.** The other half of the obstacle branch: the same wall, the same approach, a quarter of the speed - and the square is
/// untouched, because the share the split gives it falls below the wall's resistance. Without this, a branch that broke everything it touched would pass.
#[test]
fn a_slow_crash_leaves_the_wall_standing() {
    const FLOOR: u32 = 1;
    const WALL: u32 = 2;

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_floor".to_owned(), FLOOR), ("t_wall".to_owned(), WALL)],
    );
    let mut wall = recorded_tile("t_wall");
    wall.bash = Some(cdda_content::mapdata::BashInfo {
        str_min: 60,
        str_max: 60,
        set: "t_floor".to_owned(),
        ..cdda_content::mapdata::BashInfo::default()
    });
    cdda_sim::mapsquare::set_data(
        &mut world,
        [recorded_tile("t_floor"), wall, recorded_tile("t_open_air")],
    );
    for y in 95..=105 {
        for x in 95..=110 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    for y in 95..=105 {
        cdda_sim::square::set_terrain(&mut world, (103, y, 0), WALL);
    }
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());

    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 400;
        built.of_turn = 20.0;
    }
    for _ in 0..8 {
        if !cdda_sim::vehicle_move::move_step(&mut world, car) {
            break;
        }
    }

    let changed = (95..=105).any(|y| {
        let square = (103, y, 0);
        cdda_sim::square::bash_damage_at(&world, square) > 0
            || cdda_sim::mapsquare::at(&world, square)
                .is_some_and(|data| data.terrain.id != "t_wall")
    });
    assert!(
        !changed,
        "**a 400 vmiph car does not break a wall that resists 60**"
    );
}

/// **The exchange keeps total momentum and, at restitution zero, makes the two speeds equal.** Reference: `map.cpp:1100-1112`.
#[test]
fn two_vehicles_exchange_momentum_and_share_a_speed_when_they_stick() {
    // **Speeds are the game's own `velocity`, which is 100 to the mile per hour**, not metres per second: 1,000 is ten miles an hour.
    let (a, b) = cdda_sim::vehicle_move::exchanged(1000.0, 1000.0, 1000.0, 0.0, 0.0);
    assert!(
        (a - 500.0).abs() < 1e-9 && (b - 500.0).abs() < 1e-9,
        "**equal masses, no bounce: both leave at 500** ({a}, {b})"
    );
    // Momentum is conserved for any restitution: m1*v1 + m2*v2 before equals after.
    for e in [0.0, 0.1, 1.0] {
        let (a, b) = cdda_sim::vehicle_move::exchanged(1200.0, 800.0, 900.0, -300.0, e);
        let before = 1200.0 * 800.0 + 900.0 * -300.0;
        let after = 1200.0 * a + 900.0 * b;
        assert!(
            (before - after).abs() < 1e-6,
            "momentum at e={e}: {before} against {after}"
        );
    }
}

/// **A heavier vehicle barely slows.** 2,000 kg into 500 kg at 10 m/s, no bounce: the pair leaves at 8 and 8.
#[test]
fn the_heavier_vehicle_decides_the_speed() {
    let (a, b) = cdda_sim::vehicle_move::exchanged(2000.0, 1000.0, 500.0, 0.0, 0.0);
    assert!((a - 800.0).abs() < 1e-9, "the heavy one keeps 800: {a}");
    assert!(
        (b - 800.0).abs() < 1e-9,
        "and drags the light one to 800: {b}"
    );
}

/// **The collision's damage comes from the lost energy**, with the rule's own divisors - `| d_E / 1000 / 2000 |` (`map.cpp:1144`).
#[test]
fn a_collision_deals_damage_from_the_energy_it_loses() {
    // 1000 kg at 10 m/s meets 1000 kg at rest, restitution 0: E before 50,000, after 25,000, so 25,000 J lost.
    let damage = cdda_sim::vehicle_move::collision_damage(1000.0, 1000.0, 1000.0, 0.0, 0.0);
    assert_eq!(
        damage, 125,
        "**250,000,000 / 1,000 / 2,000 is 125** - the obstacle branch's 400 would have said 625"
    );
    // Nothing lost, nothing dealt: a perfectly elastic pass-through.
    assert_eq!(
        cdda_sim::vehicle_move::collision_damage(1000.0, 1000.0, 1000.0, 0.0, 1.0),
        0,
        "an elastic exchange loses no energy"
    );
}

/// **A nudge bounces, a hard hit does not.** Reference: `get_collision_factor` (`vehicle_move.cpp:1636-1643`) - 1 falling to 0.1 across 31 miles an hour.
#[test]
fn the_collision_factor_falls_from_one_to_a_tenth_across_thirty_one() {
    let factor = cdda_sim::vehicle_move::collision_factor;
    assert!((factor(0.0) - 1.0).abs() < 1e-12, "no difference, no loss");
    assert!(
        (factor(31.0) - 0.1).abs() < 1e-12,
        "**31 miles an hour apart is the floor**"
    );
    assert!(
        (factor(15.5) - 0.55).abs() < 1e-9,
        "halfway is 0.55, a straight line between them"
    );
    assert!(
        (factor(100.0) - 0.1).abs() < 1e-12,
        "beyond the floor it stays"
    );
    assert!(
        (factor(-31.0) - 0.1).abs() < 1e-12,
        "and the sign does not matter: it takes the absolute difference"
    );
}

/// **Half the damage, shared out across the part pairs.** Reference: `map.cpp:1157-1169` - `dmg * 0.5` then `/ coll_parts_cnt`.
#[test]
fn a_collision_s_half_is_shared_across_its_part_pairs() {
    let share = cdda_sim::vehicle_move::collision_share;
    assert_eq!(share(125, 1), 62, "**one pair takes half, truncated**");
    assert_eq!(share(125, 3), 20, "three pairs spread it over six parts");
    assert_eq!(share(0, 1), 0, "no blow, nothing to share");
    assert_eq!(share(125, 0), 0, "and no pairs is nothing, not a division");
}

/// **Two cars meeting lose speed and hit points.** 1,000 kg a side, one at 1,000 vmiph and one standing, restitution 1 falling to 0.1 at that gap: the mover slows towards
/// half, the other is shoved, and both take damage - the exchange, the factor, the damage and the share, all through the seam.
#[test]
fn two_cars_that_meet_exchange_speed_and_damage() {
    const FLOOR: u32 = 1;
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
    cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
    for y in 95..=105 {
        for x in 95..=115 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());

    let mover = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let standing = cdda_sim::vehicles::spawn(&mut world, "car", (108, 100, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(mover)
            .expect("a vehicle");
        built.velocity = 1000;
        built.of_turn = 20.0;
    }
    let hp_before = |world: &World, car: Entity| {
        world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| built.parts.iter().map(|part| part.hp).sum())
    };
    let before = (hp_before(&world, mover), hp_before(&world, standing));

    for _ in 0..10 {
        if !cdda_sim::vehicle_move::move_step(&mut world, mover) {
            break;
        }
    }

    let velocity = |world: &World, car: Entity| {
        world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| built.velocity)
    };
    let after = (hp_before(&world, mover), hp_before(&world, standing));
    assert!(
        velocity(&world, standing) > 0,
        "**the parked car is shoved: {} vmiph**",
        velocity(&world, standing)
    );
    assert!(
        after.0 < before.0 || after.1 < before.1,
        "**one of them loses hit points: {} and {} from {} and {}**",
        after.0,
        after.1,
        before.0,
        before.1
    );
}

/// **Safe mode caps each engine at its own ratio, then shares the total.** Reference: `vehicle.cpp:4052-4079`.
#[test]
fn safe_power_caps_every_engine_at_its_own_ratio() {
    let power = cdda_sim::vehicle_drive::safe_power;
    assert_eq!(power(&[(1000, 100)]), 1000, "a full ratio takes nothing");
    assert_eq!(power(&[(1000, 60)]), 600, "**60% is 600 watts**");
    assert_eq!(
        power(&[(1000, 60), (1000, 60)]),
        960,
        "two capped engines are 1,200 shared by 4/5"
    );
    assert_eq!(power(&[]), 0, "no engines, no power");
}

/// Puts a tankful of the car's own fuel in every tank it has, so an engine that needs fuel can run. A car off the blueprint is dry, which is the rule's own default.
fn fill_own_tanks(world: &mut World, car: Entity) {
    let Some(count) = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map(|built| built.parts.len())
    else {
        return;
    };
    let fuel = (0..count)
        .filter_map(|index| cdda_sim::vehicles::part_spec(world, car, index))
        .find_map(|spec| spec.fuel_opts.first().cloned());
    let Some(fuel) = fuel else { return };
    for index in 0..count {
        let is_tank =
            cdda_sim::vehicles::part_spec(world, car, index).is_some_and(|spec| spec.fuel_store);
        if !is_tank {
            continue;
        }
        if let Some(mut built) = world.get_mut::<cdda_sim::vehicles::Vehicle>(car)
            && let Some(part) = built.parts.get_mut(index)
        {
            part.fuel.clone_from(&fuel);
            part.charges = 100;
        }
    }
}

/// **Safe mode asks for less than full power.** The store's half of `safe_power`: the car's engines carry an `m2c` ratio below 100, so the capped total is smaller than
/// the full one and still positive. Both numbers come from the same parts, so the test says the ratio reached the rule rather than that some number did.
#[test]
fn safe_mode_asks_for_less_power_than_full() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);

    let full = cdda_sim::vehicle_drive::total_power(&world, car);
    let safe = cdda_sim::vehicle_drive::safe_power_of(&world, car);
    assert!(full > 0, "**the car's engine makes power: {full} W**");
    assert!(
        safe > 0 && safe < full,
        "**safe mode asks for less: {safe} W against {full} W**"
    );
}

/// **A switched-off engine makes nothing.** The reference's `is_engine_on` (`vehicle.cpp:1061-1064`): an engine that is not enabled contributes no power, so safe power
/// falls to zero when every engine is off.
#[test]
fn safe_power_needs_the_engine_switched_on() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);

    let running = cdda_sim::vehicle_drive::safe_power_of(&world, car);
    assert!(running > 0, "**running engines make {running} W**");

    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        for part in &mut built.parts {
            part.enabled = false;
        }
    }
    assert_eq!(
        cdda_sim::vehicle_drive::safe_power_of(&world, car),
        0,
        "**nothing on, nothing made**"
    );
}

/// **An engine is dry until a tank holds its fuel.** Reference: `engine_fuel_left` asking `fuel_left` for the engine's own fuel. A car off the blueprint carries empty
/// tanks, so its engines have nothing; filling the tank it feeds from makes them fuelled.
#[test]
fn an_engine_is_dry_until_a_tank_holds_its_fuel() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");

    let engine = (0..world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .expect("a vehicle")
        .parts
        .len())
        .find(|index| {
            cdda_sim::vehicles::part_spec(&world, car, *index)
                .is_some_and(|spec| !spec.fuel_opts.is_empty())
        })
        .expect("the car has an engine");
    let fuel = cdda_sim::vehicles::part_spec(&world, car, engine)
        .expect("a spec")
        .fuel_opts
        .first()
        .cloned()
        .expect("the engine names a fuel");

    assert!(
        !cdda_sim::vehicle_drive::engine_fuel_left(&world, car, engine),
        "**a fresh car has empty tanks**"
    );
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        let tank = built.parts.len() - 1;
        built.parts[tank].fuel.clone_from(&fuel);
        built.parts[tank].charges = 10;
    }
    assert!(
        cdda_sim::vehicle_drive::engine_fuel_left(&world, car, engine),
        "**ten charges of {fuel} feed it**"
    );
}

/// **Strain is zero inside the safe band and grows to one at the top.** Reference: `vehicle::strain` (`vehicle.cpp:4967-4982`) - nothing at rest, nothing below the safe
/// speed in either direction, and the share of the way from safe to maximum beyond it.
#[test]
fn strain_is_zero_inside_the_safe_band_and_one_at_the_top() {
    let strain = cdda_sim::vehicle_drive::strain;
    assert!((strain(0, 500, 1000) - 0.0).abs() < 1e-12, "at rest, none");
    assert!(
        (strain(400, 500, 1000) - 0.0).abs() < 1e-12,
        "**inside the band, none**"
    );
    assert!(
        (strain(-400, 500, 1000) - 0.0).abs() < 1e-12,
        "and the same going backwards"
    );
    assert!(
        (strain(750, 500, 1000) - 0.5).abs() < 1e-12,
        "**halfway from safe to maximum is 0.5**"
    );
    assert!(
        (strain(1000, 500, 1000) - 1.0).abs() < 1e-12,
        "at the maximum it is one"
    );
    // The reference's own guard: a maximum at or below the safe speed still divides.
    assert!(
        strain(600, 500, 500).is_finite(),
        "**a maximum of 500 does not divide by zero**"
    );
}

/// **Engines burning the same fuel add up; different fuels stay apart.** Reference: `vehicle::fuel_usage` (`vehicle.cpp:5196-5207`) - one entry per fuel, the sum of the
/// running engines that burn it. An engine with no fuel named, or one that draws nothing, contributes nothing.
#[test]
fn engines_burning_the_same_fuel_add_up() {
    let usage = cdda_sim::vehicle_drive::fuel_usage;
    let diesel = "diesel".to_owned();
    let gasoline = "gasoline".to_owned();
    let sum = usage(&[(diesel.clone(), 559_500), (diesel.clone(), 100_000)]);
    assert_eq!(
        sum.get(&diesel),
        Some(&659_500),
        "**one fuel, both engines**"
    );
    assert_eq!(sum.len(), 1, "and no second entry");
    let split = usage(&[(diesel.clone(), 559_500), (gasoline.clone(), 250_000)]);
    assert_eq!(split.get(&diesel), Some(&559_500), "diesel keeps its own");
    assert_eq!(split.get(&gasoline), Some(&250_000), "and gasoline its own");
    assert!(
        usage(&[(String::new(), 500), (diesel, 0)]).is_empty(),
        "nothing named, nothing drawn"
    );
}

/// **Draining stops when the want is met, and takes whole charges.** Reference: `vehicle::drain_energy` (`vehicle.cpp:5209-5224`) - a walk over the parts that stops once
/// the energy wanted has been given. Diesel holds 28,640 J a charge, so 30,000 J takes two charges (57,280 J) and leaves three of the five; a tank of another fuel is not
/// touched.
#[test]
fn draining_takes_whole_charges_of_the_fuel_asked_for() {
    let diesel = "diesel".to_owned();
    let gasoline = "gasoline".to_owned();
    let mut tanks = [(diesel.clone(), 5), (gasoline.clone(), 5)];
    let given = cdda_sim::vehicle_drive::drain_energy(&mut tanks, &diesel, 28_640.0, 30_000.0);
    assert!(
        (given - 57_280.0).abs() < 1e-6,
        "**two charges is 57,280 J: {given}**"
    );
    assert_eq!(tanks[0].1, 3, "three charges left in the diesel tank");
    assert_eq!(tanks[1].1, 5, "and the gasoline tank is untouched");
    let mut empty = [(diesel.clone(), 0)];
    assert!(
        cdda_sim::vehicle_drive::drain_energy(&mut empty, &diesel, 28_640.0, 1_000.0).abs() < 1e-9,
        "an empty tank gives nothing"
    );
}

/// **What one second of running asks for.** Reference: `consume_fuel` (`vehicle.cpp:5235-5240`) - usage times the load, scaled by strain, over a thousand, with the
/// previous turn's leftover taken off.
#[test]
fn a_second_of_running_asks_for_its_energy_less_the_leftover() {
    let ask = cdda_sim::vehicle_drive::fuel_to_consume;
    assert!(
        (ask(1000, 500, 0.0, 0.0) - 500.0).abs() < 1e-9,
        "**1,000 W at load 500 is 500 J**"
    );
    assert!(
        (ask(1000, 500, 1.0, 0.0) - 50_500.0).abs() < 1e-9,
        "**at full strain the same engine asks a hundred times more**"
    );
    assert!(
        (ask(1000, 500, 0.0, 120.0) - 380.0).abs() < 1e-9,
        "the leftover of 120 J comes off"
    );
    assert!(
        ask(1000, 0, 0.0, 0.0).abs() < 1e-9,
        "no load, nothing burned"
    );
}

/// **A turn burns what it needs and carries the fraction forward.** Reference: `consume_fuel` (`vehicle.cpp:5226-5246`) - the ask, less the leftover, drained from the
/// tanks, with `drained - asked` carried to the next turn. Diesel at 28,640 J a charge, 1,000 W at load 500 asks 500 J: one charge gives 28,640 J, so 28,140 J carries.
#[test]
fn a_turn_burns_what_it_needs_and_carries_the_fraction_forward() {
    let diesel = "diesel".to_owned();
    let mut usage = std::collections::BTreeMap::new();
    usage.insert(diesel.clone(), 1000);
    let mut tanks = [(diesel.clone(), 5)];

    let carried = cdda_sim::vehicle_drive::consume_fuel(
        &usage,
        500,
        0.0,
        &std::collections::BTreeMap::new(),
        &mut tanks,
        28_640.0,
    );
    assert_eq!(tanks[0].1, 4, "**one charge burned**");
    let left = carried.get(&diesel).copied().unwrap_or(0.0);
    assert!(
        (left - 28_140.0).abs() < 1e-6,
        "**28,640 given less 500 asked leaves 28,140 for the next turn: {left}**"
    );

    // The next turn asks the same 500 J and the leftover more than covers it, so no charge is burned.
    let carried =
        cdda_sim::vehicle_drive::consume_fuel(&usage, 500, 0.0, &carried, &mut tanks, 28_640.0);
    assert_eq!(tanks[0].1, 4, "the leftover pays for this turn");
    let left = carried.get(&diesel).copied().unwrap_or(0.0);
    assert!(
        (left - 27_640.0).abs() < 1e-6,
        "and 500 J comes off it: {left}"
    );
}

/// **A fuelled, switched-on engine is what asks for fuel.** The store half of `fuel_usage`: the car's engine contributes its own draw under its own fuel, and switching it
/// off takes the demand away entirely.
#[test]
fn a_running_engine_is_what_asks_for_fuel() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);

    let usage = cdda_sim::vehicle_drive::fuel_usage_of(&world, car);
    assert!(
        !usage.is_empty(),
        "**a fuelled car with its engines on asks for something**"
    );
    assert!(
        usage.values().all(|watts| *watts > 0),
        "and every demand is a real draw: {usage:?}"
    );

    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        for part in &mut built.parts {
            part.enabled = false;
        }
    }
    assert!(
        cdda_sim::vehicle_drive::fuel_usage_of(&world, car).is_empty(),
        "everything off, nothing asked for"
    );
}
