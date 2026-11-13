//! Vehicle parts and prototypes, against the reference's own list.
//!
//! `corpus/vehicles.json` version 2 is `--dump-vehicles`: every part the reference's store holds - the **486 definitions the data writes** resolved through
//! `copy-from`, plus the **239 turret parts** `finalize` invents from mountable guns, which makes 701 - each carrying what a part *is*: the item it is made of, the slot it
//! occupies, its durability, its damage modifier, its string flags and its flat damage reduction.

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
    assert_eq!(sorted.len(), ids.len(), "no duplicates, and the dump is sorted");
}

/// **A part carries what it is, not only its name.** `seat` is the reference's own example: 300 hit points, a 60% damage modifier, made of the item `seat`, in the
/// `center` slot, belt-able and boardable, and it shaves 5 off a bash before its hit points are touched.
#[test]
fn a_part_carries_its_item_its_slot_and_its_armour() {
    let dump = dump();
    let seat = dump.parts.iter().find(|part| part.id == "seat").expect("the seat");
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
    assert_eq!(seat.size, 25_000, "25 litres of cargo room, which its `CARGO` flag promises");
}

/// Every invented turret part is present and named the way `finalize` names it, which is what tells a port that read only the data's records where the other 239 came from.
#[test]
fn the_invented_turret_parts_are_in_the_store() {
    let dump = dump();
    let turrets: Vec<&vehicles::PartSpec> = dump.parts.iter().filter(|part| part.id.starts_with("turret_")).collect();
    assert_eq!(turrets.len(), 241, "**the turrets the data never writes**: `finalize` makes one per mountable gun");
    let named: Vec<&String> = turrets.iter().map(|part| &part.item).collect();
    assert!(named.iter().any(|item| *item == "m1918"), "a turret is made of the gun it mounts");
}
