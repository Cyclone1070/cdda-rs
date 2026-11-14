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
    use cdda_sim::vehicles::{install_part, parts_at, remove_part, Vehicle};

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
    use cdda_sim::vehicles::{board, unboard, Riding};

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
        Some(Riding { vehicle, part: seat }),
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
    use cdda_sim::vehicles::{board, damage, remove_part, Riding};

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
        Some(Riding { vehicle, part: seat })
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
    use cdda_sim::vehicles::{move_cost_at, open_or_close, Vehicle};

    let (mut world, vehicle) = world_with_the_helicopter();
    let door = part_index(&world, vehicle, -1, -1, "door").expect("the door");
    assert_eq!(
        move_cost_at(&world, (9, 9, 0)),
        Some(0),
        "a closed door is an obstacle"
    );
    assert!(open_or_close(&mut world, vehicle, door, true), "the door opens");
    assert!(
        world.get::<Vehicle>(vehicle).expect("a vehicle").parts[door].open,
        "the part carries its own state"
    );
    assert_eq!(
        move_cost_at(&world, (9, 9, 0)),
        Some(8),
        "**an open door stops blocking**"
    );
    assert!(open_or_close(&mut world, vehicle, door, false), "and closes");
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
    assert_eq!(move_cost_at(&world, (11, 10, 0)), Some(0), "so it still blocks");
}

#[test]
fn a_placed_vehicle_stands_where_the_reference_places_it() {
    use cdda_sim::vehicles::{part_squares, spawn_facing, Vehicle};

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
    assert_eq!(placed.len(), 71, "the reference placed the helicopter's 71 parts");

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
            assert!(spec.fuel_opts.len() >= 2, "{} is multi-fuel", placement.part);
        }
    }
    assert!(stores >= 3, "the helicopter's tanks are fuel stores: {stores}");
    assert!(batteries >= 1, "and it carries a battery: {batteries}");
    assert_eq!(engines, 2, "the helicopter has two engines");
}

#[test]
fn a_vehicle_says_how_much_fuel_of_a_type_it_carries() {
    use cdda_sim::vehicles::{damage, fuel_left, spawn_facing, Vehicle};

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
    use cdda_sim::vehicles::{spawn_facing, Vehicle};

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
    use cdda_sim::vehicles::{part_spec, Vehicle};

    let tanks: Vec<usize> = {
        let built = world.get::<Vehicle>(vehicle).expect("a vehicle");
        built
            .parts
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                part_spec(world, vehicle, *index).is_some_and(|spec| spec.fuel_store && !spec.battery)
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
    use cdda_sim::vehicles::{damage, engine_fuel_left, fuel_current, is_engine_on, start_engine, Vehicle};

    let (mut world, vehicle) = fueled_helicopter();
    let engine = part_index(&world, vehicle, -2, -1, "engine_turbine_small").expect("the engine");
    assert!(!is_engine_on(&world, vehicle, engine), "an engine starts off");

    assert!(
        start_engine(&mut world, vehicle, engine, true),
        "**the tanks hold 300 units of its fuel, so it starts**"
    );
    assert!(is_engine_on(&world, vehicle, engine), "and it reads as on");
    assert_eq!(fuel_current(&world, vehicle, engine), "gasoline");
    assert_eq!(engine_fuel_left(&world, vehicle, engine), 300, "its fuel is the three full tanks");

    assert!(
        !start_engine(&mut world, vehicle, engine, true),
        "**asking for the state it already has changes nothing and answers no**"
    );
    assert!(start_engine(&mut world, vehicle, engine, false), "turning it off always works");
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
    use cdda_sim::vehicles::{engine_fuel_left, fuel_current, part_spec, start_engine, Vehicle};

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

    // Diesel only: its own type is dry, so it takes the first option that is not.
    fill_tanks(&mut world, vehicle, "diesel", 100);
    assert!(start_engine(&mut world, vehicle, engine, true), "it starts on diesel");
    assert_eq!(
        fuel_current(&world, vehicle, engine),
        "diesel",
        "**the engine takes the first fuel it can burn**"
    );
    assert_eq!(engine_fuel_left(&world, vehicle, engine), 300);

    // Gasoline again: the choice is made afresh on each start, not remembered forever.
    assert!(start_engine(&mut world, vehicle, engine, false));
    fill_tanks(&mut world, vehicle, "gasoline", 100);
    assert!(start_engine(&mut world, vehicle, engine, true), "it starts on gasoline");
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
fn bicycle_with_cargo() -> (World, Entity, usize, usize, cdda_sim::ItemTypeId, cdda_sim::ItemTypeId) {
    use cdda_sim::item_registry::{id_of, set_item_types};
    use cdda_sim::vehicles::spawn_facing;

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let kinds = [cargo_kind("null", 0), cargo_kind("rock", 50), cargo_kind("anvil", 25_000)];
    set_item_types(&mut world, &kinds);
    let rock = id_of(&mut world, "rock").expect("the rock");
    let anvil = id_of(&mut world, "anvil").expect("the anvil");
    let vehicle = spawn_facing(&mut world, "bicycle", (10, 10, 0), 0.0).expect("the bicycle builds");
    let basket = part_index(&world, vehicle, -1, 0, "basketsm").expect("the basket");
    let frame = part_index(&world, vehicle, 0, 0, "xlframe").expect("the frame");
    (world, vehicle, basket, frame, rock, anvil)
}

#[test]
fn a_cargo_part_holds_only_what_fits() {
    use cdda_sim::vehicle_cargo::{
        InVehiclePart, cargo_capacity, free_volume, items_in_part, stow_item, stored_volume,
    };

    let (mut world, vehicle, basket, frame, rock, anvil) = bicycle_with_cargo();

    assert_eq!(cargo_capacity(&world, vehicle, basket), 20_000, "**a basket holds twenty litres**");
    assert_eq!(cargo_capacity(&world, vehicle, frame), 0, "a frame is not cargo and holds nothing");
    assert_eq!(free_volume(&world, vehicle, basket), 20_000);

    assert!(
        stow_item(&mut world, vehicle, basket, anvil, 1).is_none(),
        "**a part refuses what it cannot hold**"
    );
    assert!(
        stow_item(&mut world, vehicle, frame, rock, 1).is_none(),
        "and a part that is not cargo holds nothing at all"
    );
    assert_eq!(items_in_part(&world, vehicle, basket).len(), 0, "a refusal stores nothing");

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
        free_volume, items_in_part, remove_item, stow_item, stored_volume,
    };

    let (mut world, vehicle, basket, _frame, rock, _anvil) = bicycle_with_cargo();
    let stack = stow_item(&mut world, vehicle, basket, rock, 300).expect("300 rocks fit");
    let single = stow_item(&mut world, vehicle, basket, rock, 1).expect("a rock fits");
    assert_eq!(stored_volume(&world, vehicle, basket), 15_050);

    assert!(remove_item(&mut world, single), "**a held item comes back out**");
    assert_eq!(
        stored_volume(&world, vehicle, basket),
        15_000,
        "and the room it took comes back with it"
    );
    assert_eq!(items_in_part(&world, vehicle, basket).len(), 1);
    assert!(!remove_item(&mut world, single), "the same item cannot come out twice");

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
    assert_eq!(items_in_part(&world, vehicle, basket).len(), 1, "the wreck still holds it");
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
    let vehicle = spawn_facing(&mut world, "bicycle", (20, 20, 0), 0.0).expect("the bicycle builds");
    let before = occupied_squares(&world, vehicle);
    assert_eq!(before.len(), 3, "the bicycle stands on three squares");
    for square in &before {
        assert!(things_at(&world, square.0, square.1).contains(&vehicle), "the index holds it before the move");
    }

    assert!(move_vehicle(&mut world, vehicle, (1, 0)), "**a vehicle moves one square**");
    let after = occupied_squares(&world, vehicle);
    assert_eq!(after.len(), before.len(), "it stands on as many squares as before");
    for (old, new) in before.iter().zip(&after) {
        assert_eq!(
            *new,
            (old.0 + 1, old.1, old.2),
            "**every square of it moves with it**"
        );
    }
    let left: Vec<_> = before.iter().filter(|square| !after.contains(square)).collect();
    assert_eq!(left.len(), 1, "a three-square bicycle moved one square leaves one square behind");
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

    assert!(!move_vehicle(&mut world, vehicle, (0, 0)), "an empty displacement is refused");
    assert!(!move_vehicle(&mut world, vehicle, (2, 0)), "a displacement longer than a square is refused");
    assert_eq!(occupied_squares(&world, vehicle), after, "a refused move changes nothing");
}

#[test]
fn a_rider_moves_with_the_seat_it_sits_on() {
    use cdda_sim::vehicles::{Riding, board, move_vehicle, part_squares, spawn_facing};

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds");
    let saddle = part_index(&world, vehicle, 0, 0, "saddle").expect("the saddle");
    let rider = creature_at(&mut world, 30, 30);
    assert_eq!(board(&mut world, vehicle, rider), Some(saddle), "the rider takes the saddle");
    let seat_before = part_squares(&world, vehicle)[saddle];

    assert!(move_vehicle(&mut world, vehicle, (0, 1)), "the bicycle moves");
    let seat_after = part_squares(&world, vehicle)[saddle];
    assert_ne!(seat_before, seat_after, "the seat moved");
    let position = world.get::<cdda_sim::Position>(rider).expect("the rider has a square");
    assert_eq!(
        (position.x, position.y, position.z),
        seat_after,
        "**a rider is put on the seat's new square**"
    );
    assert!(
        world.get::<Riding>(rider).is_some_and(|riding| riding.vehicle == vehicle && riding.part == saddle),
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
fn collision_world(case: &cdda_json::Object, at: (i32, i32, i32)) -> (World, Entity, Option<Entity>) {
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    if case.get("terrain").is_some() {
        let id = text_of(case, "terrain");
        let code = 7;
        cdda_sim::terrain_types::set_types(&mut world, [(id.clone(), code)]);
        cdda_sim::bash::set_data(&mut world, [recorded_tile(&id)]);
        cdda_sim::square::set_terrain(&mut world, square_of(case, "terrain_at"), code);
    }
    let first = cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", at, 0.0).expect("the bicycle builds");
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
            cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", there, 0.0).expect("the obstacle builds")
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
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_collision.json");
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
        let kind = if hit { text_of(case, "type") } else { "nothing".to_owned() };

        let (world, first, blocker) = collision_world(case, at);
        let answer = first_collision(&world, first, delta);
        let part = match answer {
            None => {
                assert!(!hit, "**a {name} move hits nothing, as the reference says**");
                None
            }
            Some(Collision::Vehicle {
                part,
                target,
                target_part,
            }) => {
                assert!(hit && kind == "veh", "**the reference answers a vehicle for {name}**");
                assert_eq!(Some(target), blocker, "{name}: the vehicle it hits is the one standing there");
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
                assert!(hit && kind == "body", "**the reference answers a body for {name}**");
                assert_eq!(Some(target), blocker, "{name}: the creature it hits is the one standing there");
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
    let second = spawn_facing(&mut world, "bicycle", (42, 40, 0), 0.0).expect("the obstacle builds");
    assert!(first_collision(&world, first, (1, 0)).is_some(), "the square ahead holds the other bicycle");
    assert!(move_vehicle(&mut world, first, (1, 0)), "the move itself is not refused yet");
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
    let vehicle = spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds");
    let rider = creature_at(&mut world, 30, 30);
    cdda_sim::singletons::index_mut(&mut world).insert(rider, (30, 30, 0));
    let saddle = part_index(&world, vehicle, 0, 0, "saddle").expect("the saddle");
    assert_eq!(board(&mut world, vehicle, rider), Some(saddle), "the rider takes the saddle");

    // The trailing frame moves onto the square the rider stands on; a passenger is not debris (`vehicle_move.cpp:867-872`).
    assert_eq!(
        first_collision(&world, vehicle, (1, 0)),
        None,
        "**a passenger is not something the vehicle carrying it hits**"
    );
    assert!(unboard(&mut world, rider), "the rider gets off");
    assert_eq!(
        first_collision(&world, vehicle, (1, 0)),
        Some(Collision::Creature { part: 6, target: rider }),
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
fn bicycle_facing(world: &mut World, at: (i32, i32, i32), ahead: (i32, i32, i32), tile: cdda_content::mapdata::MapData) -> Entity {
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
            prototype.parts.retain(|placement| !placement.part.starts_with("wheel_bicycle"));
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
        cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds")
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
        cdda_sim::vehicles::spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds")
    };
    assert!(
        matches!(first_collision(&world, vehicle, (1, 0)), Some(Collision::Terrain { bashable: true, .. })),
        "**with a wheel on its mount the same flower is hit**"
    );

    // `NOCOLLIDE` never interacts with a vehicle, even when it is bashable.
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = bicycle_facing(&mut world, (30, 30, 0), (32, 30, 0), synthetic_tile("t_smoke_thing", 6, &["NOCOLLIDE"]));
    assert_eq!(first_collision(&world, vehicle, (1, 0)), None, "**a NOCOLLIDE thing never stops a vehicle**");

    // A `SHORT` thing is not hit by a `PROTRUSION` on the part's mount, though the same part hits anything taller. The spike goes into the blueprint because a
    // structure part cannot be installed beside the frame it shares a square with.
    let low = synthetic_tile("t_low_table", 6, &["SHORT"]);
    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let plain = bicycle_facing(&mut world, (30, 30, 0), (32, 30, 0), low.clone());
    assert!(
        matches!(first_collision(&world, plain, (1, 0)), Some(Collision::Terrain { bashable: true, .. })),
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
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_armour.json");
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
    let mut healthy_part: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
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
        let vehicle = spawn_facing(&mut world, &name, origin, 0.0).expect("the recorded vehicle builds");
        assert_eq!(int_of(case, "hit"), 1, "the wall is in the way in every case");
        if case.get("plates").is_some() {
            let ruined = *healthy_part.get(&name).expect("the healthy case came first");
            let mut built = world.get_mut::<Vehicle>(vehicle).expect("a vehicle");
            let part = built.parts.get_mut(ruined).expect("the part the healthy case blamed");
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
            panic!("{}: the wall is hit, not something else", text_of(case, "name"));
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
        assert_eq!(named, text_of(case, "reported"), "{}: and by name", text_of(case, "name"));
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
    let vehicle = spawn_facing(&mut world, "bicycle", (30, 30, 0), 0.0).expect("the bicycle builds");
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
