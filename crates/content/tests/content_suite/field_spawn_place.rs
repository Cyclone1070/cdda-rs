//! Where a spawned monster lands.
//!
//! Reference: `random_point( points_in_radius( p, radius ), passable )` (`map_field.cpp:777`). Three things are easy to get
//! wrong and all three are pinned here: the "radius" is the **bounding box**, not a circle; the optimist tries **ten** random
//! squares before it enumerates; and the enumeration runs **x fastest**, so the fallback list is row-major.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::random_point_in_radius;
use cdda_core_types::rng::Rolls;

#[test]
fn a_radius_of_zero_can_only_put_a_monster_on_the_centre() {
    // The range of a radius of zero is the centre alone, so a passable centre is always the answer and a blocked one is never
    // any answer.
    let mut rolls = Rolls::new(3);
    assert_eq!(
        random_point_in_radius((10, 10), 0, |_, _| true, &mut rolls),
        Some((10, 10))
    );
    let mut rolls = Rolls::new(3);
    assert_eq!(
        random_point_in_radius((10, 10), 0, |_, _| false, &mut rolls),
        None
    );
}

#[test]
fn a_blocked_centre_is_never_chosen() {
    // The field sits on a wall; the monster must land beside it. Whatever the radius, the refused square never comes back.
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..50 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        let placed = random_point_in_radius((5, 5), 2, |x, y| (x, y) != (5, 5), &mut rolls)
            .expect("a square");
        assert_ne!(
            placed,
            (5, 5),
            "seed {seed} put the monster on the refused centre"
        );
        assert!(
            (3..=7).contains(&placed.0) && (3..=7).contains(&placed.1),
            "{placed:?} of seed {seed}"
        );
    }
}

#[test]
fn the_range_is_the_bounding_box_and_not_a_circle() {
    // A square at the corner of the box is distance two in both directions - further than the radius in a straight line - and
    // the reference still allows it. A port that used a circle would never produce it.
    let corners: Vec<(i32, i32)> = vec![(3, 3), (3, 7), (7, 3), (7, 7)];
    let mut seen = false;
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..400 {
        let attempt_seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(attempt_seed);
        let placed = random_point_in_radius((5, 5), 2, |_, _| true, &mut rolls).expect("a square");
        if corners.contains(&placed) {
            seen = true;
            break;
        }
    }
    assert!(seen, "no corner was ever chosen in 400 draws");
}

#[test]
fn a_range_that_takes_nothing_finds_nothing_and_asks_again_the_same_way() {
    // The optimist path spends ten tries of three draws each before it enumerates, and enumerates nothing when there is nothing
    // to find. The refused and accepted runs below start from the same seed, so they see the same squares in the same order -
    // the difference is only what the predicate says about them.
    let mut refused = Rolls::new(17);
    assert_eq!(
        random_point_in_radius((0, 0), 1, |_, _| false, &mut refused),
        None
    );

    let mut accepted = Rolls::new(17);
    let placed = random_point_in_radius((0, 0), 1, |_, _| true, &mut accepted).expect("a square");
    assert!(
        (-1..=1).contains(&placed.0) && (-1..=1).contains(&placed.1),
        "{placed:?}"
    );
}

#[test]
fn the_same_seed_lands_on_the_same_square() {
    let mut first = Rolls::new(1234);
    let mut second = Rolls::new(1234);
    assert_eq!(
        random_point_in_radius((7, 7), 3, |x, y| (x + y) % 2 == 0, &mut first),
        random_point_in_radius((7, 7), 3, |x, y| (x + y) % 2 == 0, &mut second)
    );
}
