//! **Breaking a whole vehicle at once**, and the two branches that decide how.
//!
//! Reference: `vehicle::smash` (`vehicle.cpp:908-948`). The rule asks two questions per part, in order: a square holding **more than one structure** is emptied whatever the dice
//! say, and every other part is rolled for against `percent` and then loses a random fraction of its own durability.

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

use cdda_sim::vehicle_smash::{Smash, smash};
use cdda_sim::vehicles::{Vehicle, spawn_facing};

/// A vehicle built from the given `(part id, square, durability, location)` list, standing at the origin.
fn a_hull(parts: &[(&str, (i32, i32), i32, &str)]) -> (World, bevy_ecs::entity::Entity) {
    let mut world = World::new();
    let specs = parts
        .iter()
        .map(
            |(id, _, durability, location)| cdda_content::vehicles::PartSpec {
                id: (*id).to_owned(),
                name: (*id).to_owned(),
                durability: *durability,
                location: (*location).to_owned(),
                ..cdda_content::vehicles::PartSpec::default()
            },
        )
        .collect::<Vec<_>>();
    let placements = parts
        .iter()
        .map(|(id, (x, y), _, _)| cdda_content::vehicles::Placement {
            part: (*id).to_owned(),
            x: *x,
            y: *y,
            ..cdda_content::vehicles::Placement::default()
        })
        .collect::<Vec<_>>();
    cdda_sim::vehicles::set_store(
        &mut world,
        cdda_content::vehicles::Vehicles {
            parts: specs,
            prototypes: vec![cdda_content::vehicles::Prototype {
                id: "hull".to_owned(),
                name: "hull".to_owned(),
                parts: placements,
                spawns: Vec::new(),
            }],
        },
    );
    let vehicle = spawn_facing(&mut world, "hull", (10, 10, 0), 0.0).expect("the hull builds");
    (world, vehicle)
}

fn health_of(world: &World, vehicle: bevy_ecs::entity::Entity) -> Vec<(String, i32, bool)> {
    world
        .get::<Vehicle>(vehicle)
        .map(|record| {
            record
                .parts
                .iter()
                .map(|part| (part.part.clone(), part.hp, part.broken))
                .collect()
        })
        .expect("the hull is still there")
}

/// **A square with two structures is emptied whatever the dice say.** Reference: `vehicle::smash` (`vehicle.cpp:917-934`) counts the structures on the part's square and, when
/// there is more than one, destroys **every part on it** before any roll - because two structures cannot share a square, so the square is already wreckage.
///
/// The roll is pinned to **one**, which fails every part check: so anything this test sees destroyed was destroyed by the structure branch and nothing else.
#[test]
fn a_square_with_two_structures_is_emptied_whatever_the_dice_say() {
    let (mut world, vehicle) = a_hull(&[
        ("frame_a", (0, 0), 400, "STRUCTURE"),
        ("frame_b", (0, 0), 400, "STRUCTURE"),
        ("seat", (0, 0), 100, "CENTER"),
        ("board", (1, 0), 200, "STRUCTURE"),
    ]);
    let mut rolls = cdda_core_types::rng::Rolls::fixed(1.0);
    smash(&mut world, vehicle, &Smash::half(), &mut rolls);
    let health = health_of(&world, vehicle);
    for name in ["frame_a", "frame_b", "seat"] {
        assert!(
            health
                .iter()
                .find(|(part, _, _)| part == name)
                .is_some_and(|(_, hp, broken)| *hp == 0 && *broken),
            "**{name} stands on the over-built square and is destroyed: {health:?}**"
        );
    }
    assert!(
        health
            .iter()
            .find(|(part, _, _)| part == "board")
            .is_some_and(|(_, hp, broken)| *hp == 200 && !*broken),
        "**while the square with one structure is untouched at this roll: {health:?}**"
    );
}

/// **A single structure loses a share of its own durability.** Reference: `vehicle::smash` (`vehicle.cpp:936-947`): the part is rolled for against one in a thousand per
/// thousandth of `percent`, and when it loses it gives up `durability * rng_float( min, max )`.
///
/// At roll zero every part is reached and takes the **least** loss, so `Smash::half` takes exactly half of each part's durability - which is arithmetic the test can state
/// without repeating the formula.
#[test]
fn a_single_structure_loses_the_least_share_of_its_durability() {
    let (mut world, vehicle) = a_hull(&[
        ("frame", (0, 0), 400, "STRUCTURE"),
        ("board", (1, 0), 200, "STRUCTURE"),
    ]);
    let mut rolls = cdda_core_types::rng::Rolls::fixed(0.0);
    smash(&mut world, vehicle, &Smash::half(), &mut rolls);
    let health = health_of(&world, vehicle);
    assert_eq!(
        health
            .iter()
            .find(|(part, _, _)| part == "frame")
            .map(|(_, hp, broken)| (*hp, *broken)),
        Some((200, false)),
        "**four hundred of durability loses half of it**"
    );
    assert_eq!(
        health
            .iter()
            .find(|(part, _, _)| part == "board")
            .map(|(_, hp, broken)| (*hp, *broken)),
        Some((100, false)),
        "**and two hundred loses half of that**"
    );
    // **A part already broken is skipped.** Reference: `if( part.is_broken() || part.removed ) { continue; }`.
    let (mut world, vehicle) = a_hull(&[("frame", (0, 0), 400, "STRUCTURE")]);
    if let Some(mut record) = world.get_mut::<Vehicle>(vehicle)
        && let Some(part) = record.parts.first_mut()
    {
        part.hp = 0;
        part.broken = true;
    }
    let mut rolls = cdda_core_types::rng::Rolls::fixed(0.0);
    smash(&mut world, vehicle, &Smash::half(), &mut rolls);
    assert_eq!(
        health_of(&world, vehicle),
        vec![("frame".to_owned(), 0, true)],
        "**a broken part is neither rolled for nor changed**"
    );
}
