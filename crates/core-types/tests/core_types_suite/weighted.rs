//! Choosing from a list where each entry carries a weight.
//!
//! Reference: `tests/weighted_list_test.cpp` (basic function, distribution,
//! weights after remove, add-or-replace with zero) and `src/weighted_list.h:41-273`.
//!
//! Upstream checks the distribution statistically over 4,000 draws; here the
//! draw is passed in explicitly wherever a specific entry is expected, so the
//! rule is pinned exactly and the test cannot flake.

use cdda_core_types::rng::Rolls;
use cdda_core_types::weighted::{WeightedFloatList, WeightedIntList};

const A: i32 = 1;
const B: i32 = 2;
const C: i32 = 3;

#[test]
fn a_weight_that_is_not_positive_is_refused() {
    // `weighted_list.h:41-49`: the entry is not stored and the total does not move.
    let mut list = WeightedIntList::new();
    assert!(!list.add(A, -1));
    assert!(!list.add(A, 0));
    assert_eq!(list.total_weight(), 0);
    assert_eq!(list.specific_weight(&A), 0);
    assert_eq!(list.len(), 0);
    assert!(list.pick_with(0).is_none());
}

#[test]
fn a_single_entry_is_the_only_thing_picked() {
    let mut list = WeightedIntList::new();
    assert!(list.add(A, 1));
    assert_eq!(list.total_weight(), 1);
    assert_eq!(list.specific_weight(&A), 1);
    // `pick_ent` returns 0 for a one-entry list whatever the draw is.
    for draw in [0, 1, 4_294_967_295] {
        assert_eq!(list.pick_with(draw), Some(&A));
    }
}

#[test]
fn clearing_empties_the_list_and_its_total() {
    let mut list = WeightedIntList::new();
    list.add(A, 5);
    list.add(B, 3);
    list.clear();
    assert_eq!(list.total_weight(), 0);
    assert_eq!(list.len(), 0);
    assert!(list.pick_with(0).is_none());
}

#[test]
fn removing_an_entry_takes_its_weight_with_it() {
    let mut list = WeightedIntList::new();
    list.add(B, 3);
    list.add(C, 4);
    assert_eq!(list.total_weight(), 7);

    list.remove(&B);
    assert_eq!(list.total_weight(), 4);
    assert_eq!(list.specific_weight(&B), 0);
    assert_eq!(list.specific_weight(&C), 4);
    // Only C is left, so every draw lands on it.
    for draw in [0, 1, 2, 3, 4, 100] {
        assert_eq!(list.pick_with(draw), Some(&C));
    }
}

#[test]
fn the_draw_decides_by_accumulated_weight() {
    // `weighted_list.h:244-268`: `picked = (randi % total) + 1`, then the first
    // entry whose running total reaches it. Weights 3, 1, 5 over a total of 9.
    let mut list = WeightedIntList::new();
    list.add(A, 3);
    list.add(B, 1);
    list.add(C, 5);

    let expected = [
        (0, A), // picked 1, inside A's 1..=3
        (2, A), // picked 3, the last of A
        (3, B), // picked 4, the whole of B
        (4, C), // picked 5, the first of C
        (8, C), // picked 9, the last of C
        (9, A), // 9 % 9 + 1 = 1, wraps to the front
    ];
    let mut checked = 0;
    for (draw, winner) in expected {
        assert_eq!(
            list.pick_with(draw),
            Some(&winner),
            "draw {draw} should land in {winner}'s band"
        );
        checked += 1;
    }
    println!("weighted pick: {checked} draws land in their weight bands");
}

#[test]
fn adding_or_replacing_can_also_delete() {
    // `weighted_list.h:73-77`: a zero weight removes the entry rather than
    // storing a useless one, and a negative weight is refused outright.
    let mut list = WeightedIntList::new();
    list.add(A, 4);
    assert!(!list.add_or_replace(A, 0));
    assert_eq!(list.specific_weight(&A), 0);
    assert_eq!(list.total_weight(), 0);

    list.add(A, 4);
    assert!(list.add_or_replace(A, 9));
    assert_eq!(list.specific_weight(&A), 9);
    assert_eq!(list.total_weight(), 9);
    assert_eq!(list.len(), 1, "replacing does not add a second entry");

    assert!(!list.add_or_replace(B, -3));
    assert_eq!(list.total_weight(), 9);
}

#[test]
fn picking_from_an_empty_list_still_costs_a_draw() {
    // `weighted_list.h:144-146`: `pick()` takes its number before it checks
    // whether there is anything to pick, so an empty list advances the stream.
    // A port that returned early would desynchronise every later roll.
    let mut list: WeightedIntList<i32> = WeightedIntList::new();
    let mut rolls = Rolls::new(12_345);
    assert_eq!(list.pick(&mut rolls), None);

    let mut fresh = Rolls::new(12_345);
    let _ = fresh.rng_bits();
    assert_eq!(rolls.rng(0, 100), fresh.rng(0, 100));
}

#[test]
fn a_fractional_list_weights_its_draw_against_the_total() {
    // Reference: `weighted_list.h:277-296`. The whole-number list picks with
    // `draw % total`; this one scales the draw across the whole 32-bit range, so
    // a draw of zero selects the very front and the maximum draw selects the very
    // back. Weights 0.5, 0.25, 0.25 over a total of 1.0.
    let mut list: WeightedFloatList<char> = WeightedFloatList::new();
    assert!(list.add('a', 0.5));
    assert!(list.add('b', 0.25));
    assert!(list.add('c', 0.25));
    assert!((list.total_weight() - 1.0).abs() < f64::EPSILON);

    let max = u32::MAX;
    // The bands are [0, 0.5) for a, [0.5, 0.75) for b, [0.75, 1.0] for c, and the
    // draw is scaled onto that, so "a quarter of the way in" is still inside a.
    assert_eq!(list.pick_with(0), Some(&'a'), "the front of the range");
    assert_eq!(list.pick_with(max / 4), Some(&'a'));
    assert_eq!(
        list.pick_with(max / 2),
        Some(&'a'),
        "half is just short of a's end"
    );
    assert_eq!(list.pick_with((max / 4) * 3), Some(&'b'));
    assert_eq!(list.pick_with(max - 1), Some(&'c'));
    assert_eq!(list.pick_with(max), Some(&'c'), "the back of the range");
}

#[test]
fn a_fractional_list_refuses_an_empty_weight_too() {
    // Same refusal as the whole-number list (`weighted_list.h:41-49`), because
    // both inherit it from the same base.
    let mut list: WeightedFloatList<char> = WeightedFloatList::new();
    assert!(!list.add('a', 0.0));
    assert!(!list.add('a', -0.5));
    assert_eq!(list.len(), 0);
    assert_eq!(list.pick_with(0), None);
}
