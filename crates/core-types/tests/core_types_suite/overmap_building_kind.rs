//! Which of the city's lists a spot draws from.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{BuildingKind, building_kind};
use cdda_core_types::rng::Rolls;

// The reference's own defaults for the dda region: shops at 30 with a spread of 50, parks at 20 with a spread of 80.
const SHOP_RADIUS: i32 = 30;
const SHOP_SIGMA: i32 = 50;
const PARK_RADIUS: i32 = 20;
const PARK_SIGMA: i32 = 80;

#[test]
fn the_middle_of_town_draws_shops() {
    let mut rolls = Rolls::new(1);
    // At the very centre every threshold passes, so the first test in the reference's order wins.
    assert_eq!(
        building_kind(
            0,
            SHOP_RADIUS,
            SHOP_SIGMA,
            PARK_RADIUS,
            PARK_SIGMA,
            &mut rolls
        ),
        BuildingKind::Shop
    );
}

#[test]
fn a_spot_exactly_on_the_threshold_falls_to_the_next_list() {
    let mut rolls = Rolls::new(1);
    // With no spread the thresholds are the radii themselves, and the comparison is strict. At distance 30 the shop
    // threshold is not passed, and the park threshold of 20 is already behind us, so the spot is a house - which is
    // the case my first expectation got backwards.
    // The shop threshold is checked first and is strict: 29 is a shop, 30 is not.
    assert_eq!(
        building_kind(29, 30, 0, 20, 0, &mut rolls),
        BuildingKind::Shop
    );
    assert_eq!(
        building_kind(30, 30, 0, 20, 0, &mut rolls),
        BuildingKind::House
    );
    // The park check is only reached when the shop check fails, so with shops further out than parks a park is
    // never drawn - the spot is already past the park's radius by then. My first two attempts at this test put the
    // radii the other way round and expected parks that the rule cannot produce.
    for distance in 0..60 {
        assert_ne!(
            building_kind(distance, 30, 0, 20, 0, &mut rolls),
            BuildingKind::Park,
            "shops further out than parks leave no room for parks, at {distance}"
        );
    }
    // Turn the radii round and parks appear between them.
    assert_eq!(
        building_kind(25, 20, 0, 30, 0, &mut rolls),
        BuildingKind::Park
    );
    assert_eq!(
        building_kind(35, 20, 0, 30, 0, &mut rolls),
        BuildingKind::House
    );
}

#[test]
fn the_edge_of_town_draws_houses() {
    let mut rolls = Rolls::new(1);
    assert_eq!(
        building_kind(90, SHOP_RADIUS, 0, PARK_RADIUS, 0, &mut rolls),
        BuildingKind::House
    );
}

#[test]
fn a_spread_raises_a_threshold_but_never_lowers_it_below_the_radius() {
    // A normal roll far below the radius is clamped, so even an unlucky town keeps shops at its middle.
    let mut rolls = Rolls::new(1);
    let mut kinds = Vec::new();
    for _ in 0..200 {
        kinds.push(building_kind(
            0,
            SHOP_RADIUS,
            SHOP_SIGMA,
            PARK_RADIUS,
            PARK_SIGMA,
            &mut rolls,
        ));
    }
    assert!(
        kinds.iter().all(|kind| *kind == BuildingKind::Shop),
        "the centre is always shops"
    );
    // Far out it is almost always houses - but not always: the park threshold is a normal roll with a spread of 80,
    // so a spot 200 out still draws a park rarely, which is the reference's own behaviour and the second thing my
    // first expectation got wrong.
    let mut far = Vec::new();
    for _ in 0..2000 {
        far.push(building_kind(
            200,
            SHOP_RADIUS,
            SHOP_SIGMA,
            PARK_RADIUS,
            PARK_SIGMA,
            &mut rolls,
        ));
    }
    let houses = far
        .iter()
        .filter(|kind| **kind == BuildingKind::House)
        .count();
    assert!(
        houses > far.len() * 90 / 100,
        "almost all houses: {houses} of {}",
        far.len()
    );
    assert!(
        far.iter().any(|kind| *kind != BuildingKind::House),
        "and a park or two at the edge, because the spread is wide"
    );
}

#[test]
fn the_same_seed_asks_the_same_lists() {
    let mut first = Rolls::new(31337);
    let mut second = Rolls::new(31337);
    for distance in 0..40 {
        assert_eq!(
            building_kind(
                distance,
                SHOP_RADIUS,
                SHOP_SIGMA,
                PARK_RADIUS,
                PARK_SIGMA,
                &mut first
            ),
            building_kind(
                distance,
                SHOP_RADIUS,
                SHOP_SIGMA,
                PARK_RADIUS,
                PARK_SIGMA,
                &mut second
            ),
            "at distance {distance}"
        );
    }
}

#[test]
fn there_are_parks_somewhere_between_the_shops_and_the_houses() {
    let mut rolls = Rolls::new(99);
    let mut parks = 0;
    for _ in 0..500 {
        if building_kind(
            35,
            SHOP_RADIUS,
            SHOP_SIGMA,
            PARK_RADIUS,
            PARK_SIGMA,
            &mut rolls,
        ) == BuildingKind::Park
        {
            parks += 1;
        }
    }
    assert!(parks > 0, "a spot between the two radii can draw a park");
}
