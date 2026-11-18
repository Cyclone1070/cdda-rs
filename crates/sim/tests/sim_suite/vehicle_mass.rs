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
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_mass.json");
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
    assert_eq!(
        stowed, rocks,
        "the port stowed every rock the reference did"
    );

    let cargo = i64::from(int_of(&root, "cargo_mass_g"));
    assert_eq!(
        cargo,
        i64::from(rocks) * 657,
        "the reference's fifty rocks weigh 50 x 657 g: the recording and the rock's data agree"
    );

    for (index, entry) in parts.iter().enumerate() {
        let expected =
            i64::from(int_of(entry, "base_mass_g")) + i64::from(int_of(entry, "items_mass_g"));
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
    assert!(
        recorded_total > 0,
        "the reference weighed something: {recorded_total}"
    );
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

/// **A spawned car's loot is part of its weight.** `place_spawn_items` (`vehicle.cpp:6379-6450`) rolls items into the cargo parts when a vehicle is placed; the rolls are dice and
/// cannot be replayed, but the list they produced can - and the list is what the port owes. `corpus/vehicle_rolling.json` records the car the reference placed: seventeen stacks in two
/// cargo parts with the weight of each, and the vehicle's whole weight including them.
///
/// **The two numberings differ**: the reference's `part_count()` counts the fake parts its indexing needs, while the port keeps one part per definition. The recording is therefore
/// matched by part **name**, not by index.
/// The port's own name for a part, so a recording's reference indices can be translated.
fn port_name(world: &World, vehicle: Entity, index: usize) -> String {
    world
        .get::<cdda_sim::vehicles::Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
        .map(|part| part.part.clone())
        .unwrap_or_default()
}

#[test]
fn a_spawned_cars_cargo_is_part_of_its_weight() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicle_rolling.json"),
    )
    .expect("the vehicle recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(origin)) = root.get("origin") else {
        panic!("an origin");
    };
    let here = (
        coordinate(origin, 0),
        coordinate(origin, 1),
        coordinate(origin, 2),
    );
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };
    let Some(cdda_json::Value::Object(car)) = cases.iter().find(|case| {
        matches!(case, cdda_json::Value::Object(row) if matches!(row.get("name"), Some(cdda_json::Value::Str(name)) if name == "car"))
    }) else {
        panic!("the car case");
    };

    // The recording's two lists: each reference index's part name and base weight, and each stack of loot.
    let mut reference_parts: Vec<(i64, String, i64)> = Vec::new();
    let Some(cdda_json::Value::Array(recorded_parts)) = car.get("parts") else {
        panic!("parts");
    };
    for entry in recorded_parts {
        let cdda_json::Value::Object(row) = entry else {
            panic!("a part");
        };
        reference_parts.push((
            i64::from(int_of(row, "index")),
            text_of(row, "part"),
            i64::from(int_of(row, "base_mass_g")),
        ));
    }
    let Some(cdda_json::Value::Array(cargo)) = car.get("cargo") else {
        panic!("cargo");
    };

    let mut world = World::new();
    cdda_sim::vehicles::set_store(&mut world, store());
    let vehicle = spawn_facing(&mut world, "car", here, 0.0).expect("the car builds");
    let count = world
        .get::<cdda_sim::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());

    // **The registry, weighted from the recording**: every part's base item at the reference's own weight, and every piece of loot at its stack's weight per unit.
    let mut kinds: Vec<(String, i64)> = Vec::new();
    let mut remember = |id: &str, weight: i64| {
        if let Some(slot) = kinds.iter_mut().find(|(name, _)| name == id) {
            slot.1 = weight;
        } else {
            kinds.push((id.to_owned(), weight));
        }
    };
    for index in 0..count {
        let Some(spec) = part_spec(&world, vehicle, index) else {
            continue;
        };
        let name = port_name(&world, vehicle, index);
        let weight = reference_parts
            .iter()
            .find(|(_, part, _)| *part == name)
            .map_or(0, |(_, _, weight)| *weight);
        remember(&spec.item, weight);
    }
    for entry in cargo {
        let cdda_json::Value::Object(row) = entry else {
            panic!("a cargo entry");
        };
        let per_unit = i64::from(int_of(row, "mass_g")) / i64::from(int_of(row, "count")).max(1);
        remember(&text_of(row, "id"), per_unit);
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

    // **The recording's own list, put back where it was found**, by part name.
    let mut stowed = 0;
    for entry in cargo {
        let cdda_json::Value::Object(row) = entry else {
            panic!("a cargo entry");
        };
        let reference_index = i64::from(int_of(row, "part"));
        let Some((_, name, _)) = reference_parts
            .iter()
            .find(|(index, _, _)| *index == reference_index)
        else {
            panic!("reference part {reference_index}");
        };
        let Some(part) = (0..count).find(|index| port_name(&world, vehicle, *index) == *name)
        else {
            panic!("the port has no part called {name}");
        };
        let kind = id_in(&world, &text_of(row, "id")).expect("the loot's kind");
        let charges = int_of(row, "count").max(1);
        if stow_item(&mut world, vehicle, part, kind, charges).is_some() {
            stowed += 1;
        }
    }
    assert_eq!(
        stowed,
        cargo.len(),
        "**the port stowed every stack the reference's spawn produced**"
    );

    assert_eq!(
        mass(&world, vehicle),
        i64::from(int_of(car, "total_mass_g")),
        "**the car weighs what the reference says, loot and all**"
    );
}
