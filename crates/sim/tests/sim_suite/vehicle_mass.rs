//! **A vehicle weighs what its parts and its cargo weigh.**
//!
//! `corpus/vehicle_mass.json` is `--dump-vehicle-mass`: the reference places the bicycle on a floored square, puts **fifty rocks** in its basket through
//! `vehicle::add_item`, and records every part's base item weight and carried items' weight, then `unloaded_mass`, the cargo mass and `total_mass` - the number
//! `vehicle::calc_mass_center` caches.
//!
//! The port cannot take the weights from the corpus without taking the rule's inputs, so the gate installs a registry whose weights are **the reference's own**
//! (`base_mass_g` per part, and the rock's 657 g), stows the same fifty rocks in the port's basket, and asks `mass` for the total.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::item_registry::{Kind, id_in, set_item_types};
use cdda_sim::vehicle_cargo::stow_item;
use cdda_sim::vehicle_mass::{mass, part_mass};
use cdda_sim::vehicles::{part_spec, spawn_facing};

/// One JSON object's string field.
fn text_of(entry: &cdda_json::Object, key: &str) -> String {
    match entry.get(key) {
        Some(cdda_json::Value::Str(value)) => value.clone(),
        _ => panic!("{key} is a string"),
    }
}

/// One JSON object's integer field.
fn int_of(entry: &cdda_json::Object, key: &str) -> i32 {
    match entry.get(key) {
        Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a small number"),
        _ => panic!("{key} is an integer"),
    }
}

/// One coordinate of an origin array.
fn coordinate(origin: &[cdda_json::Value], at: usize) -> i32 {
    match origin.get(at) {
        Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a coordinate"),
        _ => panic!("a coordinate"),
    }
}

/// The recording, parsed.
fn recording() -> cdda_json::Object {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_mass.json");
    let bytes = std::fs::read(path).expect("the mass recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    root
}

/// The recording's parts, in order.
fn recorded_parts(root: &cdda_json::Object) -> Vec<cdda_json::Object> {
    let Some(cdda_json::Value::Array(parts)) = root.get("parts") else {
        panic!("the recording has parts");
    };
    parts
        .iter()
        .map(|part| {
            let cdda_json::Value::Object(entry) = part else {
                panic!("a part is an object");
            };
            entry.clone()
        })
        .collect()
}

/// The vehicle store the bicycle is built from.
fn store() -> cdda_content::vehicles::Vehicles {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicles.json");
    let bytes = std::fs::read(path).expect("the parts recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    cdda_content::vehicles::from_dump(&value).expect("the store loads")
}

/// A world with the bicycle, a registry weighted from the recording, and the reference's fifty rocks in the basket.
fn bicycle_with_cargo() -> (World, Entity, usize, Vec<cdda_json::Object>, i32) {
    let root = recording();
    let parts = recorded_parts(&root);
    let Some(cdda_json::Value::Array(origin)) = root.get("origin") else {
        panic!("an origin");
    };
    let here = (
        coordinate(origin, 0),
        coordinate(origin, 1),
        coordinate(origin, 2),
    );

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = spawn_facing(&mut world, "bicycle", here, 0.0).expect("the bicycle builds");

    // The registry, weighted from the recording: every part's base item at the reference's own weight, and the rock at its 657 g.
    let mut kinds: Vec<(String, i64)> = Vec::new();
    for (index, entry) in parts.iter().enumerate() {
        let spec = part_spec(&world, vehicle, index).expect("the part's spec");
        let weight = i64::from(int_of(entry, "base_mass_g"));
        if let Some(slot) = kinds.iter_mut().find(|(id, _)| *id == spec.item) {
            slot.1 = weight;
        } else {
            kinds.push((spec.item.clone(), weight));
        }
    }
    let rock_weight = 657;
    kinds.push(("rock".to_owned(), rock_weight));
    let specs: Vec<Kind<'_>> = kinds
        .iter()
        .map(|(id, weight)| Kind {
            id,
            volume_ml: if id == "rock" { 250 } else { 0 },
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

    let rock = id_in(&world, "rock").expect("the rock");
    let rocks = int_of(&root, "rocks");
    let basket = usize::try_from(int_of(&root, "basket")).expect("a part index");
    let mut stowed = 0;
    for _ in 0..rocks {
        if stow_item(&mut world, vehicle, basket, rock, 1).is_some() {
            stowed += 1;
        }
    }
    (world, vehicle, basket, parts, stowed)
}

#[test]
fn a_vehicle_weighs_its_parts_and_its_cargo() {
    let (mut world, vehicle, basket, parts, stowed) = bicycle_with_cargo();
    let root = recording();
    let rocks = int_of(&root, "rocks");
    assert_eq!(stowed, rocks, "the port stowed every rock the reference did");

    let cargo = i64::from(int_of(&root, "cargo_mass_g"));
    assert_eq!(
        cargo,
        i64::from(rocks) * 657,
        "the reference's fifty rocks weigh 50 x 657 g: the recording and the rock's data agree"
    );

    for (index, entry) in parts.iter().enumerate() {
        let expected = i64::from(int_of(entry, "base_mass_g")) + i64::from(int_of(entry, "items_mass_g"));
        assert_eq!(
            part_mass(&world, vehicle, index),
            expected,
            "**{} weighs its base item plus what it carries**",
            text_of(entry, "part")
        );
    }
    assert!(
        part_mass(&world, vehicle, basket) > i64::from(int_of(&parts[basket], "base_mass_g")),
        "the basket is heavier for its rocks"
    );

    let recorded_total = i64::from(int_of(&root, "total_mass_g"));
    assert!(recorded_total > 0, "the reference weighed something: {recorded_total}");
    assert_eq!(
        mass(&world, vehicle),
        recorded_total,
        "**the whole vehicle weighs what the reference says it does**"
    );

    // A stack weighs its charge count, not one item: three more rocks add three rocks' weight.
    let rock = id_in(&world, "rock").expect("the rock");
    let before = part_mass(&world, vehicle, basket);
    stow_item(&mut world, vehicle, basket, rock, 3).expect("three more rocks fit");
    assert_eq!(
        part_mass(&world, vehicle, basket),
        before + 3 * 657,
        "**a stack weighs its charges**"
    );
}

#[test]
fn a_parts_cargo_can_be_weighted_down() {
    // The data gives every part `cargo_weight_modifier` 100, so the scaling branch needs a number the dump cannot show. This is the reference's own formula with a
    // part that halves its cargo's weight (`vehicle.cpp:8504-8506`).
    let (mut world, vehicle, basket, _parts, _stowed) = bicycle_with_cargo();
    let before = part_mass(&world, vehicle, basket);
    let base = before - i64::from(50) * 657;

    let mut materials = store();
    let spec = materials
        .parts
        .iter_mut()
        .find(|part| part.id == "basketsm")
        .expect("the basket's spec");
    spec.cargo_weight_modifier = 50;
    cdda_sim::vehicles::set_store(&mut world, materials);
    assert_eq!(
        part_mass(&world, vehicle, basket),
        base + i64::from(50) * 657 / 2,
        "**a part that halves its cargo's weight adds half**"
    );
}
