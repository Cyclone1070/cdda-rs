//! Vehicle parts and prototypes, against the reference's own list.
//!
//! `corpus/vehicles.json` version 3 is `--dump-vehicles`: every part the reference's store holds - the definitions the data writes resolved through `copy-from`, plus the
//! turret parts `finalize` invents from mountable guns - each carrying what a part *is*, and every prototype carrying the **blueprint** it is built from.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::vehicles;
use cdda_json::parse;
use std::path::Path;

fn dump() -> vehicles::Vehicles {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicles.json");
    let text = std::fs::read_to_string(path).expect("the vehicle dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    vehicles::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_vehicle_part_and_prototype() {
    let dump = dump();
    assert_eq!(dump.parts.len(), 701, "parts");
    assert_eq!(dump.prototypes.len(), 249, "prototypes");
}

#[test]
fn known_ids_are_present_and_no_id_repeats() {
    let dump = dump();
    let ids: Vec<&String> = dump.parts.iter().map(|part| &part.id).collect();
    assert!(ids.iter().any(|id| *id == "seat"), "a known part");
    assert!(
        !ids.iter().any(|id| *id == "2seater2"),
        "a prototype is not a part: the two stores are separate"
    );
    let mut sorted = ids.clone();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ids.len(),
        "no duplicates, and the dump is sorted"
    );
}

/// **A part carries what it is, not only its name.** `seat` is the reference's own example: 300 hit points, a 60% damage modifier, made of the item `seat`, in the
/// `center` slot, belt-able and boardable, and it shaves 5 off a bash before its hit points are touched.
#[test]
fn a_part_carries_its_item_its_slot_and_its_armour() {
    let dump = dump();
    let seat = dump
        .parts
        .iter()
        .find(|part| part.id == "seat")
        .expect("the seat");
    assert_eq!(seat.durability, 300, "hit points");
    assert_eq!(seat.damage_modifier, 60, "damage modifier, a percentage");
    assert_eq!(seat.item, "seat", "the item it is made of");
    assert_eq!(seat.location, "center", "the slot it occupies");
    for flag in ["SEAT", "BOARDABLE", "CARGO", "BELTABLE"] {
        assert!(seat.flags.contains(flag), "the seat carries {flag}");
    }
    assert_eq!(seat.damage_reduction.get("bash"), Some(&5.0), "bash armour");
    // A seat is a `CARGO` part and its size is that cargo: 25 litres, not zero. The dump is the authority here, and the flag is what says the size is cargo rather
    // than a bed's or a workbench's.
    assert_eq!(
        seat.size, 25_000,
        "25 litres of cargo room, which its `CARGO` flag promises"
    );
}

/// Every invented turret part is present and named the way `finalize` names it, which is what tells a port that read only the data's records where the other 239 came from.
#[test]
fn the_invented_turret_parts_are_in_the_store() {
    let dump = dump();
    let turrets: Vec<&vehicles::PartSpec> = dump
        .parts
        .iter()
        .filter(|part| part.id.starts_with("turret_"))
        .collect();
    assert_eq!(
        turrets.len(),
        241,
        "**the turrets the data never writes**: `finalize` makes one per mountable gun"
    );
    let named: Vec<&String> = turrets.iter().map(|part| &part.item).collect();
    assert!(
        named.iter().any(|item| *item == "m1918"),
        "a turret is made of the gun it mounts"
    );
}

/// **A blueprint names parts the store holds.** A prototype is a list of placements - what part, where - and a placement that names a part the store does not have is a
/// vehicle that cannot be built, which is why this is the gate that makes the two halves one system rather than two lists that happen to share a file.
#[test]
fn every_placement_in_every_blueprint_names_a_part_the_store_holds() {
    let dump = dump();
    let known: std::collections::BTreeSet<&String> =
        dump.parts.iter().map(|part| &part.id).collect();
    let mut placements = 0usize;
    let mut unknown: Vec<(&String, &String)> = Vec::new();
    for prototype in &dump.prototypes {
        for placement in &prototype.parts {
            placements += 1;
            if !known.contains(&placement.part) {
                unknown.push((&prototype.id, &placement.part));
            }
        }
    }
    println!(
        "{} placements across {} prototypes",
        placements,
        dump.prototypes.len()
    );
    assert_eq!(placements, 19_938, "every placement in the data");
    assert!(
        unknown.is_empty(),
        "placements naming parts the store lacks: {unknown:?}"
    );
}

/// A known prototype, part for part: the 2-seater helicopter's blueprint opens with a cross-variant frame in the same square as its seat, controls and dashboard.
///
/// **One part per square is not the rule** - a frame carries a seat, a seatbelt and controls on the same square, which is what a vehicle is.
#[test]
fn a_blueprint_says_what_part_goes_where() {
    let dump = dump();
    let helicopter = dump
        .prototypes
        .iter()
        .find(|proto| proto.id == "2seater2")
        .expect("the helicopter");
    assert_eq!(
        helicopter.name, "2-seater helicopter A",
        "the name the data gives it"
    );
    assert_eq!(helicopter.parts.len(), 71, "parts in the blueprint");
    let first = helicopter.parts.first().expect("a first part");
    assert_eq!(first.part, "frame", "a frame comes first");
    assert_eq!((first.x, first.y), (0, 0), "in the origin square");
    assert_eq!(first.variant, "cross", "the variant the data names");
    let same_square = helicopter
        .parts
        .iter()
        .filter(|placement| (placement.x, placement.y) == (0, 0))
        .count();
    assert!(
        same_square > 1,
        "**several parts share a square**: {same_square} of them do"
    );
}

/// One prototype in the data has an **empty** blueprint. It is recorded rather than skipped, because a builder that quietly dropped it would make 249 prototypes into 248
/// and nothing would say so.
#[test]
fn the_one_empty_blueprint_is_kept_and_named() {
    let dump = dump();
    let empty: Vec<&String> = dump
        .prototypes
        .iter()
        .filter(|proto| proto.parts.is_empty())
        .map(|proto| &proto.id)
        .collect();
    assert_eq!(empty.len(), 1, "exactly one: {empty:?}");
    assert!(
        dump.parts.len() == 701,
        "and the part store is untouched by it"
    );
}

/// **A part says what its material weighs.** The crash rule divides the colliding part's density by the obstacle's, and the recording only named the part that took
/// the hit; the dump now carries the figure for every part. A steel frame is 8 per litre, a glass windshield 2.5 - the two densities the crash rows already used, now
/// in the store rather than in a formula.
#[test]
fn a_part_carries_its_material_density() {
    let dump = dump();
    let density_of = |id: &str| {
        dump.parts
            .iter()
            .find(|part| part.id == id)
            .and_then(|part| part.density)
            .unwrap_or_else(|| panic!("{id} has a density"))
    };
    assert_eq!(density_of("frame"), 8.0, "a steel frame");
    assert_eq!(density_of("windshield"), 2.5, "glass");
    assert!(
        dump.parts
            .iter()
            .filter(|part| part.density.is_none())
            .count()
            < dump.parts.len(),
        "and not every part is empty"
    );
}
