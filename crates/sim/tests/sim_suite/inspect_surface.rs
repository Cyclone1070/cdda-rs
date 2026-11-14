//! Looking at the world: the surface the headless binary and the QA runner read.
//!
//! Source: none - this is the port's own debugging surface, so the tests here are about the
//! properties a person needs when a gate fails: it must find things that are there, say what they
//! are, and notice when one of them changes.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::Position;
use cdda_sim::inspect;
use cdda_sim::square::materialise;

const AIR: u32 = 1;
const CHAIR: u32 = 9;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, AIR);
    level.set_furniture(2, 2, CHAIR, 0);
    level
}

#[test]
fn a_square_report_names_what_stands_there() {
    let mut world = World::new();
    materialise(&mut world, &level());

    let report = inspect::square_report(&world, (2, 2, 0));

    assert!(
        report.contains("holds 3 thing(s)"),
        "the square, its ground and the chair: {report}"
    );
    assert!(
        report.contains("Terrain"),
        "the ground's code is shown: {report}"
    );
    assert!(report.contains("Furniture"), "and the chair's: {report}");
}

#[test]
fn the_store_report_counts_every_kind_of_thing() {
    let mut world = World::new();
    materialise(&mut world, &level());

    let counts = inspect::component_counts(&world);

    assert_eq!(
        counts.get("Square"),
        Some(&144),
        "one square per square of the level"
    );
    assert_eq!(
        counts.get("Ground"),
        Some(&144),
        "and one piece of ground under each"
    );
    assert_eq!(
        counts.get("Furniture"),
        Some(&1),
        "only the square that was given a chair has one"
    );
    assert!(
        !counts.contains_key("Field"),
        "and nothing invented a field"
    );
}

#[test]
fn a_fingerprint_changes_when_the_square_does() {
    let mut world = World::new();
    materialise(&mut world, &level());
    let before = inspect::world_fingerprints(&world);

    let item = world
        .spawn((
            Position { x: 2, y: 2, z: 0 },
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(1),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
        ))
        .id();
    cdda_sim::singletons::index_mut(&mut world).insert(item, (2, 2, 0));
    let after = inspect::world_fingerprints(&world);

    assert_eq!(
        inspect::squares_that_differ(&before, &after),
        vec![(2, 2, 0)],
        "one square changed, and the diff names it"
    );
    assert!(
        before.contains_key(&(2, 2, 0)),
        "the square was in the first map, so this is a change and not an arrival only"
    );
}
