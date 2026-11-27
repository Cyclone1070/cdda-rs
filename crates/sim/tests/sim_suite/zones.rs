//! Zones: the labelled rectangles a faction owns, and the three questions the manager answers about them.
//!
//! Reference: `zone_data` (`clzones.h:440-543`), `zone_manager::get_near` (`clzones.cpp:1172-1200`) and `square_dist` (`line.cpp:1234-1240`).

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

/// **A zone covers its rectangle inclusively, and a zone written backwards covers the same ground.** `place` keeps one corner where the entity stands and the far one beside it,
/// and the two may be written either way round - the reference normalises its corners when it loads them, and the port orders them when it asks.
#[test]
fn a_zone_covers_its_rectangle_inclusive_and_either_way_round() {
    assert!(
        cdda_sim::zones::covers((2, 3), (4, 5), 0, (2, 3, 0)),
        "**a corner is inside**"
    );
    assert!(
        cdda_sim::zones::covers((2, 3), (4, 5), 0, (4, 5, 0)),
        "**and so is the far one, because both are inclusive**"
    );
    assert!(
        !cdda_sim::zones::covers((2, 3), (4, 5), 0, (5, 5, 0)),
        "**one past the far edge is out**"
    );
    assert!(
        !cdda_sim::zones::covers((2, 3), (4, 5), 0, (3, 4, 1)),
        "**and a level up is out, because a ground zone has one level**"
    );
    assert!(
        cdda_sim::zones::covers((4, 5), (2, 3), 0, (3, 4, 0)),
        "**the corners are a pair, not a direction**"
    );
}

/// **Near is the larger axis, not the diagonal.** `square_dist` is `max( |dx|, |dy| )`, so three across and four up is **four** away - which is why a job with a range of three
/// cannot reach it, though a diagonal distance would also be four.
#[test]
fn near_is_measured_by_the_larger_axis() {
    assert_eq!(
        cdda_sim::zones::square_dist((0, 0, 0), (3, 1, 0)),
        3,
        "**three across and one up is three**"
    );
    assert_eq!(
        cdda_sim::zones::square_dist((0, 0, 0), (-2, -4, 0)),
        4,
        "**four the other way is four**"
    );
    assert_eq!(
        cdda_sim::zones::square_dist((5, 5, 0), (5, 5, 7)),
        0,
        "**and a level up costs nothing, because the vertical axis is not in it**"
    );
}

/// **The manager's three questions, from the store.** Two corpse zones and a wood zone: one corpse zone is within range, one is not, and the wood zone is the wrong kind even
/// though it lies on the same ground - so a butchering job sees exactly the near corpse zone's squares. Switching that zone off takes it out of the answer as well.
#[test]
fn a_job_finds_the_zones_of_its_own_kind_that_are_near() {
    let mut world = World::new();
    let near = cdda_sim::zones::place(
        &mut world,
        (1, 1, 0),
        (2, 2),
        "LOOT_CORPSE",
        "your_followers",
        "butcher here",
        "",
    );
    cdda_sim::zones::place(
        &mut world,
        (30, 30, 0),
        (31, 31),
        "LOOT_CORPSE",
        "your_followers",
        "butcher far away",
        "",
    );
    cdda_sim::zones::place(
        &mut world,
        (1, 1, 0),
        (2, 2),
        "LOOT_WOOD",
        "your_followers",
        "wood here",
        "",
    );
    let squares =
        cdda_sim::zones::zones_near(&mut world, "LOOT_CORPSE", "your_followers", (0, 0, 0), 5);
    assert_eq!(
        squares.len(),
        4,
        "**the near corpse zone is four squares and the far one is out of range: {squares:?}**"
    );
    assert!(squares.contains(&(2, 2, 0)), "**including its far corner**");
    assert!(
        !cdda_sim::zones::zones_near(&mut world, "LOOT_WOOD", "your_followers", (0, 0, 0), 5)
            .is_empty(),
        "**the wood zone is the wood job's business, and it finds its own**"
    );
    // **A zone switched off is invisible**, which is the reference's `enabled` gate on its caches.
    world
        .entity_mut(near)
        .insert(cdda_sim::zones::ZoneEnabled(false));
    assert!(
        cdda_sim::zones::zones_near(&mut world, "LOOT_CORPSE", "your_followers", (0, 0, 0), 5)
            .is_empty(),
        "**a disabled zone answers nothing**"
    );
    assert!(
        cdda_sim::zones::zone_at(&mut world, "LOOT_CORPSE", "your_followers", (1, 1, 0)).is_none(),
        "**and contains nothing either**"
    );
    // **And another faction's zone is not the player's business**, though it lies on the same squares.
    cdda_sim::zones::place(
        &mut world,
        (1, 1, 0),
        (2, 2),
        "LOOT_CORPSE",
        "hells_raiders",
        "somebody else's",
        "",
    );
    assert!(
        cdda_sim::zones::zone_at(&mut world, "LOOT_CORPSE", "hells_raiders", (1, 1, 0)).is_some(),
        "**their own zone is theirs**"
    );
    world
        .entity_mut(near)
        .insert(cdda_sim::zones::ZoneEnabled(true));
    assert_eq!(
        cdda_sim::zones::zone_at(&mut world, "LOOT_CORPSE", "your_followers", (1, 1, 0))
            .map(|zone| zone.name),
        Some("butcher here".to_owned()),
        "**and the player's zone is the player's, not the raiders'**"
    );
}
