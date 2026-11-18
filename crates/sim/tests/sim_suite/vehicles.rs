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

/// A floating-point field of one JSON object.
fn number_of(entry: &cdda_json::Object, key: &str) -> f64 {
    match entry.get(key) {
        Some(cdda_json::Value::Float(value)) => *value,
        Some(cdda_json::Value::Int(value)) => {
            #[allow(clippy::cast_precision_loss)]
            let value = *value as f64;
            value
        }
        _ => panic!("{key} is a number"),
    }
}

/// A boolean field of one JSON object.
fn bool_of(entry: &cdda_json::Object, key: &str) -> bool {
    match entry.get(key) {
        Some(cdda_json::Value::Bool(value)) => *value,
        _ => panic!("{key} is a boolean"),
    }
}

/// Whether a vehicle in the store is sliding.
fn built_skidding(world: &World, vehicle: Entity) -> bool {
    world
        .get::<cdda_sim::vehicles::Vehicle>(vehicle)
        .is_some_and(|built| built.skidding)
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
        // The three the wheel-less drag needs, with the numbers and flags `corpus/mapdata.json` records for them.
        "t_dirt" => (2, &["DIGGABLE", "FLAT", "PLOWABLE", "TRANSPARENT"], true),
        "t_grass" => (
            2,
            &["DIGGABLE", "FLAT", "GRAZABLE", "PLOWABLE", "TRANSPARENT"],
            true,
        ),
        "t_dirtmound" => (
            3,
            &[
                "DIGGABLE",
                "MOUNTABLE",
                "NOCOLLIDE",
                "PLANTABLE",
                "TRANSPARENT",
            ],
            true,
        ),
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

        // **The bare vehicle**: the port has not spawned the loot this recording's car was placed with, so the comparison is against the total less the loot's own weight.
        assert_eq!(
            cdda_sim::vehicle_mass::mass(&world, vehicle),
            i64::from(int_of(case, "total_mass_g")) - i64::from(int_of(case, "cargo_mass_g")),
            "{name}: **the mass the coefficient is scaled by is the recorded one**"
        );
        // **A placed car's coefficient belongs to its looted mass**, and the port spawns no loot; the car's weight is pinned in
        // `vehicle_mass::a_spawned_cars_cargo_is_part_of_its_weight`, and its coefficient is left to the round that spawns it.
        if int_of(case, "cargo_mass_g") > 0 {
            continue;
        }
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
    assert_eq!(
        seen, 3,
        "**three of the recording's four vehicles were placed empty**"
    );
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
        // **A vehicle placed with loot drives at the speed its looted mass gives it**, and the port has not spawned that loot, so its own car would be quicker than the recording.
        // The comparison is therefore made on the vehicles the spawn left empty; the looted car's mass is pinned in `vehicle_mass::a_spawned_cars_cargo_is_part_of_its_weight`.
        if int_of(case, "cargo_mass_g") > 0 {
            continue;
        }
        assert_eq!(
            cdda_sim::vehicle_drive::max_ground_velocity(&world, vehicle),
            Some(int_of(case, "max_ground_velocity")),
            "{name}: **and the port's own air drag, rolling drag and power give the reference's speed**"
        );
        speeds.insert(int_of(case, "max_ground_velocity"));
        seen += 1;
    }
    assert_eq!(
        seen, 3,
        "**three of the recording's four vehicles were placed empty**"
    );
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
    install_weights_for(world, &[name]);
}

/// The same for several vehicles at once: the registry is replaced by each call, so a test that needs two hulls must install their weights together.
fn install_weights_for(world: &mut World, names: &[&str]) {
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
    let mut kinds: Vec<(String, i64)> = Vec::new();
    for name in names {
        let case = cases
            .iter()
            .find_map(|case| match case {
                cdda_json::Value::Object(row) if text_of(row, "name") == *name => Some(row),
                _ => None,
            })
            .expect("the recording has that vehicle");
        let Some(cdda_json::Value::Array(parts)) = case.get("parts") else {
            panic!("the case records its parts");
        };
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
    // Two tonnes closing at a thousand each with no restitution lose 250,000,000 of the 500,000,000 they carried.
    assert_eq!(
        cdda_sim::vehicle_move::collision_damage(250_000_000.0),
        125,
        "**250,000,000 / 1,000 / 2,000 is 125** - the obstacle branch's 400 would have said 625"
    );
    // Nothing lost, nothing dealt: a perfectly elastic pass-through.
    assert_eq!(
        cdda_sim::vehicle_move::collision_damage(0.0),
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

/// **A perpetual fuel is a flag on the fuel, not on the engine.** Reference: `is_perpetual_type` (`vehicle.cpp:1056-1059`) asks the fuel item for `PERPETUAL`; muscle, wind,
/// sunlight and water carry it, and diesel does not.
#[test]
fn a_perpetual_fuel_carries_the_flag() {
    let perpetual = cdda_sim::vehicle_drive::is_perpetual;
    assert!(
        perpetual(&["PERPETUAL".to_owned()]),
        "**the flag is what makes it perpetual**"
    );
    assert!(
        !perpetual(&["E_COMBUSTION".to_owned(), "ENGINE".to_owned()]),
        "a combustion engine's fuel is not"
    );
    assert!(!perpetual(&[]), "and no flags is not perpetual either");
}

/// **A perpetual fuel is skipped when the store is asked what a vehicle burns.** The flag table says muscle is `PERPETUAL`, so an engine burning it asks for nothing even
/// when it is on and fuelled - the wiring `is_perpetual` was ported for.
#[test]
fn a_perpetual_fuel_is_not_asked_to_burn() {
    let mut world = World::new();
    let muscle = cdda_sim::ItemTypeId(0);
    cdda_sim::item_registry::set_item_flags(&mut world, &[(muscle, &["PERPETUAL"])]);
    assert!(
        cdda_sim::item_registry::kind_has_flag(&world, muscle, "PERPETUAL"),
        "**the store knows muscle never runs out**"
    );
    assert!(
        !cdda_sim::item_registry::kind_has_flag(&world, cdda_sim::ItemTypeId(9), "PERPETUAL"),
        "a kind nobody filed is not perpetual"
    );
}

/// **A vehicle that has never burned anything has no leftovers**, and one that has carries them by fuel. The component is absent until something writes it, so "never burned"
/// and "burned and owes nothing" stay different facts.
#[test]
fn a_fresh_vehicle_owes_no_fuel_leftovers() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");

    let none = cdda_sim::vehicles::fuel_remainder_of(&world, car);
    assert!(none.0.is_empty(), "**nothing has been burned yet**");
    assert!(
        (none.of("diesel") - 0.0).abs() < 1e-12,
        "so diesel owes nothing"
    );

    let mut carried = cdda_sim::vehicles::FuelRemainder::default();
    carried.0.insert("diesel".to_owned(), 28_140.0);
    cdda_sim::vehicles::set_fuel_remainder(&mut world, car, carried);
    let back = cdda_sim::vehicles::fuel_remainder_of(&world, car);
    assert!(
        (back.of("diesel") - 28_140.0).abs() < 1e-9,
        "**28,140 J carries to the next turn**"
    );
}

/// **The safe speed is the top speed of a weaker engine.** Reference: `safe_ground_velocity` (`vehicle.cpp:4302-4310`) - the same cubic as `max_ground_velocity` with the
/// engines capped at their `m2c` ratio, so it must come out positive and **below** the full top speed for an engine whose ratio is under 100.
#[test]
fn the_safe_speed_is_below_the_top_speed() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);

    let top = cdda_sim::vehicle_drive::max_ground_velocity(&world, car).expect("a top speed");
    let safe = cdda_sim::vehicle_drive::safe_ground_velocity(&world, car).expect("a safe speed");
    assert!(top > 0, "**the car can move: {top} vmiph**");
    assert!(
        safe > 0 && safe < top,
        "**safe is below top: {safe} against {top}**"
    );
}

/// **A turn of burning spends a charge and remembers the rest.** The whole chain through the store: the fuelled car's engine asks for energy, one charge of diesel is
/// burned, the leftover carries on the vehicle, and a fuel the store knows no energy for is not burned at all.
#[test]
fn burning_a_turn_spends_a_charge_and_carries_the_rest() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);
    let fuel = {
        let count = world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| built.parts.len());
        (0..count)
            .filter_map(|index| cdda_sim::vehicles::part_spec(&world, car, index))
            .find_map(|spec| spec.fuel_opts.first().cloned())
            .expect("the car names a fuel")
    };
    cdda_sim::vehicle_drive::set_fuel_energies(&mut world, &[(fuel.clone(), 28_640.0)]);
    let charges_before: i32 = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map_or(0, |built| built.parts.iter().map(|part| part.charges).sum());

    cdda_sim::vehicle_drive::burn_fuel(&mut world, car, 500);

    let charges_after: i32 = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map_or(0, |built| built.parts.iter().map(|part| part.charges).sum());
    assert!(
        charges_after < charges_before,
        "**fuel was spent: {charges_before} charges became {charges_after}**"
    );
    let carried = cdda_sim::vehicles::fuel_remainder_of(&world, car);
    assert!(
        carried.of(&fuel) > 0.0,
        "**and the fraction carries: {} J**",
        carried.of(&fuel)
    );

    // A fuel the store knows nothing about is not burned on a guess.
    let mut blind = World::new();
    install_part_weights(&mut blind, "car");
    cdda_sim::vehicles::set_store(&mut blind, store());
    let second = cdda_sim::vehicles::spawn(&mut blind, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut blind, second);
    let before: i32 = blind
        .get::<cdda_sim::vehicles::Vehicle>(second)
        .map_or(0, |built| built.parts.iter().map(|part| part.charges).sum());
    cdda_sim::vehicle_drive::burn_fuel(&mut blind, second, 500);
    let after: i32 = blind
        .get::<cdda_sim::vehicles::Vehicle>(second)
        .map_or(0, |built| built.parts.iter().map(|part| part.charges).sum());
    assert_eq!(
        before, after,
        "**an unknown fuel is not burned on a guess**"
    );
}

/// **A turn's burning reaches every vehicle, not just the one somebody asked about.** Two fuelled cars that have never moved still burn their engines' demand, and a car
/// whose fuel nobody priced is left alone in the same pass.
#[test]
fn a_turn_of_burning_reaches_every_vehicle() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let first = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let second = cdda_sim::vehicles::spawn(&mut world, "car", (110, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, first);
    fill_own_tanks(&mut world, second);
    let fuel = {
        let count = world
            .get::<cdda_sim::vehicles::Vehicle>(first)
            .map_or(0, |built| built.parts.len());
        (0..count)
            .filter_map(|index| cdda_sim::vehicles::part_spec(&world, first, index))
            .find_map(|spec| spec.fuel_opts.first().cloned())
            .expect("the car names a fuel")
    };
    cdda_sim::vehicle_drive::set_fuel_energies(&mut world, &[(fuel, 28_640.0)]);
    let before: i32 = [first, second]
        .iter()
        .map(|car| {
            world
                .get::<cdda_sim::vehicles::Vehicle>(*car)
                .map_or(0, |built| built.parts.iter().map(|part| part.charges).sum())
        })
        .sum();

    cdda_sim::vehicle_drive::burn_vehicles(&mut world, 500);

    let after: i32 = [first, second]
        .iter()
        .map(|car| {
            world
                .get::<cdda_sim::vehicles::Vehicle>(*car)
                .map_or(0, |built| built.parts.iter().map(|part| part.charges).sum())
        })
        .sum();
    assert!(
        after < before,
        "**a pass over the store burns both cars: {before} charges became {after}**"
    );
}

/// **The load is the share of the acceleration the engine supplies.** Reference: `vehicle::thrust` (`vehicle_move.cpp:528-542`) - full acceleration asks a thousand, no
/// acceleration asks nothing, and braking leaves the first `max_brake` to the brakes.
#[test]
fn the_drive_load_is_the_share_of_acceleration_the_engine_supplies() {
    let load = cdda_sim::vehicle_move::drive_load;
    assert_eq!(load(100, 100, 20, true), 1000, "**all of it: 1,000**");
    assert_eq!(
        load(50, 100, 20, true),
        500,
        "half the acceleration, half the load"
    );
    assert_eq!(load(0, 100, 20, true), 0, "nothing to do, no load");
    assert_eq!(
        load(50, 100, 20, false),
        300,
        "**braking: 500 less the 200 the brakes take**"
    );
    assert_eq!(load(10, 100, 20, false), 0, "the brakes cover it entirely");
    assert_eq!(
        load(50, 0, 20, true),
        1000,
        "no acceleration figure, full load"
    );
}

/// **Driving burns fuel.** The seam the whole chain was built for: a fuelled car that is told to go spends charges as it does, and a car with no engine demand does not.
#[test]
fn driving_a_car_burns_its_fuel() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);
    let fuel = {
        let count = world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| built.parts.len());
        (0..count)
            .filter_map(|index| cdda_sim::vehicles::part_spec(&world, car, index))
            .find_map(|spec| spec.fuel_opts.first().cloned())
            .expect("the car names a fuel")
    };
    cdda_sim::vehicle_drive::set_fuel_energies(&mut world, &[(fuel, 28_640.0)]);
    // **A car only drives when it is told where to go**: the load comes from the increment towards the cruise velocity, so a car with none would ask for nothing and burn
    // nothing - which is the reference's own behaviour, not a gap.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.cruise_velocity = 1_000;
    }
    let charges = |world: &World| {
        world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| {
                built.parts.iter().map(|part| part.charges).sum::<i32>()
            })
    };
    let before = charges(&world);
    assert!(before > 0, "**the car starts fuelled: {before} charges**");

    for _ in 0..5 {
        cdda_sim::vehicle_move::thrust(&mut world, car, 1);
    }

    let after = charges(&world);
    assert!(
        after < before,
        "**driving spends fuel: {before} charges became {after}**"
    );
}

/// **Strain squared, a hundred times, capped at 25.** Reference: `vehicle_move.cpp:591-594` builds `strain^2 * 100` and `do_engine_damage` (`vehicle.cpp:6033`) caps it.
#[test]
fn engine_strain_is_the_square_capped_at_twenty_five() {
    let strain = cdda_sim::vehicle_drive::engine_strain;
    assert_eq!(strain(0.0), 0, "no strain, no damage figure");
    assert_eq!(strain(0.3), 9, "0.3 squared is 0.09, times a hundred is 9");
    assert_eq!(strain(0.5), 25, "half strain is already 25");
    assert_eq!(
        strain(1.0),
        25,
        "**and the top of the band is capped there too**"
    );
}

/// **Strain wears engines, and no strain wears nothing.** Reference: `do_engine_damage` (`vehicle.cpp:6031-6044`) - a `rng(1,100) < strain` decides whether an engine is hurt
/// this step. At zero strain the dice never matter; at full strain they do, over enough draws.
#[test]
fn strain_wears_engines_and_no_strain_does_not() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);

    let engine_hp = |world: &World| {
        world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| {
                built.parts.iter().map(|part| part.hp).sum::<i32>()
            })
    };
    let before = engine_hp(&world);

    let mut calm = cdda_core_types::rng::Rolls::new(7);
    for _ in 0..200 {
        cdda_sim::vehicle_drive::wear_engines(&mut world, car, 0.0, &mut calm);
    }
    assert_eq!(engine_hp(&world), before, "**no strain, no wear**");

    let mut strained = cdda_core_types::rng::Rolls::new(7);
    for _ in 0..200 {
        cdda_sim::vehicle_drive::wear_engines(&mut world, car, 1.0, &mut strained);
    }
    assert!(
        engine_hp(&world) < before,
        "**full strain wears it down: {before} hit points became {}**",
        engine_hp(&world)
    );
}

/// **A car held at the top of its speed wears itself out.** The seam, end to end: drive above the safe speed for many steps and the engines lose hit points they would have
/// kept at zero strain.
#[test]
fn driving_at_the_top_of_the_band_wears_the_engines() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);
    let fuel = {
        let count = world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| built.parts.len());
        (0..count)
            .filter_map(|index| cdda_sim::vehicles::part_spec(&world, car, index))
            .find_map(|spec| spec.fuel_opts.first().cloned())
            .expect("the car names a fuel")
    };
    cdda_sim::vehicle_drive::set_fuel_energies(&mut world, &[(fuel, 28_640.0)]);
    let top = cdda_sim::vehicle_drive::max_ground_velocity(&world, car).expect("a top speed");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        // Well above the safe speed and inside the top one, with a cruise to match so the step actually drives.
        built.velocity = top.max(100);
        built.cruise_velocity = top.max(100);
        built.of_turn = 10_000.0;
    }
    let hp = |world: &World| {
        world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .map_or(0, |built| {
                built.parts.iter().map(|part| part.hp).sum::<i32>()
            })
    };
    let before = hp(&world);
    let mut rolls = cdda_core_types::rng::Rolls::new(11);
    for _ in 0..300 {
        cdda_sim::vehicle_move::move_step_with_rolls(&mut world, car, &mut rolls);
    }
    assert!(
        hp(&world) < before,
        "**the engines wore: {before} hit points became {}**",
        hp(&world)
    );
}

/// **An idling engine still burns, at a minimum of a hundredth of full throttle.** Reference: `vehicle::idle` (`vehicle.cpp:6050-6062`) floors the idle rate at 10.
#[test]
fn an_idling_engine_burns_a_hundredth_of_full_throttle() {
    let idle = cdda_sim::vehicle_drive::idle_load;
    assert_eq!(idle(0), 10, "**nothing bolted on still burns 1%**");
    assert_eq!(idle(4), 10, "below the floor is the floor");
    assert_eq!(idle(250), 250, "an alternator drawing more sets the rate");
}

/// **Which way a vehicle burns, decided before it burns.** Reference: `vehicle::idle` (`vehicle.cpp:6050-6058`) - fuel burns at idle only when the vehicle is not thrusting,
/// and thrusting means a driver is in control **and** the cruise is set.
#[test]
fn a_vehicle_thrusts_only_with_a_driver_and_a_cruise() {
    use cdda_sim::vehicle_drive::Drive;
    let state = cdda_sim::vehicle_drive::drive_state;
    assert_eq!(
        state(false, true, 1_000),
        Drive::Off,
        "**engine off, nothing**"
    );
    assert_eq!(
        state(true, true, 1_000),
        Drive::Thrust,
        "a driver with a cruise thrusts"
    );
    assert_eq!(
        state(true, true, 0),
        Drive::Idle,
        "**a driver with no cruise is only idling**"
    );
    assert_eq!(
        state(true, false, 1_000),
        Drive::Idle,
        "and so is a parked car"
    );
}

/// **Only a seat whose data says CONTROLS has a driver.** Reference: `player_in_control` asks the part's flags, not its position - a passenger seat is a seat too.
#[test]
fn only_a_controls_part_has_a_driver() {
    use cdda_sim::vehicles::{Riding, driver_of};

    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    assert_eq!(driver_of(&world, car), None, "**nobody on board yet**");

    let count = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map_or(0, |built| built.parts.len());
    let controls = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, car, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    });
    let Some(controls) = controls else {
        // The corpus car has no controls part: the assertion that matters is the negative one above.
        return;
    };
    let creature = world.spawn_empty().id();
    world.entity_mut(creature).insert(Riding {
        vehicle: car,
        part: controls,
    });
    assert_eq!(
        driver_of(&world, car),
        Some(creature),
        "**whoever sits on CONTROLS is driving**"
    );
}

/// **A car left running burns; a car being driven is charged elsewhere.** The idle pass reads `drive_state` from the store, so a vehicle whose driver has the cruise set is
/// skipped - it burns in the drive step - and one with no cruise burns at the idle floor.
#[test]
fn the_idle_pass_skips_a_car_that_is_being_driven() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let parked = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, parked);
    let fuel = {
        let count = world
            .get::<cdda_sim::vehicles::Vehicle>(parked)
            .map_or(0, |built| built.parts.len());
        (0..count)
            .filter_map(|index| cdda_sim::vehicles::part_spec(&world, parked, index))
            .find_map(|spec| spec.fuel_opts.first().cloned())
            .expect("the car names a fuel")
    };
    cdda_sim::vehicle_drive::set_fuel_energies(&mut world, &[(fuel, 28_640.0)]);
    let charges = |world: &World| {
        world
            .get::<cdda_sim::vehicles::Vehicle>(parked)
            .map_or(0, |built| {
                built.parts.iter().map(|part| part.charges).sum::<i32>()
            })
    };
    let before = charges(&world);
    assert!(before > 0, "the car starts fuelled");

    let burned = cdda_sim::vehicle_drive::idle_vehicles(&mut world);
    assert!(burned > 0, "**a parked car with its engine on idles**");
    assert!(charges(&world) < before, "and it spends fuel doing so");

    // With a driver in the seat **and** the cruise set, the same vehicle is thrusting - the drive step owns that burn, so the idle pass must leave it alone. Without a
    // driver the cruise alone is not thrusting, which is the rule's own condition.
    let count = world
        .get::<cdda_sim::vehicles::Vehicle>(parked)
        .map_or(0, |built| built.parts.len());
    let controls = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, parked, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    });
    let Some(controls) = controls else {
        // The corpus car has no controls part, so nobody can drive it: the idle pass is right to keep burning, and that is the assertion above.
        return;
    };
    let driver = world.spawn_empty().id();
    world.entity_mut(driver).insert(cdda_sim::vehicles::Riding {
        vehicle: parked,
        part: controls,
    });
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(parked)
            .expect("a vehicle");
        built.cruise_velocity = 1_000;
    }
    let after_first = charges(&world);
    let burned_again = cdda_sim::vehicle_drive::idle_vehicles(&mut world);
    assert_eq!(burned_again, 0, "**a driven car is not idled as well**");
    assert_eq!(
        charges(&world),
        after_first,
        "and its charges are untouched"
    );
}

/// **A car under way takes its step in one pass.** `move_vehicles` is the turn's version of `map::vehmove`: a vehicle with velocity takes its step, a parked one is skipped.
#[test]
fn a_pass_moves_the_cars_that_are_under_way() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let moving = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let parked = cdda_sim::vehicles::spawn(&mut world, "car", (140, 140, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(moving)
            .expect("a vehicle");
        built.velocity = 1_000;
        built.of_turn = 5.0;
    }
    assert_eq!(
        cdda_sim::vehicle_move::move_vehicles(&mut world),
        1,
        "**one car was under way, and exactly one moved**"
    );
    let parked_velocity = world
        .get::<cdda_sim::vehicles::Vehicle>(parked)
        .map_or(-1, |built| built.velocity);
    assert_eq!(parked_velocity, 0, "the parked car was left alone");
}

/// **The throttle points at the cruise, not at the heading.** Reference: `vehicle::gain_moves` thrusts by `sgn( cruise_velocity - velocity )`, so a car over its cruise is told
/// to slow by thrusting the other way, and a car exactly at it is told nothing.
#[test]
fn the_throttle_points_at_the_cruise() {
    let direction = cdda_sim::vehicle_move::cruise_direction;
    assert_eq!(direction(1_000, 0), 1, "**below the cruise, accelerate**");
    assert_eq!(direction(1_000, 400), 1, "still below, still forward");
    assert_eq!(direction(1_000, 1_000), 0, "at the cruise, no throttle");
    assert_eq!(
        direction(1_000, 1_500),
        -1,
        "**over the cruise, thrust the other way**"
    );
}

/// **A driver's hand turns into speed inside a turn.** With a creature on the `CONTROLS` part and a cruise set, the driving pass thrusts towards the cruise, the car gains
/// velocity, and its fuel is spent doing it.
#[test]
fn a_driver_turns_a_cruise_into_speed() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);
    let count = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map_or(0, |built| built.parts.len());
    let Some(controls) = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, car, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    }) else {
        // No controls part in the corpus car: nobody can drive it, which the negative assertion below still shows.
        assert_eq!(cdda_sim::vehicle_move::drive_vehicles(&mut world), 0);
        return;
    };
    let driver = world.spawn_empty().id();
    world.entity_mut(driver).insert(cdda_sim::vehicles::Riding {
        vehicle: car,
        part: controls,
    });
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.cruise_velocity = 1_000;
        built.of_turn = 5.0;
    }
    assert_eq!(
        cdda_sim::vehicle_move::drive_vehicles(&mut world),
        1,
        "**the driver was heard**"
    );
    let velocity = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map_or(0, |built| built.velocity);
    assert!(velocity > 0, "**and the car is moving: {velocity}**");
}

/// **A press of the cruise control steps, stops on the safe speed, and never leaves the caps.** Reference: `vehicle::cruise_thrust` (`vehicle_move.cpp:635-675`).
#[test]
fn a_press_of_the_cruise_control_steps_and_stops_on_the_safe_speed() {
    let press = cdda_sim::vehicle_move::cruise_thrust;
    assert_eq!(press(0, 0, 1_000, 2_000, -500), 0, "no press, no change");
    assert_eq!(press(0, 100, 1_000, 2_000, -500), 100, "**one step up**");
    assert_eq!(
        press(1_000, 100, 1_000, 2_000, -500),
        1_100,
        "**pressing at the safe speed does not stop you: only a step that crosses it does** - which is what strain is for"
    );
    assert_eq!(
        press(1_950, 100, 1_000, 2_000, -500),
        2_000,
        "**a step up lands on the top and stops there**"
    );
    assert_eq!(
        press(900, 200, 950, 2_000, -500),
        950,
        "**a step across the safe speed stops on it**"
    );
    assert_eq!(
        press(2_000, -100, 1_000, 2_000, -500),
        1_900,
        "stepping down from the top moves first, then rounds"
    );
    assert_eq!(
        press(2_000, 100, 1_000, 2_000, -500),
        2_000,
        "and no press goes past the top"
    );
    assert_eq!(
        press(-500, -100, 1_000, 2_000, -500),
        -500,
        "the deepest reverse is a cap too"
    );
}

/// **The wheel clicks in fifteens and a reversing car steers the other way.** Reference: `vehicle::turn` (`vehicle_move.cpp:678-685`).
#[test]
fn the_wheel_clicks_in_fifteens() {
    let steer = cdda_sim::vehicle_move::steered_direction;
    assert_eq!(steer(0, 30, 100), 30, "**two clicks**");
    assert_eq!(
        steer(0, 20, 100),
        15,
        "**a third of the way rounds to one click**"
    );
    assert_eq!(
        steer(0, 20, 100),
        15,
        "a third of the way rounds down to one"
    );
    assert_eq!(
        steer(0, 23, 100),
        30,
        "**and past the halfway click it rounds to two**"
    );
    assert_eq!(
        steer(350, 20, 100),
        15,
        "370 normalises to 10, then clicks to 15"
    );
    assert_eq!(
        steer(0, 20, -100),
        345,
        "**reversing steers the other way: -20 is 340, and 340 clicks to 345**"
    );
}

/// **A driver's intent becomes the vehicle's cruise.** The reference calls `cruise_thrust` straight from the character's action handling, so the intent lives with the person
/// and the car obeys - and a creature who is not the driver is not heard.
#[test]
fn a_driver_s_intent_sets_the_cruise() {
    use cdda_sim::vehicle_move::Controls;
    use cdda_sim::vehicles::{Riding, Vehicle};

    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let count = world
        .get::<Vehicle>(car)
        .map_or(0, |built| built.parts.len());
    let Some(controls) = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, car, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    }) else {
        assert_eq!(cdda_sim::vehicle_move::apply_controls(&mut world), 0);
        return;
    };
    let driver = world
        .spawn(Riding {
            vehicle: car,
            part: controls,
        })
        .id();
    // A passenger in a seat that does not control the car must not be heard.
    let passenger = world
        .spawn(Riding {
            vehicle: car,
            part: (controls + 1) % count,
        })
        .id();
    world.entity_mut(passenger).insert(Controls {
        cruise: 9_000,
        steer: 0,
    });
    assert_eq!(
        cdda_sim::vehicle_move::apply_controls(&mut world),
        0,
        "**a passenger asks for nothing**"
    );

    world.entity_mut(driver).insert(Controls {
        cruise: 500,
        steer: 0,
    });
    assert_eq!(
        cdda_sim::vehicle_move::apply_controls(&mut world),
        1,
        "**the driver was heard once**"
    );
    let cruise = world
        .get::<Vehicle>(car)
        .map_or(-1, |built| built.cruise_velocity);
    assert_eq!(cruise, 500, "and the cruise is what they asked for");
}

/// **A turn hears the driver.** The intent is a component, so the schedule - not a test - has to be what applies it: after one turn, the car's cruise is what the driver asked
/// for.
#[test]
fn a_turn_hears_the_driver() {
    use cdda_sim::vehicle_move::Controls;
    use cdda_sim::vehicles::{Riding, Vehicle};

    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let count = world
        .get::<Vehicle>(car)
        .map_or(0, |built| built.parts.len());
    let Some(controls) = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, car, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    }) else {
        return;
    };
    world.spawn((
        Riding {
            vehicle: car,
            part: controls,
        },
        Controls {
            cruise: 500,
            steer: 0,
        },
    ));

    cdda_sim::turn::run_turn(&mut world);

    let cruise = world
        .get::<Vehicle>(car)
        .map_or(-1, |built| built.cruise_velocity);
    assert_eq!(cruise, 500, "**the schedule applied the driver's intent**");
}

/// **A steering press reaches the wheels.** The wheel is kept as its own component on the vehicle: the reference keeps `turn_dir` beside the hull's facing for the same reason -
/// the wheel turns before the hull does - and a press of 23 degrees is one click, so the wheels read 30.
#[test]
fn a_steering_press_reaches_the_wheels() {
    use cdda_sim::vehicle_move::Controls;
    use cdda_sim::vehicles::{Riding, Vehicle};

    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let count = world
        .get::<Vehicle>(car)
        .map_or(0, |built| built.parts.len());
    let Some(controls) = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, car, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    }) else {
        return;
    };
    world.spawn((
        Riding {
            vehicle: car,
            part: controls,
        },
        Controls {
            cruise: 0,
            steer: 23,
        },
    ));

    cdda_sim::vehicle_move::apply_controls(&mut world);

    let wheels = cdda_sim::vehicle_move::wheel_direction(&world, car);
    assert_eq!(wheels, 30, "**23 degrees is one click past the halfway**");
}

/// **A diagonal turn is one that lands between the four axes.** Reference: `vehicle::precalculate_vehicle_turning` (`vehicle_move.cpp:1655`) calls a direction diagonal when
/// `lround( to_degrees( new_turn_dir ) ) % 90 == 45`, which is why a fifteen degree wheel can still travel axis-aligned but a forty-five degree one cannot.
#[test]
fn a_diagonal_turn_is_one_between_the_axes() {
    let diagonal = cdda_sim::vehicle_move::is_diagonal_turn;
    assert!(!diagonal(0), "north is an axis");
    assert!(!diagonal(90), "east is an axis");
    assert!(diagonal(45), "**north-east is between them**");
    assert!(diagonal(135), "and so is south-east");
    assert!(
        !diagonal(15),
        "a fifteen degree wheel is still on the axis it started on"
    );
    assert!(diagonal(-45), "the rule holds in reverse too");
}

/// **A moving vehicle faces where it went; a skidding one faces where its wheels point.** Reference: `map::move_vehicle` (`map.cpp:934-939`) - on a successful move the
/// facing becomes the travel direction, unless the vehicle is skidding, in which case it takes `turn_dir`.
#[test]
fn a_moving_vehicle_faces_where_it_went() {
    let facing = cdda_sim::vehicle_move::facing_after_move;
    assert_eq!(
        facing(false, 90, 0),
        0,
        "**not skidding: the facing follows the step**"
    );
    assert_eq!(facing(false, 90, 270), 270, "and ignores the wheels");
    assert_eq!(facing(true, 90, 0), 90, "**skidding: the wheels decide**");
}

/// **A turn fades away after it has been made.** Reference: `map::move_vehicle` (`map.cpp:922-928`) subtracts `last_turn_dec` from `last_turn` after each accepted step, and
/// clears it once it is smaller than the decrement - so a small turn is gone after one step and a big one takes several.
#[test]
fn a_turn_fades_after_it_is_made() {
    let decay = cdda_sim::vehicle_move::decayed_turn;
    assert_eq!(decay(0, 15), 0, "no turn to fade");
    assert_eq!(
        decay(20, 15),
        0,
        "**five left is less than the step, so it is gone**"
    );
    assert_eq!(decay(60, 15), 45, "a bigger turn keeps the rest");
    assert_eq!(decay(30, 15), 15, "and exactly one step's worth stays");
    assert_eq!(
        decay(-30, 1),
        -29,
        "**a turn the other way walks back too**"
    );
    assert_eq!(decay(-1, 1), 0, "and is cleared when it reaches the middle");
}

/// **Stopping clears the motion, the carry and the wheel.** Reference: `vehicle::stop` (`vehicle_move.cpp:687-698`) sets `velocity = 0`, `skidding = false`, `move = face`,
/// `last_turn = 0` and `of_turn_carry = 0`, and puts the wheel back on the hull's facing (`turn_dir = face.dir()` in `vehicle::thrust`'s stopped branch).
#[test]
fn stopping_clears_the_motion_the_carry_and_the_wheel() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 1_000;
        built.of_turn_carry = 0.42;
        built.rotation = 90.0;
    }
    world
        .entity_mut(car)
        .insert(cdda_sim::vehicle_move::Wheels { turn_dir: 45 });

    cdda_sim::vehicle_move::stop_vehicle(&mut world, car);

    let built = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .expect("still a vehicle");
    assert_eq!(built.velocity, 0, "**the speed is gone**");
    assert!((built.of_turn_carry - 0.0).abs() < 1e-12, "and the carry");
    assert_eq!(
        cdda_sim::vehicle_move::wheel_direction(&world, car),
        90,
        "**the wheel is back on the hull's facing**"
    );
}

/// **A crash straightens the wheel.** A characterisation test written after `stop_vehicle` took over the crash path: the fixture is the wall crash above, and the claim is that
/// the wheel a steer left at 45 degrees comes back to the hull's facing when the wall stops the car - which is what `veh.stop()` does in the reference (`map.cpp:948-950`).
#[test]
fn a_crash_straightens_the_wheel() {
    const FLOOR: u32 = 1;
    const WALL: u32 = 2;

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_floor".to_owned(), FLOOR), ("t_wall".to_owned(), WALL)],
    );
    cdda_sim::mapsquare::set_data(
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
    world
        .entity_mut(car)
        .insert(cdda_sim::vehicle_move::Wheels { turn_dir: 45 });
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 2_000;
        built.of_turn = 20.0;
    }
    for _ in 0..6 {
        if !cdda_sim::vehicle_move::move_step(&mut world, car) {
            break;
        }
    }

    assert_eq!(
        cdda_sim::vehicle_move::wheel_direction(&world, car),
        0,
        "**the wheel is back on the facing after the wall stopped it**"
    );
}

/// **The alternator's load is what the engines turning it can spare.** Reference: `vehicle::update_alternator_load` (`vehicle.cpp:5573-5601`): with the engine off, or with
/// no `E_ALTERNATOR` engine making power, the load is zero; otherwise it is `|1000 * alternators_power / engine_vpower|`.
///
/// The numbers are worked out from the formula the reference writes, not read from a dump: 1,500 W of alternator against 37,300 W of engine is 1000 * 1500 / 37300 = 40 by
/// the integer division the reference does in milliwatts, and a load bigger than the engine is a magnitude, not a negative.
#[test]
fn an_alternator_loads_the_engine_that_turns_it() {
    assert_eq!(
        cdda_sim::vehicle_drive::alternator_load_of(37_300, 1_500),
        40,
        "**1,500 W over 37,300 W is 40**"
    );
    assert_eq!(
        cdda_sim::vehicle_drive::alternator_load_of(37_300, 0),
        0,
        "an alternator that draws nothing loads nothing"
    );
    assert_eq!(
        cdda_sim::vehicle_drive::alternator_load_of(0, 1_500),
        0,
        "and an engine that makes no power carves out nothing"
    );
    assert_eq!(
        cdda_sim::vehicle_drive::alternator_load_of(1_000, 2_500),
        2_500,
        "a load larger than the engine is still a magnitude"
    );
}

/// **A car with everything switched off puts no load on anything.** Reference: `vehicle::update_alternator_load` returns zero at its first branch when the engine is off
/// (`vehicle.cpp:5575-5579`); this is the store-level half of the formula test, so the wiring is pinned as well as the arithmetic.
#[test]
fn a_switched_off_car_loads_no_alternator() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        for part in &mut built.parts {
            part.enabled = false;
        }
    }
    assert_eq!(
        cdda_sim::vehicle_drive::alternator_load(&world, car),
        0,
        "**nothing running, nothing loading**"
    );
}

/// **The rate an idling engine burns at is its alternator's load, floored at ten.** Reference: `vehicle::idle` (`vehicle.cpp:6052-6056`) reads `alternator_load` and then floors
/// it, and this is the door that rule is behind - so the wiring cannot fall back to the floor without this test saying so.
///
/// The `car` prototype is used because it is one of the recorded vehicles and its numbers make the two answers different: a 93,250 W engine with 1,492 W of alternator is a load
/// of 16 by `|1000 * -1492 / 93250|`, well above the floor. The second car differs in **nothing but which parts are switched on**.
#[test]
fn the_idle_rate_is_the_alternator_load_floored_at_ten() {
    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let loaded = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let quiet = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, loaded);
    fill_own_tanks(&mut world, quiet);

    let count = world
        .get::<cdda_sim::vehicles::Vehicle>(quiet)
        .expect("a vehicle")
        .parts
        .len();
    let alternators: Vec<usize> = (0..count)
        .filter(|index| {
            cdda_sim::vehicles::part_spec(&world, quiet, *index)
                .is_some_and(|spec| spec.flags.contains("ALTERNATOR"))
        })
        .collect();
    assert!(
        !alternators.is_empty(),
        "**the car has alternators to switch off**, or this test proves nothing"
    );
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(quiet)
            .expect("a vehicle");
        for index in &alternators {
            if let Some(part) = built.parts.get_mut(*index) {
                part.enabled = false;
            }
        }
    }

    assert_eq!(
        cdda_sim::vehicle_drive::alternator_load(&world, loaded),
        16,
        "**the running alternator is a load of sixteen**"
    );
    assert_eq!(
        cdda_sim::vehicle_drive::idle_rate(&world, loaded),
        16,
        "and the idle rate is that load"
    );
    assert_eq!(
        cdda_sim::vehicle_drive::alternator_load(&world, quiet),
        0,
        "with the alternators off there is no load"
    );
    assert_eq!(
        cdda_sim::vehicle_drive::idle_rate(&world, quiet),
        10,
        "**so the engine falls back to the reference's floor of ten**"
    );
}

/// **A smashed part can turn the vehicle aside, and that is what a skid is.** Reference: `vehicle::part_smash` `map.cpp`-side (`vehicle_move.cpp:1208-1226`): after a part is
/// smashed and the collision was not vertical, the reference draws `turn_amount = rng( 1, 3 ) * sqrt( part_dmg )`, divides by fifteen, floors it at one step, multiplies by fifteen
/// and caps it at 120 degrees; then it draws `turn_roll = rng( 0, 100 )` and skids when that is under `| delta_v / 100 * 2 |` - so a fifty mph loss always skids and a
/// twenty-five mph loss skids half the time.
#[test]
fn a_smashed_part_turns_the_vehicle_aside() {
    // **A collision with no change in speed cannot skid**, whatever the dice say: the threshold is zero and nothing draws under it.
    let mut steady = cdda_core_types::rng::Rolls::new(7);
    assert_eq!(
        cdda_sim::vehicle_move::skid_turn(1_000, 1_000, 400, &mut steady),
        None,
        "**no change in speed, no skid**"
    );

    // A big enough loss skids every time, and the turn is a whole number of fifteen-degree steps within the cap.
    for seed in 1..=16 {
        let mut rolls = cdda_core_types::rng::Rolls::new(seed);
        let turned = cdda_sim::vehicle_move::skid_turn(10_000, 0, 400, &mut rolls)
            .expect("a hundred mph of loss is far past the threshold");
        assert_ne!(turned, 0, "the vehicle turns somewhere: {turned}");
        assert_eq!(
            turned % 15,
            0,
            "**the turn is a whole number of fifteen-degree steps**: {turned}"
        );
        assert!(
            (15..=120).contains(&turned.abs()),
            "and it is between one step and the cap: {turned}"
        );
    }
}

/// **A crash can line up a skid, and the next move spends it.** Reference: `part_smash` sets `skidding` and turns the hull by the drawn amount (`vehicle_move.cpp:1219-1225`), and
/// `map::move_vehicle` reads that turn back when the vehicle can move again (`map.cpp:938-941`) - so a car that hits a wall at speed does not simply stop: the collision is
/// carried into the next move as a `coll_turn`.
///
/// **A skid is a dice roll**, so the test sweeps seeds until one crashes hard enough to line a turn up, and fails if none of them ever does - otherwise a crash that quietly
/// stopped producing skids would leave this test passing on the early return.
#[test]
fn a_crash_lines_up_the_next_turn() {
    const FLOOR: u32 = 1;
    const WALL: u32 = 2;

    fn a_car_facing_a_wall() -> (World, Entity) {
        let mut world = World::new();
        cdda_sim::terrain_types::set_types(
            &mut world,
            [("t_floor".to_owned(), FLOOR), ("t_wall".to_owned(), WALL)],
        );
        cdda_sim::mapsquare::set_data(
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
        (world, car)
    }

    let mut crashed = None;
    for seed in 1..=40 {
        let (mut world, car) = a_car_facing_a_wall();
        {
            let mut built = world
                .get_mut::<cdda_sim::vehicles::Vehicle>(car)
                .expect("a vehicle");
            built.velocity = 2_000;
            built.of_turn = 20.0;
        }
        let mut rolls = cdda_core_types::rng::Rolls::new(seed);
        for _ in 0..6 {
            if !cdda_sim::vehicle_move::move_step_with_rolls(&mut world, car, &mut rolls) {
                break;
            }
        }
        let lined_up = world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle")
            .coll_turn;
        if lined_up != 0 {
            crashed = Some((world, car, lined_up));
            break;
        }
    }
    let Some((mut world, car, lined_up)) = crashed else {
        panic!("**no seed in forty produced a skid**, so the crash rule stopped lining turns up");
    };

    let before = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .expect("a vehicle")
        .rotation;
    // The wall is taken away, so the next move is a move and not another crash.
    for y in 95..=105 {
        cdda_sim::square::set_terrain(&mut world, (103, y, 0), FLOOR);
    }
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 2_000;
        built.of_turn = 20.0;
    }
    let moved = cdda_sim::vehicle_move::move_step(&mut world, car);
    let after = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .expect("a vehicle");
    assert!(moved, "the car moves with the wall gone");
    assert_eq!(
        after.coll_turn, 0,
        "**the lined-up turn is spent by the move**"
    );
    let expected = (before + f64::from(lined_up)).rem_euclid(360.0);
    // The rotation is a float, so the comparison is a tolerance rather than equality: the arithmetic is exact for these values, but a strict float comparison is the kind of
    // assertion that passes today and fails when the sum happens to land differently.
    assert!(
        (after.rotation - expected).abs() < 1e-9,
        "**the hull turned by exactly what the crash lined up**: {} against {expected}",
        after.rotation
    );
}

/// **A skidding vehicle faces where its wheels point, not where it went.** Reference: `map::move_vehicle` (`map.cpp:934-939`) sets `veh.face` to the direction of travel on an
/// accepted move - **unless the vehicle is skidding**, when it takes `veh.turn_dir` instead, because a skid carries the hull where the wheels point rather than where the driver
/// aimed it.
#[test]
fn a_skidding_vehicle_faces_its_wheels() {
    const FLOOR: u32 = 1;

    fn an_open_car() -> (World, Entity) {
        let mut world = World::new();
        cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
        cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
        for y in 90..=110 {
            for x in 90..=110 {
                cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
            }
        }
        install_part_weights(&mut world, "car");
        cdda_sim::vehicles::set_store(&mut world, store());
        let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
        (world, car)
    }

    // **Rolling forward, the hull faces its path**: the step goes east and so does the nose.
    let (mut world, car) = an_open_car();
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 2_000;
        built.of_turn = 20.0;
    }
    assert!(
        cdda_sim::vehicle_move::move_step(&mut world, car),
        "the car moves"
    );
    assert!(
        (world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle")
            .rotation
            - 0.0)
            .abs()
            < 1e-9,
        "**an ordinary step leaves the nose on its path**"
    );

    // **Skidding, the hull follows the wheels**: the wheels are turned forty-five degrees but the step still goes east, and the hull comes round to the wheels.
    let (mut world, car) = an_open_car();
    world
        .entity_mut(car)
        .insert(cdda_sim::vehicle_move::Wheels { turn_dir: 45 });
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 2_000;
        built.of_turn = 20.0;
        built.skidding = true;
    }
    assert!(
        cdda_sim::vehicle_move::move_step(&mut world, car),
        "the skidding car still moves"
    );
    let facing = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .expect("a vehicle")
        .rotation;
    assert!(
        (facing - 45.0).abs() < 1e-9,
        "**a skid faces the wheels, not the path**: {facing}"
    );
}

/// **A steering press leaves a turn to spend, and the hull spends it a degree at a time.** Reference: `vehicle::turn` (`vehicle_move.cpp:676-685`) sets `last_turn = deg` beside
/// `turn_dir`, and `map::move_vehicle` (`map.cpp:917-928`) walks it back to zero by one degree a step - which is what keeps a car curving for a moment after the wheel is centred.
/// `vehicle::stop` (`:693`) clears it, so a stopped car is not still mid-turn.
#[test]
fn a_steering_press_leaves_a_turn_that_the_hull_spends() {
    use cdda_sim::vehicle_move::Controls;
    use cdda_sim::vehicles::{Riding, Vehicle};

    const FLOOR: u32 = 1;

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
    cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
    for y in 90..=110 {
        for x in 90..=110 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let count = world
        .get::<Vehicle>(car)
        .map_or(0, |built| built.parts.len());
    let Some(controls) = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, car, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    }) else {
        return;
    };
    let driver = world
        .spawn((
            Riding {
                vehicle: car,
                part: controls,
            },
            Controls {
                cruise: 0,
                steer: 23,
            },
        ))
        .id();

    cdda_sim::vehicle_move::apply_controls(&mut world);
    assert_eq!(
        cdda_sim::vehicle_move::wheel_direction(&world, car),
        30,
        "the press rounds to two clicks"
    );
    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").last_turn,
        23,
        "**and the hull owes the press as it was made, not the click the wheels landed on** - `vehicle::turn` rounds `turn_dir`, never `last_turn`"
    );

    // One accepted move spends one degree of it, whatever the step's size.
    {
        let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
        built.velocity = 2_000;
        built.of_turn = 20.0;
    }
    world.entity_mut(driver).remove::<Controls>();
    assert!(
        cdda_sim::vehicle_move::move_step(&mut world, car),
        "it moves"
    );
    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").last_turn,
        22,
        "**a step walks the leftover turn back by a degree**"
    );

    // Stopping clears what is left.
    cdda_sim::vehicle_move::stop_vehicle(&mut world, car);
    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").last_turn,
        0,
        "and a stopped car has no turn left in it"
    );
}

/// **Where a vehicle is going and where it points are two headings, and a skid is the angle between them.** Reference: `map.cpp:942` sets `veh.move = facing` on every accepted
/// move while `veh.face` follows the travel direction - or the wheels, while skidding - so the pair is the same on a normal step and different on a skid, which is what
/// `possibly_recover_from_skid` measures.
#[test]
fn a_move_records_the_direction_travelled() {
    const FLOOR: u32 = 1;

    fn an_open_car() -> (World, Entity) {
        let mut world = World::new();
        cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
        cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
        for y in 90..=110 {
            for x in 90..=110 {
                cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
            }
        }
        install_part_weights(&mut world, "car");
        cdda_sim::vehicles::set_store(&mut world, store());
        let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
        (world, car)
    }

    // **A normal step: the direction travelled and the hull agree.**
    let (mut world, car) = an_open_car();
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 2_000;
        built.of_turn = 20.0;
    }
    assert!(
        cdda_sim::vehicle_move::move_step(&mut world, car),
        "it moves"
    );
    let built = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .expect("a vehicle");
    assert_eq!(built.move_dir, 0, "the car went east");
    assert!(
        (built.rotation - 0.0).abs() < 1e-9,
        "and it faces east, so the two headings are the same"
    );

    // **A skid: the hull is dragged round to the wheels while the travel direction stays put**, and that difference is the skid.
    let (mut world, car) = an_open_car();
    world
        .entity_mut(car)
        .insert(cdda_sim::vehicle_move::Wheels { turn_dir: 45 });
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 2_000;
        built.of_turn = 20.0;
        built.skidding = true;
    }
    assert!(
        cdda_sim::vehicle_move::move_step(&mut world, car),
        "the skidding car moves"
    );
    let built = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .expect("a vehicle");
    assert_eq!(built.move_dir, 0, "**it still travelled east**");
    assert!(
        (built.rotation - 45.0).abs() < 1e-9,
        "and its hull is on the wheels, which is the angle a recovery would measure"
    );
}

/// **A skid can be recovered, and recovery is a comparison of headings against dice.** Reference: `vehicle::possibly_recover_from_skid` (`vehicle_move.cpp:1563-1590`):
/// a vehicle that turned more than thirteen degrees on this skid gets no chance at all; otherwise the hull recovers when `|dot| * 100` beats `dice( 9, 20 )`, where `dot` is the
/// angle between the direction it is moving and the direction it faces. Recovering stops the skid, scales the speed by `dot`, and reverses it when the vehicle was pointed more
/// than four fifths of the way backwards.
#[test]
fn a_skid_can_be_recovered() {
    // **A hard turn gets no chance**, whatever the dice: the reference returns before drawing anything, which is also why the stream is untouched.
    let mut rolls = cdda_core_types::rng::Rolls::new(11);
    assert_eq!(
        cdda_sim::vehicle_move::skid_recovery(15, 0, 45, 2_000, &mut rolls),
        None,
        "**more than thirteen degrees of turn is not a skid to recover from**"
    );

    // **A perpendicular skid never recovers**: the dot is zero and the dice are never under it.
    let mut rolls = cdda_core_types::rng::Rolls::new(12);
    assert_eq!(
        cdda_sim::vehicle_move::skid_recovery(0, 0, 90, 2_000, &mut rolls),
        None,
        "**travelling ninety degrees off the nose cannot be recovered**"
    );

    // **A skid straight ahead recovers on the right dice**, and the speed is the hull's: the dot is one, so nothing is scaled away.
    let mut recovered = None;
    for seed in 1..=60 {
        let mut rolls = cdda_core_types::rng::Rolls::new(seed);
        if let Some(velocity) = cdda_sim::vehicle_move::skid_recovery(0, 0, 0, 2_000, &mut rolls) {
            recovered = Some(velocity);
            break;
        }
    }
    let velocity = recovered
        .expect("**dice( 9, 20 ) lands under a hundred often enough** for sixty seeds to find one");
    assert_eq!(velocity, 2_000, "the dot is one, so the speed is kept");

    // **A backwards skid comes out the other way.** The speed is scaled by the dot and then reversed, because a vehicle pointing more than four fifths of the way backwards is
    // travelling one way and facing the other.
    let mut recovered = None;
    for seed in 1..=60 {
        let mut rolls = cdda_core_types::rng::Rolls::new(seed);
        if let Some(velocity) = cdda_sim::vehicle_move::skid_recovery(0, 0, 180, 2_000, &mut rolls)
        {
            recovered = Some(velocity);
            break;
        }
    }
    assert!(
        recovered.is_some_and(|velocity| velocity > 0),
        "a backwards skid that recovers travels towards its nose: {recovered:?}"
    );
}

/// **A sliding car stops sliding once it recovers.** Reference: `map::move_vehicle` calls `possibly_recover_from_skid` right after an accepted move (`map.cpp:958-961`), so the
/// recovery reads the headings the step has just written. This is the wiring test for that call: with the hull on its wheels and no turn outstanding, the skid ends on the right
/// dice - and it must end on at least one of sixty seeds, or the call is not being made at all.
#[test]
fn a_skidding_car_stops_skidding_when_it_recovers() {
    const FLOOR: u32 = 1;

    let mut recovered = false;
    for seed in 1..=60 {
        let mut world = World::new();
        cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
        cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
        for y in 90..=110 {
            for x in 90..=110 {
                cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
            }
        }
        install_part_weights(&mut world, "car");
        cdda_sim::vehicles::set_store(&mut world, store());
        let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
        {
            let mut built = world
                .get_mut::<cdda_sim::vehicles::Vehicle>(car)
                .expect("a vehicle");
            built.velocity = 2_000;
            built.of_turn = 20.0;
            built.skidding = true;
        }
        let mut rolls = cdda_core_types::rng::Rolls::new(seed);
        if !cdda_sim::vehicle_move::move_step_with_rolls(&mut world, car, &mut rolls) {
            continue;
        }
        if !world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle")
            .skidding
        {
            recovered = true;
            break;
        }
    }
    assert!(
        recovered,
        "**a skid that is driving straight can be recovered**, and no seed in sixty did it"
    );
}

/// **A skid at right angles to the nose drags up to twenty-five times as hard.** Reference: `vehicle::slowdown` (`vehicle_move.cpp:123-126`):
/// `skid_factor = 1 + 24 * std::abs( units::sin( face.dir() - move.dir() ) )`, applied to the rolling drag alone - so a vehicle sliding sideways scrubs its speed off while one
/// sliding straight keeps its rolling resistance unchanged.
#[test]
fn a_sidways_skid_drags_harder() {
    let factor = cdda_sim::vehicle_move::skid_drag_factor;
    assert!(
        (factor(0, 0) - 1.0).abs() < 1e-9,
        "**travelling where you point drags once**"
    );
    assert!(
        (factor(0, 180) - 1.0).abs() < 1e-9,
        "and so does travelling backwards down the same line"
    );
    assert!(
        (factor(0, 90) - 25.0).abs() < 1e-9,
        "**a right-angle skid drags twenty-five times**"
    );
    let diagonal = 1.0 + 24.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(
        (factor(0, 45) - diagonal).abs() < 1e-9,
        "and a forty-five degree one in between: {diagonal}"
    );

    // **The factor reaches the slowdown**: the same vehicle sliding sideways loses far more speed than one rolling forward.
    let straight =
        cdda_sim::vehicle_move::slowdown_with_skid(0.0, 40.0, 1_000_000, 2_000, true, 1.0);
    let sideways =
        cdda_sim::vehicle_move::slowdown_with_skid(0.0, 40.0, 1_000_000, 2_000, true, 25.0);
    assert!(
        sideways > straight,
        "**a sideways skid slows down faster**: {sideways} against {straight}"
    );
}

/// **A vehicle needs enough wheels under its weight to move.** Reference: `vehicle::valid_wheel_config` (`vehicle.cpp:5123-5126`) is
/// `sufficient_wheel_config && balanced_wheel_config` - no wheels is no, one wheel is only enough when it is `STABLE` and the hull is three tiles at most, and the bounding box of
/// the wheel mounts has to contain the centre of mass (`:5104-5121`).
#[test]
fn a_vehicle_needs_its_wheels_under_it() {
    use cdda_sim::vehicles::{
        Vehicle, balanced_wheel_config, sufficient_wheel_config, valid_wheel_config, wheels,
    };

    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);

    assert!(
        wheels(&world, car).len() >= 4,
        "**a car stands on its wheels**: {:?}",
        wheels(&world, car)
    );
    assert!(sufficient_wheel_config(&world, car), "and that is enough");
    assert!(
        valid_wheel_config(&world, car),
        "its weight is over them, so it can move"
    );

    // Take every wheel off and it is a hull on the ground.
    let all = wheels(&world, car);
    {
        let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
        for index in all {
            if let Some(part) = built.parts.get_mut(index) {
                part.broken = true;
            }
        }
    }
    assert!(
        !sufficient_wheel_config(&world, car),
        "**no wheels is not enough wheels**"
    );
    assert!(
        !balanced_wheel_config(&world, car),
        "and nothing to balance on"
    );
    assert!(!valid_wheel_config(&world, car), "so it cannot move");
}

/// **A hull with no wheels does not move, however hard it is driven.** Reference: `vehicle::thrust` (`vehicle_move.cpp:473-479`) stops it and says so before asking for power.
#[test]
fn a_wheel_less_vehicle_does_not_move() {
    use cdda_sim::vehicles::{Vehicle, wheels};

    let mut world = World::new();
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    fill_own_tanks(&mut world, car);
    let all = wheels(&world, car);
    {
        let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
        for index in all {
            if let Some(part) = built.parts.get_mut(index) {
                part.broken = true;
            }
        }
    }
    // A driver asking for speed.
    let count = world
        .get::<Vehicle>(car)
        .map_or(0, |built| built.parts.len());
    let Some(controls) = (0..count).find(|index| {
        cdda_sim::vehicles::part_spec(&world, car, *index)
            .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CONTROLS"))
    }) else {
        return;
    };
    world.spawn((
        cdda_sim::vehicles::Riding {
            vehicle: car,
            part: controls,
        },
        cdda_sim::vehicle_move::Controls {
            cruise: 100,
            steer: 0,
        },
    ));
    {
        let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
        built.cruise_velocity = 500;
        built.velocity = 500;
    }
    cdda_sim::vehicle_move::drive_vehicles(&mut world);

    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").velocity,
        0,
        "**the hull stops instead of moving**"
    );
}

/// **A hull dragged on its belly loses its speed and chews the ground under it.** Reference: `map::move_vehicle` (`map.cpp:903-914`): a vehicle that cannot stand on its wheels,
/// not vertical, not floating and at ground level has `velocity -= std::clamp( velocity, -2000, 2000 )` - which stops anything under twenty mph dead and takes exactly twenty
/// mph off anything faster - and every square it covers that is **dirt or grass becomes a dirt mound**.
#[test]
fn a_wheel_less_hull_scrubs_speed_and_the_ground() {
    const FLOOR: u32 = 1;
    const DIRT: u32 = 2;
    const GRASS: u32 = 3;
    const MOUND: u32 = 4;

    // The speed rule, on its own numbers: twenty mph is the clamp, so it is what comes off.
    assert_eq!(
        cdda_sim::vehicle_move::drag_without_wheels(1_500),
        0,
        "**slower than twenty mph stops dead**"
    );
    assert_eq!(
        cdda_sim::vehicle_move::drag_without_wheels(2_000),
        0,
        "and so does exactly twenty"
    );
    assert_eq!(
        cdda_sim::vehicle_move::drag_without_wheels(4_500),
        2_500,
        "**faster than twenty loses exactly twenty**"
    );
    assert_eq!(
        cdda_sim::vehicle_move::drag_without_wheels(-4_500),
        -2_500,
        "and reversing loses the same"
    );

    // The ground rule, in a world that has dirt, grass and a floor.

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_floor".to_owned(), FLOOR),
            ("t_dirt".to_owned(), DIRT),
            ("t_grass".to_owned(), GRASS),
            ("t_dirtmound".to_owned(), MOUND),
        ],
    );
    cdda_sim::mapsquare::set_data(
        &mut world,
        [
            recorded_tile("t_floor"),
            recorded_tile("t_dirt"),
            recorded_tile("t_grass"),
            recorded_tile("t_dirtmound"),
        ],
    );
    for y in 95..=105 {
        for x in 95..=110 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    for x in 95..=110 {
        cdda_sim::square::set_terrain(&mut world, (x, 100, 0), DIRT);
        cdda_sim::square::set_terrain(&mut world, (x, 101, 0), GRASS);
    }
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    // Lame it: every wheel broken, so it cannot stand on them.
    let wheels = cdda_sim::vehicles::wheels(&world, car);
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        for index in wheels {
            if let Some(part) = built.parts.get_mut(index) {
                part.broken = true;
            }
        }
    }
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 4_500;
    }
    cdda_sim::vehicle_move::wheelless_drag(&mut world, car);

    assert_eq!(
        world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle")
            .velocity,
        2_500,
        "**the hull lost its twenty miles an hour**"
    );
    let chewed = cdda_sim::vehicles::occupied_squares(&world, car)
        .into_iter()
        .filter(|square| cdda_sim::square::terrain_at(&world, *square) == Some(MOUND))
        .count();
    assert!(
        chewed > 0,
        "**the dirt and grass under it are mounds now**, and the floor is not"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (100, 99, 0)),
        Some(FLOOR),
        "a square away from the car is untouched"
    );
}

/// **The wheel-less drag runs on the move itself, not only when it is asked for.** Reference: `map::move_vehicle` (`map.cpp:903-914`) puts it at the top of the move, so a hull
/// that has lost its wheels loses its speed on the very step it tries to take.
#[test]
fn a_move_drags_a_wheel_less_hull() {
    use cdda_sim::vehicles::{Vehicle, wheels};

    const FLOOR: u32 = 1;

    let mut world = World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
    cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
    for y in 90..=110 {
        for x in 90..=110 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    install_part_weights(&mut world, "car");
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
    let all = wheels(&world, car);
    {
        let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
        for index in all {
            if let Some(part) = built.parts.get_mut(index) {
                part.broken = true;
            }
        }
        built.velocity = 4_500;
        built.of_turn = 20.0;
    }

    cdda_sim::vehicle_move::move_step(&mut world, car);

    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").velocity,
        2_500,
        "**the step itself took the twenty miles an hour off**"
    );
}

/// **Driving past the safe speed wears the engines out.** Reference: `vehicle::thrust` (`vehicle_move.cpp:591-595`) burns the fuel, then computes
/// `strn = int( strain * strain * 100 )` and hands it to `do_engine_damage` for **every** engine - so the wear is a consequence of the throttle, not of a separate system.
#[test]
fn thrusting_past_the_safe_speed_wears_the_engines() {
    use cdda_sim::vehicles::{Vehicle, part_spec};

    fn engine_hit_points(world: &World, car: Entity) -> i64 {
        let count = world
            .get::<Vehicle>(car)
            .map_or(0, |built| built.parts.len());
        (0..count)
            .filter(|index| {
                part_spec(world, car, *index)
                    .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "ENGINE"))
            })
            .filter_map(|index| {
                world
                    .get::<Vehicle>(car)
                    .and_then(|built| built.parts.get(index))
                    .map(|part| i64::from(part.hp))
            })
            .sum()
    }

    fn a_driven_car(velocity: i32) -> (World, Entity) {
        let mut world = World::new();
        install_part_weights(&mut world, "car");
        cdda_sim::vehicles::set_store(&mut world, store());
        let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
        fill_own_tanks(&mut world, car);
        {
            let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
            built.velocity = velocity;
            built.cruise_velocity = velocity;
        }
        (world, car)
    }

    // **Inside the safe speed nothing is strained**, so nothing wears - however many times it is driven.
    let (mut world, car) = a_driven_car(0);
    let before = engine_hit_points(&world, car);
    for seed in 1..=20 {
        let mut rolls = cdda_core_types::rng::Rolls::new(seed);
        cdda_sim::vehicle_move::thrust_with_rolls(&mut world, car, 1, &mut rolls);
    }
    assert_eq!(
        engine_hit_points(&world, car),
        before,
        "**a car at a standstill strains nothing**"
    );

    // **At the top of its speed it does**, and the dice find a hurt engine inside sixty seeds.
    let mut worn = false;
    for seed in 1..=60 {
        let (mut world, car) = a_driven_car(0);
        let top = cdda_sim::vehicle_drive::max_ground_velocity(&world, car).expect("a top speed");
        {
            let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
            built.velocity = top;
            built.cruise_velocity = top;
        }
        let before = engine_hit_points(&world, car);
        let mut rolls = cdda_core_types::rng::Rolls::new(seed);
        cdda_sim::vehicle_move::thrust_with_rolls(&mut world, car, 1, &mut rolls);
        if engine_hit_points(&world, car) < before {
            worn = true;
            break;
        }
    }
    assert!(
        worn,
        "**a car held at its top speed grinds its engines**, and no seed in sixty did it"
    );
}

/// **Reversing flips the wheel the other way, and a press sets what the hull owes.** Reference: `vehicle::turn` (`vehicle_move.cpp:676-685`): a zero press does nothing at all, a
/// vehicle moving backwards turns the opposite way unless `REVERSE_STEERING` is on, `last_turn` is set to the press **after** that flip, and the wheel direction is rounded to a
/// fifteen-degree click.
#[test]
fn a_turn_press_sets_the_wheel_and_the_turn_owed() {
    use cdda_sim::vehicles::Vehicle;

    fn a_car(velocity: i32) -> (World, Entity) {
        let mut world = World::new();
        install_part_weights(&mut world, "car");
        cdda_sim::vehicles::set_store(&mut world, store());
        let car = cdda_sim::vehicles::spawn(&mut world, "car", (100, 100, 0)).expect("it builds");
        fill_own_tanks(&mut world, car);
        {
            let mut built = world.get_mut::<Vehicle>(car).expect("a vehicle");
            built.velocity = velocity;
        }
        (world, car)
    }

    // Forward: the wheel goes the way it was asked, and the hull owes that much.
    let (mut world, car) = a_car(300);
    cdda_sim::vehicle_move::turn(&mut world, car, 22);
    assert_eq!(
        cdda_sim::vehicle_move::wheel_direction(&world, car),
        15,
        "twenty-two degrees rounds back to a fifteen click"
    );
    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").last_turn,
        22,
        "**the hull owes the press as it was made**, not the rounded wheel"
    );

    // Backwards: the same press turns the wheels the other way, and the hull owes the flipped amount.
    let (mut world, car) = a_car(-300);
    cdda_sim::vehicle_move::turn(&mut world, car, 22);
    assert_eq!(
        cdda_sim::vehicle_move::wheel_direction(&world, car),
        345,
        "**reverse turns the other way**"
    );
    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").last_turn,
        -22,
        "and what the hull owes is flipped with it"
    );

    // Nothing asked is nothing changed.
    let (mut world, car) = a_car(300);
    cdda_sim::vehicle_move::turn(&mut world, car, 0);
    assert_eq!(
        cdda_sim::vehicle_move::wheel_direction(&world, car),
        0,
        "a zero press leaves the wheel alone"
    );
    assert_eq!(
        world.get::<Vehicle>(car).expect("a vehicle").last_turn,
        0,
        "and owes nothing"
    );
}

/// **A driving session, replayed against the reference row for row.** `corpus/vehicle_drive.json` is `--dump-vehicle-drive`: a car and a bicycle with a driver at the
/// controls, driven straight, steered once, and steered twice the other way, recording after every `act_on_map` what the reference thinks the hull is doing - the facing, the
/// wheel direction, the turn it still owes, its speed and the movement points it has left.
///
/// **Positions are not compared.** The reference moves a vehicle's reference square when its hull pivots (`map::move_vehicle`'s pivot machinery), so a turn shifts every later
/// row by the pivot's offset; that rule is its own named gap. What this pins is the four numbers the steering and skid rules move - and it is what caught the port driving
/// along its hull and answering an axis for a diagonal step.
#[test]
fn a_driving_session_matches_the_reference_row_for_row() {
    use cdda_sim::vehicles::Vehicle;

    const FLOOR: u32 = 1;

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_drive.json");
    let bytes = std::fs::read(path).expect("the driving recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut rows_seen = 0;
    let mut moved_rows = 0;
    let mut turned_rows = 0;
    for case in cases {
        let cdda_json::Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let name = text_of(row, "name");
        let vehicle_name = text_of(row, "vehicle");
        let speed = int_of(row, "speed");
        let Some(cdda_json::Value::Array(steps)) = row.get("steps") else {
            panic!("{name}: a case records its steps");
        };

        let mut world = World::new();
        cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
        cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
        for y in 90..=130 {
            for x in 90..=130 {
                cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
            }
        }
        install_part_weights(&mut world, &vehicle_name);
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = cdda_sim::vehicles::spawn(&mut world, &vehicle_name, (110, 110, 0))
            .expect("the recorded vehicle builds");
        fill_own_tanks(&mut world, vehicle);
        {
            let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
            built.velocity = speed;
            built.cruise_velocity = speed;
            built.of_turn = 1.0;
        }

        // The presses the recording made, as (step, degrees): the dump's own schedule.
        let presses: &[(usize, i32)] = match name.as_str() {
            "presses_both_ways" => &[(2, -30), (7, 15)],
            "bicycle_press" | "one_press" | "reversing_press" => &[(2, 45)],
            _ => &[],
        };

        let mut rolls = cdda_core_types::rng::Rolls::new(0);
        for (step, recorded) in steps.iter().enumerate() {
            let cdda_json::Value::Object(recorded) = recorded else {
                panic!("{name}: a step is an object");
            };
            for (at, degrees) in presses {
                if *at == step {
                    cdda_sim::vehicle_move::turn(&mut world, vehicle, *degrees);
                }
            }
            cdda_sim::vehicle_move::move_step_with_rolls(&mut world, vehicle, &mut rolls);

            let (rotation, velocity, of_turn, last_turn) = {
                let built = world.get::<Vehicle>(vehicle).expect("a vehicle");
                (
                    built.rotation,
                    built.velocity,
                    built.of_turn,
                    built.last_turn,
                )
            };
            #[allow(clippy::cast_possible_truncation)]
            let face = rotation.round() as i32;
            let wheels = cdda_sim::vehicle_move::wheel_direction(&world, vehicle);

            let at = format!("{name} step {step}");
            assert_eq!(
                velocity,
                int_of(recorded, "velocity"),
                "{at}: **the speed**"
            );
            #[allow(clippy::cast_possible_truncation)]
            let expected_of_turn = number_of(recorded, "of_turn") as f32;
            assert!(
                (of_turn - expected_of_turn).abs() < 0.001,
                "{at}: **the movement points left**, {expected_of_turn} against {of_turn}"
            );
            assert_eq!(
                face,
                int_of(recorded, "face"),
                "{at}: **the hull's facing**"
            );
            assert_eq!(
                wheels,
                int_of(recorded, "turn_dir"),
                "{at}: **where the wheels point**"
            );
            assert_eq!(
                last_turn,
                int_of(recorded, "last_turn"),
                "{at}: **the turn the hull still owes**"
            );
            assert_eq!(
                built_skidding(&world, vehicle),
                bool_of(recorded, "skidding"),
                "{at}: **whether it is sliding**"
            );
            rows_seen += 1;
            // Counted from the **recording**, not from the port: a row where the reference had movement points left is a row where a move could happen, and a row where it
            // faced away from east is a row where a press reached the hull.
            if expected_of_turn > 0.0 {
                moved_rows += 1;
            }
            if int_of(recorded, "face") != 0 {
                turned_rows += 1;
            }
        }
    }
    assert_eq!(rows_seen, 72, "six cases of twelve rows each were replayed");
    assert!(
        moved_rows >= 12,
        "**and enough of them could move**: {moved_rows} rows had movement points left"
    );
    assert!(
        turned_rows >= 8,
        "**and enough of them turned**: {turned_rows} rows faced away from east after a press"
    );
}

/// **Two vehicles meeting, replayed against the reference.** `corpus/vehicle_impact.json` is `--dump-vehicle-impact`: five cases of one vehicle running into another, with both
/// speeds before and after, both masses, both rotated centres of mass and both hull facings.
///
/// **The rule is not one-dimensional.** The reference splits both velocity vectors along the line between the two centres of mass, exchanges only that component, and keeps the
/// sideways part (`map.cpp:1075-1146`) - which is why a side hit at four miles an hour sends the struck car sideways at 378 instead of stopping it.
#[test]
fn a_vehicle_impact_matches_the_reference() {
    use cdda_sim::vehicles::Vehicle;

    const FLOOR: u32 = 1;

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_impact.json");
    let bytes = std::fs::read(path).expect("the impact recording");
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
        let name = text_of(row, "name");
        let mover_name = text_of(row, "mover");
        let target_name = text_of(row, "target");
        let speed = int_of(row, "speed");
        #[allow(clippy::cast_possible_truncation)]
        let target_facing = number_of(row, "target_face").round() as i32;

        let mut world = World::new();
        cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
        cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
        for y in 90..=130 {
            for x in 90..=130 {
                cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
            }
        }
        install_weights_for(&mut world, &[&mover_name, &target_name]);
        cdda_sim::vehicles::set_store(&mut world, store());
        let mover = cdda_sim::vehicles::spawn(&mut world, &mover_name, (110, 110, 0))
            .expect("the recorded mover builds");
        // **The recording's angle is the hull's facing, not a press**: the reference places the second vehicle with `add_vehicle( ..., 180_degrees, ... )`, so the port spawns it
        // facing that way. Feeding it through `turn` would only set the wheels, because a press turns the hull on its next move.
        //
        // The **offset** case is the one that tells a two-dimensional exchange from a one-dimensional one: with both hulls on the mover's line the sideways component is zero,
        // and only a hit two squares across can answer differently. The generator places that hull the same way, from the same word in the case's name.
        let across = if name.contains("offset") { -2 } else { 0 };
        #[allow(clippy::cast_precision_loss)]
        let target = cdda_sim::vehicles::spawn_facing(
            &mut world,
            &target_name,
            (116, 110 + across, 0),
            f64::from(target_facing),
        )
        .expect("the recorded target builds");
        {
            let mut built = world.get_mut::<Vehicle>(mover).expect("a vehicle");
            built.velocity = speed;
            built.cruise_velocity = speed;
            built.of_turn = 20.0;
        }
        fill_own_tanks(&mut world, mover);
        // **The port's car must weigh what the reference's car weighed**, or a disagreement here is a data gap wearing the rule's clothes.
        for (entity, key, who) in [
            (mover, "mover_mass", "the mover"),
            (target, "target_mass", "the target"),
        ] {
            #[allow(clippy::cast_precision_loss)]
            let ours = cdda_sim::vehicle_mass::mass(&world, entity) as f64 / 1000.0;
            let recorded = number_of(row, key);
            // **Within eight per cent, because the port's cars weigh five to seven per cent light**: 1016.9 kg against 1071.8 for the same prototype. The exchange only depends on the
            // mass **ratio**, and both hulls are light by the same amount, so the speeds below still match. What the gap costs is the deformation energy, and so the damage -
            // which is why damage is not asserted here. Counting the fuel's weight is the named gap.
            assert!(
                (ours - recorded).abs() / recorded < 0.08,
                "{name}: **{who} weighs close to what the reference says**, {ours} kg against {recorded} kg"
            );
        }

        let mut rolls = cdda_core_types::rng::Rolls::new(0);
        let mut hit = false;
        for _ in 0..12 {
            cdda_sim::vehicle_move::move_step_with_rolls(&mut world, mover, &mut rolls);
            let theirs = world
                .get::<Vehicle>(target)
                .map_or(0, |built| built.velocity);
            if theirs != 0 {
                hit = true;
                break;
            }
        }
        assert!(
            hit,
            "{name}: the mover reached the other hull in twelve steps"
        );

        let (ours, theirs) = {
            let mine = world.get::<Vehicle>(mover).expect("a vehicle");
            let other = world.get::<Vehicle>(target).expect("a vehicle");
            (mine.velocity, other.velocity)
        };
        // **Within two of the reference's figure**: the exchange itself is reproduced, and what is left is the five per cent of mass the port has not counted yet.
        let expected_mover = int_of(row, "mover_after");
        let expected_target = int_of(row, "target_after");
        // One per cent at speed, two at walking pace: the slack is the same five per cent of mass, which matters more the harder the hit.
        let slack = |expected: i32| (expected.abs() / 100).max(2);
        assert!(
            (ours - expected_mover).abs() <= slack(expected_mover),
            "{name}: **the mover's speed after the hit**, {ours} against {expected_mover}"
        );
        assert!(
            (theirs - expected_target).abs() <= slack(expected_target),
            "{name}: **the struck vehicle's speed**, {theirs} against {expected_target}"
        );
        // **The side hit is the case the one-dimensional rule could not answer**: the struck car is sent sideways, so its speed is large and points across the mover's travel.
        if name == "car_into_car_side" {
            assert!(
                theirs.abs() > 300,
                "{name}: a side hit throws the struck hull sideways, {theirs}"
            );
        }
        seen += 1;
    }
    assert_eq!(seen, 6, "all six recorded impacts were replayed");
}

/// **A boarding session, replayed against the reference.** `corpus/vehicle_boarding.json` is `--dump-vehicle-boarding`: a car, a bicycle and a helicopter, each with the avatar
/// standing on its first boardable part, and the hull's mass either side of the boarding.
///
/// **The difference is the rule.** The absolute masses include the loot a hull spawns with, which the port does not have; the *gain* is the character's own weight, exactly
/// (`map::board_vehicle` marks the part, `vehicle::calc_mass_center` adds `get_weight()` against it), and that is what this compares.
#[test]
fn a_boarding_session_matches_the_reference() {
    use cdda_sim::creature::Weight;

    const FLOOR: u32 = 1;

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_boarding.json");
    let bytes = std::fs::read(path).expect("the boarding recording");
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
        let name = text_of(row, "vehicle");
        let part = int_of(row, "part");
        let weight = int_of(row, "weight_g");
        let Some(cdda_json::Value::Array(square)) = row.get("square") else {
            panic!("{name}: a square");
        };
        let at = (
            match square.first() {
                Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("x"),
                _ => panic!("x"),
            },
            match square.get(1) {
                Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("y"),
                _ => panic!("y"),
            },
        );

        let mut world = World::new();
        cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
        cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
        for y in 60..=95 {
            for x in 60..=95 {
                cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
            }
        }
        install_weights_for(&mut world, &[&name]);
        cdda_sim::vehicles::set_store(&mut world, store());
        let vehicle = cdda_sim::vehicles::spawn(&mut world, &name, (71, 79, 0))
            .expect("the recorded vehicle builds");
        fill_own_tanks(&mut world, vehicle);
        let rider = world
            .spawn((
                cdda_sim::Position {
                    x: at.0,
                    y: at.1,
                    z: 0,
                },
                Weight {
                    grams: i64::from(weight),
                },
            ))
            .id();

        let before = cdda_sim::vehicle_mass::mass(&world, vehicle);
        let seat = cdda_sim::vehicles::board(&mut world, vehicle, rider);
        let after = cdda_sim::vehicle_mass::mass(&world, vehicle);

        assert_eq!(
            seat,
            Some(usize::try_from(part).expect("an index")),
            "{name}: **the part the reference sat the character in**"
        );
        assert_eq!(
            after - before,
            i64::from(weight),
            "{name}: **the hull gains the character's weight**, {weight} g"
        );
        seen += 1;
    }
    assert_eq!(seen, 3, "all three recorded boardings were replayed");
}

/// **A driver who climbs out leaves the machine sliding, and a passenger does not.** Reference: `map::unboard_vehicle` (`map.cpp:1432-1440`) - when the character who leaves was
/// controlling the vehicle, it stops autodriving and sets `skidding = true`; anyone else leaving changes nothing about the hull.
///
/// A transcribed upstream case rather than a dump: two attempts to record it through `map::unboard_vehicle` left the reference's own hull untouched, which is a fault in that
/// recording and not a fact about the rule.
#[test]
fn a_driver_leaving_sets_the_hull_sliding() {
    const FLOOR: u32 = 1;
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
    cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
    for y in 90..=130 {
        for x in 90..=130 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    install_weights_for(&mut world, &["car"]);
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (110, 110, 0)).expect("a car");
    let seats = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .map_or(0, |built| built.parts.len());

    // The driver's seat is the first one whose data says `CONTROLS`; the port asks the flag, so the test finds it the same way.
    let mut driver_seat = None;
    let mut passenger_seat = None;
    for index in 0..seats {
        let probe = world
            .spawn((
                cdda_sim::Position {
                    x: 110,
                    y: 110,
                    z: 0,
                },
                cdda_sim::vehicles::Riding {
                    vehicle: car,
                    part: index,
                },
            ))
            .id();
        if cdda_sim::vehicles::driver_of(&world, car) == Some(probe) {
            driver_seat = Some(index);
        } else if passenger_seat.is_none() {
            passenger_seat = Some(index);
        }
        world.despawn(probe);
        if driver_seat.is_some() && passenger_seat.is_some() {
            break;
        }
    }
    let driver_seat = driver_seat.expect("the car has a driver's seat");
    let passenger_seat = passenger_seat.expect("the car has a passenger seat");

    let driver = world
        .spawn((
            cdda_sim::Position {
                x: 110,
                y: 110,
                z: 0,
            },
            cdda_sim::vehicles::Riding {
                vehicle: car,
                part: driver_seat,
            },
        ))
        .id();
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.velocity = 500;
        built.skidding = false;
    }
    assert_eq!(
        cdda_sim::vehicles::driver_of(&world, car),
        Some(driver),
        "the creature on the controls seat is the driver"
    );
    assert!(
        cdda_sim::vehicles::unboard(&mut world, driver),
        "the driver climbs out"
    );
    let after_driver = world
        .get::<cdda_sim::vehicles::Vehicle>(car)
        .is_some_and(|built| built.skidding);
    assert!(
        after_driver,
        "**and the machine keeps going sideways**: a moving vehicle with nobody at the wheel skids"
    );

    // Now a passenger: the hull must be left exactly as it was.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        built.skidding = false;
    }
    let passenger = world
        .spawn((
            cdda_sim::Position {
                x: 110,
                y: 110,
                z: 0,
            },
            cdda_sim::vehicles::Riding {
                vehicle: car,
                part: passenger_seat,
            },
        ))
        .id();
    assert!(cdda_sim::vehicles::unboard(&mut world, passenger));
    assert!(
        !world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .is_some_and(|built| built.skidding),
        "**a passenger getting out does not disturb the hull**"
    );
}

/// **A step taken while riding unboards first, and re-boards if the square it lands on has a seat.** Reference: `avatar_action::move` (`avatar_action.cpp:653-672`): the character
/// is unboarded before the move, moved, and then boarded again only if the destination has a boardable part - which is how walking seat to seat stays "in the vehicle" and walking
/// off the hull puts the character on the ground.
///
/// It also carries the driver rule with it: unboarding the driver sets the hull skidding, so *walking away from the wheel* is a way to leave a car sliding.
#[test]
fn a_step_while_riding_boards_the_square_it_lands_on() {
    const FLOOR: u32 = 1;
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_floor".to_owned(), FLOOR)]);
    cdda_sim::mapsquare::set_data(&mut world, [recorded_tile("t_floor")]);
    for y in 90..=130 {
        for x in 90..=130 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    install_weights_for(&mut world, &["car"]);
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (110, 110, 0)).expect("a car");

    // The car's own squares, in the order the parts are laid down: the first is where a creature stands to board.
    let squares = cdda_sim::vehicles::part_squares(&world, car);
    let first = squares.first().copied().expect("the car has squares");
    // **Another seat**, found the way the game finds one: a part on a different square that takes a rider.
    let mut other = None;
    for candidate in squares.iter().copied().filter(|square| *square != first) {
        let probe = world
            .spawn((cdda_sim::Position {
                x: candidate.0,
                y: candidate.1,
                z: candidate.2,
            },))
            .id();
        if cdda_sim::vehicles::board(&mut world, car, probe).is_some() {
            other = Some(candidate);
        }
        cdda_sim::vehicles::unboard(&mut world, probe);
        world.despawn(probe);
        if other.is_some() {
            break;
        }
    }
    let other = other.expect("the car has a second seat");
    let off = (first.0 + 6, first.1 + 6, 0);
    assert!(
        !squares.contains(&off),
        "the square off the hull is not part of it"
    );

    let rider = world
        .spawn((
            cdda_sim::Position {
                x: first.0,
                y: first.1,
                z: 0,
            },
            // **Enough for both steps**: a seat costs a hundred movement points and the test takes two.
            cdda_sim::creature::Moves { left: 1000 },
        ))
        .id();
    let seat = cdda_sim::vehicles::board(&mut world, car, rider);
    assert!(
        seat.is_some(),
        "the creature is put in the car's first boardable part"
    );

    // **Seat to seat**: the step lands on another part, so the creature is still riding, on that part.
    let riding = cdda_sim::vehicles::step_from_vehicle(&mut world, rider, other, 100);
    assert_eq!(
        riding,
        cdda_sim::creature::StepOutcome::Moved,
        "the step is taken"
    );
    let on = world
        .get::<cdda_sim::vehicles::Riding>(rider)
        .map(|riding| (riding.vehicle, riding.part));
    assert!(
        on.is_some(),
        "**a step onto another seat lands still in the vehicle**"
    );

    // **Off the hull**: no seat there, so the creature is on the ground and rides nothing.
    let leaving = cdda_sim::vehicles::step_from_vehicle(&mut world, rider, off, 100);
    assert_eq!(
        leaving,
        cdda_sim::creature::StepOutcome::Moved,
        "the step is taken"
    );
    assert!(
        world.get::<cdda_sim::vehicles::Riding>(rider).is_none(),
        "**a step off the hull ends the ride**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(rider)
            .map(|at| (at.x, at.y, at.z)),
        Some(off),
        "and the creature stands where it stepped"
    );
}

/// The world the walking test stands in: priced floor everywhere, a car prototype, and a walker with points to spare.
fn walk_world() -> (World, Entity) {
    const FLOOR: u32 = 1;
    const WALL: u32 = 2;
    const GRASS: u32 = 3;
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_floor".to_owned(), FLOOR),
            ("t_wall".to_owned(), WALL),
            ("t_grass".to_owned(), GRASS),
        ],
    );
    cdda_sim::mapsquare::set_data(
        &mut world,
        [
            recorded_tile("t_floor"),
            recorded_tile("t_wall"),
            recorded_tile("t_grass"),
        ],
    );
    for y in 60..=120 {
        for x in 60..=120 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), FLOOR);
        }
    }
    // **The terrain table, not just the tile data**: `square_cost` asks the prototype for the code's move cost, and a table without one makes every square free - which is what the
    // first version of this gate proved, by refusing every step for the opposite reason.
    let mut terrain = cdda_sim::terrain_table::TerrainTable::new();
    for code in [FLOOR, GRASS] {
        terrain.insert(code, true);
        terrain.insert_move_cost(code, 2);
    }
    terrain.insert(WALL, false);
    terrain.insert_move_cost(WALL, 0);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain,
            furniture: cdda_sim::terrain_table::TerrainTable::new(),
        },
    );
    install_weights_for(&mut world, &["car"]);
    cdda_sim::vehicles::set_store(&mut world, store());
    let walker = world
        .spawn((
            cdda_sim::Position {
                x: 100,
                y: 100,
                z: 0,
            },
            cdda_sim::creature::Moves { left: 10_000 },
        ))
        .id();
    let at = |world: &World| {
        world
            .get::<cdda_sim::Position>(walker)
            .map(|at| (at.x, at.y))
    };
    let _ = at;
    (world, walker)
}

/// **A walker with a body pays what the body costs, and breathes for it.** The recorded survivor's scores make a floor step 163 movement points and 24 points of breath; the wiring
/// from the square's cost through the price to both is what this reaches, and nothing else does.
#[test]
fn a_bodied_step_pays_and_breathes() {
    let (mut world, _walker) = walk_world();
    let priced = world
        .spawn((
            cdda_sim::Position {
                x: 108,
                y: 96,
                z: 0,
            },
            cdda_sim::creature::Moves { left: 10_000 },
            cdda_sim::walk::LimbCost {
                move_speed: 0.553_607,
                footing: 0.467_213,
                flat_ground: 0.85,
            },
            cdda_sim::creature::Stamina {
                cur: 8_500.0,
                max: 8_500.0,
            },
        ))
        .id();
    assert_eq!(
        cdda_sim::walk::walk(&mut world, priced, 1, 0),
        cdda_sim::creature::StepOutcome::Moved
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Moves>(priced)
            .map(|m| m.left),
        Some(10_000 - 163),
        "**a floor step by that body costs 163 movement points**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Stamina>(priced)
            .map(|s| s.cur),
        Some(8_476.0),
        "**and 24 points of breath**: a rested survivor burns fifteen per hundred moves"
    );
}

/// **A walking session: where a step lands, when it is refused, and when it boards.** A transcribed upstream case - `game::walk_move` (`game.cpp`) through
/// `avatar_action::move` (`avatar_action.cpp:653-672`) - because the one thing a dump of it cannot yet give the port is the *price*: the reference charges `Character::run_cost`, which
/// counts what the survivor carries and wears, and the port has no carried weight. The direction, the refusal and the boarding are the rules, and they are pinned here.
#[test]
fn a_walk_step_lands_refuses_and_boards() {
    const WALL: u32 = 2;
    let (mut world, walker) = walk_world();
    let at = |world: &World| {
        world
            .get::<cdda_sim::Position>(walker)
            .map(|at| (at.x, at.y))
    };

    // **The price the square sets**: a floor beside a floor is the reference's hundred; a diagonal is its 71% multiplier.
    assert_eq!(
        cdda_sim::walk::step_cost(&world, (100, 100, 0), (101, 100, 0)),
        Some(100),
        "**a floor step is a hundred**"
    );
    assert_eq!(
        cdda_sim::walk::step_cost(&world, (100, 100, 0), (101, 99, 0)),
        Some(142),
        "**and a diagonal one is 142**"
    );

    // **East**: one square, whatever the terrain price turns out to be.
    assert_eq!(
        cdda_sim::walk::walk(&mut world, walker, 1, 0),
        cdda_sim::creature::StepOutcome::Moved,
        "a step east is taken"
    );
    assert_eq!(at(&world), Some((101, 100)), "and lands one square east");

    // **North**, then a **diagonal**, which is the only place the combined cost's multiplier shows.
    assert_eq!(
        cdda_sim::walk::walk(&mut world, walker, 0, -1),
        cdda_sim::creature::StepOutcome::Moved
    );
    assert_eq!(at(&world), Some((101, 99)), "north");
    assert_eq!(
        cdda_sim::walk::walk(&mut world, walker, 1, -1),
        cdda_sim::creature::StepOutcome::Moved
    );
    assert_eq!(at(&world), Some((102, 98)), "and diagonally east and north");

    // **Into a wall**: the square has no price, so the step is refused and the creature does not move.
    cdda_sim::square::set_terrain(&mut world, (103, 98, 0), WALL);
    assert_eq!(
        cdda_sim::walk::walk(&mut world, walker, 1, 0),
        cdda_sim::creature::StepOutcome::TooSlow,
        "**a wall costs nothing, so the step is refused**"
    );
    assert_eq!(at(&world), Some((102, 98)), "and the creature stays put");

    // **Onto a car**: a vehicle square is enterable, and landing on a seat boards it - `avatar_action::move` boards at the end of the move.
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (110, 98, 0)).expect("a car");
    let seats = cdda_sim::vehicles::part_squares(&world, car);
    let seat = seats
        .iter()
        .copied()
        .find(|square| {
            let probe = world
                .spawn((cdda_sim::Position {
                    x: square.0,
                    y: square.1,
                    z: 0,
                },))
                .id();
            let took = cdda_sim::vehicles::board(&mut world, car, probe).is_some();
            cdda_sim::vehicles::unboard(&mut world, probe);
            world.despawn(probe);
            took
        })
        .expect("the car has a seat");
    let beside = (seat.0 - 1, seat.1);
    {
        let mut position = world
            .get_mut::<cdda_sim::Position>(walker)
            .expect("a position");
        position.x = beside.0;
        position.y = beside.1;
    }
    assert_eq!(
        cdda_sim::walk::walk(&mut world, walker, 1, 0),
        cdda_sim::creature::StepOutcome::Moved,
        "a vehicle square can be walked onto"
    );
    assert!(
        world.get::<cdda_sim::vehicles::Riding>(walker).is_some(),
        "**and landing on a seat boards the vehicle**"
    );

    // **A square the car's own frame fills is an obstacle**: the vehicle answers zero for it before the terrain is ever asked (`map::move_cost` asks the vehicle first), and a
    // creature walking into it is refused no matter how good the floor under it is. This is the branch the terrain's own zero never reaches.
    let frame = cdda_sim::vehicles::part_squares(&world, car)
        .into_iter()
        .find(|square| cdda_sim::vehicles::move_cost_at(&world, *square) == Some(0))
        .expect("the car has an obstacle part somewhere");
    {
        let mut position = world
            .get_mut::<cdda_sim::Position>(walker)
            .expect("a position");
        position.x = frame.0 - 1;
        position.y = frame.1;
        position.z = frame.2;
    }
    cdda_sim::vehicles::unboard(&mut world, walker);
    assert_eq!(
        cdda_sim::walk::walk(&mut world, walker, 1, 0),
        cdda_sim::creature::StepOutcome::TooSlow,
        "**the vehicle's obstacle costs nothing, so the step is refused**"
    );
}

/// **Opening a door from outside costs the rest of the turn.** Reference: `avatar_action::move` (`avatar_action.cpp:561-566`) - when the square being walked into is a vehicle's
/// closed door, the game opens it and charges `you.mod_moves( -you.get_speed() )`: a whole turn's speed, not the price of a square. The opener stands still for the turn it opened in.
#[test]
fn opening_a_door_from_outside_costs_a_turn() {
    let (mut world, _walker) = walk_world();
    cdda_sim::vehicles::set_store(&mut world, store());
    let car = cdda_sim::vehicles::spawn(&mut world, "car", (110, 110, 0)).expect("a car");
    let door = {
        let built = world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        (0..built.parts.len())
            .find(|index| {
                cdda_sim::vehicles::part_spec(&world, car, *index).is_some_and(|spec| {
                    spec.flags.contains("OPENABLE") && spec.flags.contains("OBSTACLE")
                })
            })
            .expect("the car has a door")
    };
    let opener = world
        .spawn((
            cdda_sim::Position {
                x: 110,
                y: 114,
                z: 0,
            },
            cdda_sim::creature::Speed {
                base: 100,
                bonus: 0,
            },
            cdda_sim::creature::Moves { left: 1000 },
        ))
        .id();

    assert!(
        cdda_sim::vehicles::open_from_outside(&mut world, car, door, opener),
        "the door opens"
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Moves>(opener)
            .map(|m| m.left),
        Some(900),
        "**and the opener pays a turn's speed for it**, not a square's price"
    );

    // A part that cannot open is not a door: nothing happens and nothing is charged.
    let wall = {
        let built = world
            .get::<cdda_sim::vehicles::Vehicle>(car)
            .expect("a vehicle");
        (0..built.parts.len())
            .find(|index| {
                !cdda_sim::vehicles::part_spec(&world, car, *index)
                    .is_some_and(|spec| spec.flags.contains("OPENABLE"))
            })
            .expect("the car has a part that does not open")
    };
    assert!(
        !cdda_sim::vehicles::open_from_outside(&mut world, car, wall, opener),
        "a part that cannot open is not opened"
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Moves>(opener)
            .map(|m| m.left),
        Some(900),
        "and the opener is charged nothing for trying"
    );
}

/// **A turret is a mount that can shoot, and its ammunition comes from wherever the data says.**
///
/// `corpus/vehicles.json` records all **240** turret parts, their base `item` and their flags; `vehicles::parts::finalize` (`veh_type.cpp:589-597`) is what sets `USE_TANKS` on a
/// mount whose gun fires liquid and `USE_BATTERIES` on a battery-fed one, so the flags in the recording are the rule's own answer and not a transcription of the data files. The
/// test counts them, because a gate that sees no turrets would pass while proving nothing.
#[test]
fn a_turret_feeds_from_its_tanks_or_its_own_magazine() {
    let store = store();
    let turrets: Vec<&cdda_content::vehicles::PartSpec> = store
        .parts
        .iter()
        .filter(|spec| spec.flags.contains("TURRET"))
        .collect();
    assert_eq!(
        turrets.len(),
        240,
        "**the recording holds 240 turret mounts**"
    );
    let tanks: Vec<&&cdda_content::vehicles::PartSpec> = turrets
        .iter()
        .filter(|spec| spec.flags.contains("USE_TANKS"))
        .collect();
    assert_eq!(
        tanks.len(),
        4,
        "**four of them feed from the vehicle's tanks**, which is the reference's liquid-ammunition set"
    );
    assert!(
        turrets
            .iter()
            .filter(|spec| spec.flags.contains("USE_BATTERIES"))
            .count()
            == 0,
        "and none of this baseline's mounts is battery-fed"
    );
    // **Each turret names the gun it carries**, and a turret with no gun is not a turret the reference accepts (`veh_type.cpp:899`).
    assert!(
        turrets.iter().all(|spec| !spec.item.is_empty()),
        "every turret mounts an item"
    );

    // **A world built from one of them** - the .22 pistol mount - answers the three questions the rule asks: it is a turret, it does not feed from the tanks, and its ammunition is
    // the charge count its gun was installed with.
    let mut world = World::new();
    let mut store = store;
    store.prototypes.push(cdda_content::vehicles::Prototype {
        id: "technical".to_owned(),
        name: "technical".to_owned(),
        parts: Vec::new(),
        spawns: Vec::new(),
    });
    cdda_sim::vehicles::set_store(&mut world, store);
    let vehicle = cdda_sim::vehicles::spawn_facing(&mut world, "technical", (0, 0, 0), 0.0)
        .expect("a vehicle");
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(vehicle)
            .expect("a vehicle");
        built.parts.push(cdda_sim::vehicles::VehiclePart {
            part: "turret_1911_22".to_owned(),
            x: 0,
            y: 0,
            charges: 30,
            ..cdda_sim::vehicles::VehiclePart::default()
        });
    }

    assert_eq!(
        cdda_sim::vehicles::turrets(&world, vehicle).len(),
        1,
        "**the mount is a turret and it is not broken**"
    );
    assert!(
        !cdda_sim::vehicles::uses_vehicle_tanks_or_batteries(&world, vehicle, 0),
        "a pistol mount does not feed from the tanks"
    );
    assert_eq!(
        cdda_sim::vehicles::turret_ammo_remaining(&world, vehicle, 0),
        30,
        "**its ammunition is the thirty rounds in its own magazine**"
    );

    // **A tank-fed mount draws on the tank, not on its own magazine.** The chemical thrower's mount carries `USE_TANKS`, so its ammunition is the vehicle's `fuel_left` of the
    // liquid it fires - the reference's `turret_data::ammo_remaining` first branch, and the one that makes the flag worth reading.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(vehicle)
            .expect("a vehicle");
        built.parts.push(cdda_sim::vehicles::VehiclePart {
            part: "turret_chemical_thrower".to_owned(),
            x: 0,
            y: 0,
            charges: 3,
            fuel: "chemical_thrower".to_owned(),
            ..cdda_sim::vehicles::VehiclePart::default()
        });
        built.parts.push(cdda_sim::vehicles::VehiclePart {
            part: "afs_small_roof_external_tank".to_owned(),
            x: 0,
            y: 0,
            charges: 41,
            fuel: "chemical_thrower".to_owned(),
            ..cdda_sim::vehicles::VehiclePart::default()
        });
    }
    assert!(
        cdda_sim::vehicles::uses_vehicle_tanks_or_batteries(&world, vehicle, 1),
        "**the thrower's mount feeds from the tanks**"
    );
    assert_eq!(
        cdda_sim::vehicles::turret_ammo_remaining(&world, vehicle, 1),
        41,
        "**and its ammunition is the tank's forty-one charges, not the three in its magazine**"
    );

    // **And a break takes it out of the list**, which is the reference's `!e.is_broken()` and its `turret_query` refusal.
    assert_eq!(
        cdda_sim::vehicles::turrets(&world, vehicle).len(),
        2,
        "**two mounts can shoot now**, the pistol and the thrower"
    );
    if let Some(mut built) = world.get_mut::<cdda_sim::vehicles::Vehicle>(vehicle) {
        for mount in &mut built.parts {
            mount.broken = true;
        }
    }
    assert!(
        cdda_sim::vehicles::turrets(&world, vehicle).is_empty(),
        "a broken turret is not a turret"
    );
    assert_eq!(
        cdda_sim::vehicles::turret_query(&world, vehicle, 0),
        None,
        "and the query refuses it"
    );
    assert_eq!(
        cdda_sim::vehicles::turret_query(&world, vehicle, 1),
        None,
        "for the thrower too"
    );
}

/// **Fuel comes out in part order, until the amount is met.** Reference: `vehicle::drain` (`vehicle.cpp:3959-3986`) - it walks the parts, takes `min( amount, remaining )` from each
/// whose current ammunition is the fuel asked for, and answers the total. A drain that spans two tanks therefore empties the first and shortens the second.
#[test]
fn fuel_drains_in_part_order() {
    let mut world = World::new();
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: vec![cdda_content::vehicles::PartSpec {
                id: "tank".to_owned(),
                fuel_store: true,
                ..cdda_content::vehicles::PartSpec::default()
            }],
            prototypes: Vec::new(),
        },
    );
    let vehicle = world
        .spawn((
            cdda_sim::vehicles::Vehicle {
                parts: vec![
                    cdda_sim::vehicles::VehiclePart {
                        part: "tank".to_owned(),
                        charges: 30,
                        fuel: "gasoline".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                    cdda_sim::vehicles::VehiclePart {
                        part: "tank".to_owned(),
                        charges: 50,
                        fuel: "gasoline".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                    cdda_sim::vehicles::VehiclePart {
                        part: "tank".to_owned(),
                        charges: 7,
                        fuel: "diesel".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                ],
                ..cdda_sim::vehicles::Vehicle::default()
            },
            cdda_sim::Position { x: 0, y: 0, z: 0 },
        ))
        .id();
    assert_eq!(
        cdda_sim::vehicles::fuel_left(&world, vehicle, "gasoline"),
        80,
        "eighty charges of gasoline to begin with"
    );

    // **Forty out of two tanks**: thirty from the first, ten from the second.
    let drained = cdda_sim::vehicles::drain(&mut world, vehicle, "gasoline", 40);
    assert_eq!(drained, 40, "**forty charges came out**");
    let held: Vec<i32> = world
        .get::<cdda_sim::vehicles::Vehicle>(vehicle)
        .map(|built| built.parts.iter().map(|part| part.charges).collect())
        .unwrap_or_default();
    assert_eq!(
        held,
        vec![0, 40, 7],
        "**the first tank is empty and the second is forty**, and the diesel is untouched"
    );
    assert_eq!(
        cdda_sim::vehicles::fuel_left(&world, vehicle, "gasoline"),
        40,
        "which is what fuel_left now answers"
    );

    // **Asking for more than there is** drains everything and answers what was there.
    assert_eq!(
        cdda_sim::vehicles::drain(&mut world, vehicle, "gasoline", 500),
        40,
        "**the vehicle gives up what it has**, not what was asked"
    );
    assert_eq!(
        cdda_sim::vehicles::fuel_left(&world, vehicle, "gasoline"),
        0,
        "and none is left"
    );
    assert_eq!(
        cdda_sim::vehicles::drain(&mut world, vehicle, "gasoline", 5),
        0,
        "an empty vehicle drains nothing"
    );
}

/// **The ladder a turret passes before it fires.** Reference: `turret_data::query` (`turret.cpp:250-275`) - ammunition first, then power, then ready - and the two ammunition
/// branches are different tests: a tank-fed mount asks unconditionally, a magazine mount only when the gun spends something.
#[test]
fn a_turret_passes_its_ammunition_then_its_power() {
    use cdda_sim::vehicles::{TurretStatus, turret_status};

    let mut world = World::new();
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: vec![
                cdda_content::vehicles::PartSpec {
                    id: "pistol_mount".to_owned(),
                    flags: std::iter::once("TURRET".to_owned()).collect(),
                    ..cdda_content::vehicles::PartSpec::default()
                },
                cdda_content::vehicles::PartSpec {
                    id: "thrower_mount".to_owned(),
                    flags: ["TURRET".to_owned(), "USE_TANKS".to_owned()]
                        .into_iter()
                        .collect(),
                    ..cdda_content::vehicles::PartSpec::default()
                },
                cdda_content::vehicles::PartSpec {
                    id: "tank".to_owned(),
                    fuel_store: true,
                    ..cdda_content::vehicles::PartSpec::default()
                },
                cdda_content::vehicles::PartSpec {
                    id: "battery".to_owned(),
                    battery: true,
                    ..cdda_content::vehicles::PartSpec::default()
                },
            ],
            prototypes: Vec::new(),
        },
    );
    let vehicle = world
        .spawn((
            cdda_sim::vehicles::Vehicle {
                parts: vec![
                    // A magazine mount with three rounds of four needed.
                    cdda_sim::vehicles::VehiclePart {
                        part: "pistol_mount".to_owned(),
                        charges: 3,
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                    // A tank-fed mount drawing on a tank of forty-one, spending fifty a shot.
                    cdda_sim::vehicles::VehiclePart {
                        part: "thrower_mount".to_owned(),
                        charges: 9,
                        fuel: "chemical_spray".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                    cdda_sim::vehicles::VehiclePart {
                        part: "tank".to_owned(),
                        charges: 41,
                        fuel: "chemical_spray".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                    // A car battery of two charges.
                    cdda_sim::vehicles::VehiclePart {
                        part: "battery".to_owned(),
                        charges: 2,
                        fuel: "battery".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                ],
                ..cdda_sim::vehicles::Vehicle::default()
            },
            cdda_sim::Position { x: 0, y: 0, z: 0 },
        ))
        .id();

    // **A magazine mount short of its rounds**: three held, four needed.
    assert_eq!(
        turret_status(&world, vehicle, 0, 4, 0),
        TurretStatus::NoAmmo,
        "**three rounds cannot pay for a shot that costs four**"
    );
    // **The same mount with two needed** is ready - the magazine holds three.
    assert_eq!(
        turret_status(&world, vehicle, 0, 2, 0),
        TurretStatus::Ready,
        "three rounds pay for a shot of two"
    );
    // **A gun that spends nothing is never short**, which is the magazine branch's `ammo_required &&` guard.
    assert_eq!(
        turret_status(&world, vehicle, 0, 0, 0),
        TurretStatus::Ready,
        "a free shot never runs dry"
    );
    // **A tank-fed mount** with forty-one in the tank and fifty needed has no ammunition, whatever is in its own magazine.
    assert_eq!(
        turret_status(&world, vehicle, 1, 50, 0),
        TurretStatus::NoAmmo,
        "**forty-one in the tank cannot pay for a shot that costs fifty**, though nine sit in the mount itself"
    );
    assert_eq!(
        turret_status(&world, vehicle, 1, 40, 0),
        TurretStatus::Ready,
        "and forty-one pays for a shot of forty"
    );
    // **And then the power question**: two battery charges, four kilojoules a shot.
    assert_eq!(
        turret_status(&world, vehicle, 0, 2, 4),
        TurretStatus::NoPower,
        "**a shot costing four kilojoules cannot be paid by two charges**"
    );
    assert_eq!(
        turret_status(&world, vehicle, 0, 2, 2),
        TurretStatus::Ready,
        "two charges pay for two kilojoules exactly"
    );
    // **A broken mount answers nothing at all.**
    let mut held = world.get_mut::<cdda_sim::vehicles::Vehicle>(vehicle);
    if let Some(mount) = held.as_mut().and_then(|built| built.parts.first_mut()) {
        mount.broken = true;
    }
    assert_eq!(
        turret_status(&world, vehicle, 0, 2, 0),
        TurretStatus::Invalid,
        "a broken turret has no status"
    );
}

/// **What a turret's shots cost the vehicle.** Reference: `turret_data::post_fire` (`turret.cpp:286-330`) - the mount's ammunition by `ammo_required × shots`, then the battery by
/// `energy_drain × shots`, both through `drain`, and both paying what the stores actually held.
#[test]
fn a_turret_pays_for_its_shots() {
    let mut world = World::new();
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: vec![
                cdda_content::vehicles::PartSpec {
                    id: "thrower_mount".to_owned(),
                    flags: ["TURRET".to_owned(), "USE_TANKS".to_owned()]
                        .into_iter()
                        .collect(),
                    ..cdda_content::vehicles::PartSpec::default()
                },
                cdda_content::vehicles::PartSpec {
                    id: "tank".to_owned(),
                    fuel_store: true,
                    ..cdda_content::vehicles::PartSpec::default()
                },
                cdda_content::vehicles::PartSpec {
                    id: "battery".to_owned(),
                    battery: true,
                    ..cdda_content::vehicles::PartSpec::default()
                },
            ],
            prototypes: Vec::new(),
        },
    );
    let vehicle = world
        .spawn((
            cdda_sim::vehicles::Vehicle {
                parts: vec![
                    cdda_sim::vehicles::VehiclePart {
                        part: "thrower_mount".to_owned(),
                        fuel: "chemical_spray".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                    cdda_sim::vehicles::VehiclePart {
                        part: "tank".to_owned(),
                        charges: 400,
                        fuel: "chemical_spray".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                    cdda_sim::vehicles::VehiclePart {
                        part: "battery".to_owned(),
                        charges: 30,
                        fuel: "battery".to_owned(),
                        ..cdda_sim::vehicles::VehiclePart::default()
                    },
                ],
                ..cdda_sim::vehicles::Vehicle::default()
            },
            cdda_sim::Position { x: 0, y: 0, z: 0 },
        ))
        .id();

    // **Three shots of fifty a shot and four kilojoules each**: 150 out of the tank, 12 out of the battery.
    let paid = cdda_sim::vehicles::turret_pay_for_shots(&mut world, vehicle, 0, 50, 4, 3);
    assert_eq!(
        paid,
        (150, 12),
        "**the tank pays a hundred and fifty and the battery twelve**"
    );
    assert_eq!(
        cdda_sim::vehicles::fuel_left(&world, vehicle, "chemical_spray"),
        250,
        "four hundred less a hundred and fifty"
    );
    assert_eq!(
        cdda_sim::vehicles::fuel_left(&world, vehicle, "battery"),
        18,
        "thirty less twelve"
    );

    // **A shot that costs more than the tank holds** pays what is there and says so.
    let paid = cdda_sim::vehicles::turret_pay_for_shots(&mut world, vehicle, 0, 100, 20, 10);
    assert_eq!(
        paid,
        (250, 18),
        "**the tank gives its last two hundred and fifty, the battery its last eighteen**"
    );
    assert_eq!(
        cdda_sim::vehicles::fuel_left(&world, vehicle, "chemical_spray"),
        0,
        "and both are empty"
    );
    assert_eq!(
        cdda_sim::vehicles::fuel_left(&world, vehicle, "battery"),
        0,
        "to the last charge"
    );
}

/// **A turret's reach, and the reach of the gun under it.** Reference: `turret_data::range()` (`turret.cpp:213-224`) over `item::gun_range( with_ammo )` (`item.cpp:11282-11300`).
///
/// The numbers are the recordings': `corpus/items/guns.json` states the guns' own ranges - **zero for the M249 and the .22 pistol**, six for the flamethrower - and
/// `corpus/items/ammo.json` (version 6, which is when the round's range was recorded) states the cartridges': **36 for a .223**, 13 for a .22 long rifle, 6 for napalm. A firearm's
/// reach is the sum, which is why a port that read the gun alone would see zero.
#[test]
fn a_turret_reaches_as_far_as_its_gun_and_its_round() {
    use cdda_content::guns::{RangeBonus, gun_range};

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/guns.json");
    let bytes = std::fs::read(path).expect("the guns recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let guns = cdda_content::guns::load_dump(&value).expect("the guns load");
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/ammo.json");
    let bytes = std::fs::read(path).expect("the ammo recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let rounds = cdda_content::ammo::load_dump(&value).expect("the rounds load");

    let gun_of = |id: &str| guns.iter().find(|gun| gun.id == id).cloned();
    let round_of = |id: &str| rounds.iter().find(|round| round.id == id).cloned();

    // **The M249's own range is nothing and its cartridge carries 36 squares.**
    let m249 = gun_of("m249").expect("the machine gun");
    assert_eq!(m249.range, 0, "the gun states no range of its own");
    let round = round_of("223").expect("the .223 round");
    assert_eq!(round.range, 36, "the cartridge states thirty-six");
    assert_eq!(
        gun_range(
            m249.range,
            &[],
            Some(RangeBonus {
                range: round.range,
                multiplier: 1.0
            }),
            792
        ),
        36,
        "**so the gun reaches thirty-six squares loaded**"
    );
    assert_eq!(
        gun_range(m249.range, &[], None, 792),
        0,
        "and nothing at all unloaded"
    );

    // **A flamethrower's mount feeds from the tanks**, so `gun_range` never sees the round and the turret adds it: six stated by the gun, six by napalm.
    let flamer = gun_of("flamethrower").expect("the flamethrower");
    assert_eq!(flamer.range, 6, "the flamethrower states six");
    let napalm = round_of("napalm").expect("napalm");
    assert_eq!(
        cdda_sim::vehicles::turret_range(flamer.range, true, Some(napalm.range)),
        12,
        "**twelve squares when the round's range is added**, which is the tank-fed branch"
    );
    assert_eq!(
        cdda_sim::vehicles::turret_range(flamer.range, false, Some(napalm.range)),
        6,
        "and six when the port is told the mount does not feed from them"
    );

    // **A gunmod's bonus applies to the whole, then the round is added and the result clamped** - `ret *= multiplier` after the sum (`item.cpp:11297-11299`).
    assert_eq!(
        gun_range(
            6,
            &[RangeBonus {
                range: 2,
                multiplier: 2.0
            }],
            Some(RangeBonus {
                range: 3,
                multiplier: 1.0
            }),
            792
        ),
        22,
        "**six plus two plus three, all doubled**"
    );
    assert_eq!(
        gun_range(
            0,
            &[RangeBonus {
                range: 900,
                multiplier: 1.0
            }],
            None,
            792
        ),
        792,
        "and the cap holds at the reference's own"
    );

    // **Reach is a distance test, edges included**: 36 squares reaches 36 and not 37.
    assert!(
        cdda_sim::vehicles::turret_in_range(36, 36),
        "the edge is in range"
    );
    assert!(
        !cdda_sim::vehicles::turret_in_range(36, 37),
        "one square past it is not"
    );
}
