//! Weighted lists: tickets, the walk, and the empty case.

#![allow(clippy::panic)]

use cdda_core_types::rng::Rolls;
use cdda_core_types::weighted::WeightedList;

#[test]
fn a_list_holds_the_sum_of_its_weights() {
    let list = WeightedList::from_entries([("a", 3), ("b", 1)]);
    assert_eq!(list.total_weight(), 4);
    assert_eq!(list.len(), 2);
    assert!(!list.is_empty());
}

#[test]
fn a_zero_weight_entry_cannot_be_picked() {
    let list = WeightedList::from_entries([("never", 0), ("always", 5)]);
    assert_eq!(list.len(), 1, "the zero entry holds no tickets");
    assert_eq!(list.total_weight(), 5);
    for bits in 0..50 {
        assert_eq!(list.pick_by_bits(bits), Some(&"always"));
    }
}

#[test]
fn the_walk_gives_each_entry_its_own_span() {
    let list = WeightedList::from_entries([("a", 2), ("b", 3)]);
    // Tickets 0 and 1 are a's, 2, 3 and 4 are b's, and anything beyond wraps.
    assert_eq!(list.pick_by_bits(0), Some(&"a"));
    assert_eq!(list.pick_by_bits(1), Some(&"a"));
    assert_eq!(list.pick_by_bits(2), Some(&"b"));
    assert_eq!(list.pick_by_bits(4), Some(&"b"));
    assert_eq!(list.pick_by_bits(5), Some(&"a"), "five wraps to zero");
}

#[test]
fn an_empty_list_picks_nothing() {
    let list: WeightedList<&str> = WeightedList::new();
    assert!(list.is_empty());
    assert_eq!(list.pick_by_bits(0), None);
    let mut rolls = Rolls::new(1);
    assert_eq!(list.pick(&mut rolls), None);
}

#[test]
fn the_dice_draw_the_same_entry_rather_than_a_second_one() {
    let list = WeightedList::from_entries([("a", 1), ("b", 1), ("c", 1)]);
    let mut first = Rolls::new(31337);
    let mut second = Rolls::new(31337);
    for _ in 0..20 {
        assert_eq!(list.pick(&mut first), list.pick(&mut second));
    }
}

#[test]
fn a_heavy_entry_is_picked_more_often_but_not_always() {
    let list = WeightedList::from_entries([("common", 9), ("rare", 1)]);
    let mut rolls = Rolls::new(4242);
    let mut common = 0;
    let mut rare = 0;
    for _ in 0..2000 {
        match list.pick(&mut rolls) {
            Some(&"common") => common += 1,
            Some(&"rare") => rare += 1,
            _ => panic!("a pick"),
        }
    }
    assert!(common > rare * 3, "nine to one: {common} against {rare}");
    assert!(rare > 0, "and the rare one still happens: {rare}");
}
