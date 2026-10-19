//! Container parity with the reference.
//!
//! Reference: `src/colony.h`, `src/cata_bitset.h` and `src/cata_bitset.cpp` at
//! tag `0.I-1` (commit `7b2efa5`), plus the reference's own test file
//! `tests/colony_test.cpp`. Test names below cite the reference case and line
//! numbers they are translated from.
//!
//! Two kinds of divergence are called out where they occur:
//! * place where C++ relies on pointer identity or undefined behaviour, and a
//!   Rust container cannot copy it literally;
//! * place where the reference's behaviour is an artefact of its allocator and
//!   is therefore reported rather than reproduced.

// Tests fail by panicking; the workspace denies `expect_used`/`panic` in
// library code, not in how a failing test reports itself.
#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::containers::{Colony, Handle, TinyBitset};

/// The handle of the element at `index` in iteration order, i.e. the
/// reference's `get_iterator_from_index()` (`src/colony.h:3205-3212`), built
/// from `begin()` and repeated `next()`.
fn handle_at(colony: &Colony<i32>, index: usize) -> Handle {
    let mut handle = colony.begin().expect("begin");
    for _ in 0..index {
        handle = colony.next(handle).expect("index in range");
    }
    handle
}

#[test]
fn colony_starts_empty_and_insert_returns_a_handle() {
    // Translated from "colony_basics" (tests/colony_test.cpp:33-51).
    let mut colony: Colony<i32> = Colony::new();

    assert!(colony.is_empty());
    assert_eq!(colony.size(), 0);
    assert_eq!(colony.begin(), None);

    let ten = colony.insert(10);

    assert!(!colony.is_empty());
    assert_eq!(colony.size(), 1);
    assert_eq!(colony.get(ten), Some(&10));
    assert_eq!(colony.begin(), Some(ten));
    // begin() != end() after an insert, and end() is reached in one step.
    assert_eq!(colony.next(ten), None);

    colony.clear();

    // begin() == end() after clear.
    assert_eq!(colony.begin(), None);
    assert!(colony.is_empty());
}

#[test]
fn colony_iterates_in_insertion_order() {
    // Translated from "colony_basics" (tests/colony_test.cpp:54-69): 400
    // elements inserted alternately, counted and summed through iteration.
    let mut colony: Colony<i32> = Colony::new();
    for _ in 0..200 {
        colony.insert(10);
        colony.insert(20);
    }

    let mut count = 0;
    let mut sum = 0;
    for value in &colony {
        count += 1;
        sum += *value;
    }

    assert_eq!(count, 400);
    assert_eq!(sum, 6000);
}

#[test]
fn colony_erase_keeps_survivors_in_order_and_returns_the_next_element() {
    // Translated from "colony_basics" (tests/colony_test.cpp:180-190): erase
    // every second element, exactly as the reference's erase-and-advance loop
    // does, then count what is left.
    let mut colony: Colony<i32> = Colony::new();
    let handles: Vec<Handle> = (0..20).map(|i| colony.insert(i)).collect();

    let mut it = colony
        .next(colony.begin().expect("begin"))
        .expect("second element");
    let mut erased = 0;
    // The reference loop is `for( it = ++begin(); it < end(); ++it ) { it = erase( it ); }`,
    // so the erase and the increment each advance by one.
    loop {
        if colony.get(it).is_none() {
            break;
        }
        erased += 1;
        let Some(after_erase) = colony.erase(it) else {
            break;
        };
        let Some(after_increment) = colony.next(after_erase) else {
            break;
        };
        it = after_increment;
    }

    assert_eq!(erased, 10);
    assert_eq!(colony.size(), 10);
    assert!(!colony.is_empty());

    // Iteration order of the survivors is unchanged by the erasures: the
    // reference never moves an element (src/colony.h:1740-1754).
    let survivors: Vec<i32> = colony.iter().copied().collect();
    assert_eq!(survivors, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);

    // Handles to elements that were not erased still resolve ...
    let first = *handles.first().expect("first handle");
    let second_to_last = *handles.get(18).expect("handle of the last survivor");
    let last = *handles.last().expect("last handle");
    assert_eq!(colony.get(first), Some(&0));
    assert_eq!(colony.get(second_to_last), Some(&18));
    // The loop erased indices 1, 3, ... 19, so the final handle is stale.
    assert_eq!(colony.get(last), None);
    // ... while handles to erased elements do not.
    let erased_handle = *handles.get(1).expect("second handle");
    assert_eq!(colony.get(erased_handle), None);
}

#[test]
fn colony_erase_returns_end_at_the_back_and_rejects_stale_handles() {
    let mut colony: Colony<i32> = Colony::new();
    let first = colony.insert(1);
    let second = colony.insert(2);
    let third = colony.insert(3);

    // Erasing the back element returns end() (src/colony.h:1740-1754).
    assert_eq!(colony.erase(third), None);
    assert_eq!(colony.size(), 2);
    assert_eq!(colony.get(third), None);

    // Erasing the first element returns its successor.
    assert_eq!(colony.erase(first), Some(second));

    // A stale handle is a no-op here and reports end(); the reference asserts
    // (src/colony.h:1763) and is undefined behaviour in a release build.
    assert_eq!(colony.erase(third), None);
    assert_eq!(colony.size(), 1);
    assert_eq!(colony.get(second), Some(&2));
}

#[test]
fn colony_insert_reuses_the_most_recently_erased_slot() {
    // Reference: insert()'s "reuse previous-erased element locations" branch
    // takes the head of the group's free list (src/colony.h:1019-1025), and
    // erase() pushes each newly erased slot onto that head
    // (src/colony.h:1792-1812). The most recently erased slot is therefore
    // reused first, and the new element appears *inside* iteration order
    // rather than at the back.
    let mut colony: Colony<i32> = Colony::new();
    let handles: Vec<Handle> = (0..8).map(|i| colony.insert(i)).collect();
    let two = *handles.get(2).expect("2");
    let five = *handles.get(5).expect("5");
    colony.erase(two);
    colony.erase(five);

    let hundred = colony.insert(100);
    assert_eq!(
        colony.iter().copied().collect::<Vec<i32>>(),
        vec![0, 1, 3, 4, 100, 6, 7]
    );
    assert_eq!(colony.get(hundred), Some(&100));

    let hundred_one = colony.insert(101);
    assert_eq!(
        colony.iter().copied().collect::<Vec<i32>>(),
        vec![0, 1, 101, 3, 4, 100, 6, 7]
    );
    assert_eq!(colony.get(hundred_one), Some(&101));
    assert_eq!(colony.size(), 8);
}

#[test]
fn colony_insert_reuses_the_start_of_a_merged_run_of_holes() {
    // Reference: erasing a slot adjacent to an existing run merges the two and
    // moves the free-list node to the start of the merged run
    // (src/colony.h:1814-1848); insert() then consumes a run from its start
    // (src/colony.h:1035-1055). A plain LIFO free list would hand out slot 5
    // first; the reference hands out slot 4.
    let mut colony: Colony<i32> = Colony::new();
    let handles: Vec<Handle> = (0..8).map(|i| colony.insert(i)).collect();
    let four = *handles.get(4).expect("4");
    let five = *handles.get(5).expect("5");

    colony.erase(four); // run of one at slot 4
    colony.erase(five); // merges with it; the run's node moves to slot 4

    colony.insert(100);
    assert_eq!(
        colony.iter().copied().collect::<Vec<i32>>(),
        vec![0, 1, 2, 3, 100, 6, 7]
    );
    colony.insert(101);
    assert_eq!(
        colony.iter().copied().collect::<Vec<i32>>(),
        vec![0, 1, 2, 3, 100, 101, 6, 7]
    );
}

#[test]
fn colony_spans_groups_and_erases_across_the_boundary() {
    // Reference: elements live in groups of at most max_group_size slots, a new
    // group is allocated when the last one is full (src/colony.h:978-1018), and
    // iteration walks the group chain (operator++, src/colony.h:278-298).
    let mut colony: Colony<usize> = Colony::new();
    let handles: Vec<Handle> = (0..20).map(|i| colony.insert(i)).collect();

    assert_eq!(
        colony.iter().copied().collect::<Vec<usize>>(),
        (0..20).collect::<Vec<usize>>()
    );
    // The first group is the default minimum size and each new group is as
    // large as the colony was: 8 + 8 + 16 slots for 20 elements.
    assert_eq!(colony.capacity(), 32);

    // Erasing the last element of one group returns the first element of the
    // next group, so iteration order is still contiguous.
    let last_of_first = *handles.get(7).expect("7");
    let first_of_second = *handles.get(8).expect("8");
    assert_eq!(colony.erase(last_of_first), Some(first_of_second));
    assert_eq!(colony.get(last_of_first), None);
    assert_eq!(colony.get(first_of_second), Some(&8));
    assert_eq!(colony.size(), 19);
}

#[test]
fn colony_removing_a_groups_last_element_destroys_the_group() {
    // Reference: a group that loses its last element is deallocated and its
    // capacity released (src/colony.h:1894-1976). Nothing is moved, so the
    // survivors keep their order and their handles.
    let mut colony: Colony<usize> = Colony::new();
    let handles: Vec<Handle> = (0..20).map(|i| colony.insert(i)).collect();
    let capacity_before = colony.capacity();

    for index in 0..8 {
        let handle = *handles.get(index).expect("handle");
        let returned = colony.erase(handle);
        // Each erasure returns the next element, including across the group
        // boundary when the group is destroyed.
        let next = *handles.get(index + 1).expect("next handle");
        assert_eq!(returned, Some(next));
    }

    assert_eq!(colony.capacity(), capacity_before - 8);
    assert_eq!(
        colony.iter().copied().collect::<Vec<usize>>(),
        (8..20).collect::<Vec<usize>>()
    );
    for index in 0..8 {
        let handle = *handles.get(index).expect("handle");
        assert_eq!(colony.get(handle), None);
    }

    // A new element cannot land in the destroyed group's memory: the reference
    // deallocated it (src/colony.h:1924-1926), so this appends at the back.
    colony.insert(99);
    let values = colony.iter().copied().collect::<Vec<usize>>();
    assert_eq!(values.len(), 13);
    assert_eq!(values.last(), Some(&99));
}

#[test]
fn colony_range_erase_keeps_the_remaining_elements_in_order() {
    // Translated from "colony_range_erase" (tests/colony_test.cpp:508-597),
    // which advances from begin() to a pair of positions and erases between
    // them, then counts what is left.
    let mut colony: Colony<i32> = Colony::new();
    for i in 0..1000 {
        colony.insert(i);
    }

    // [500, 800): "simple range erase 1"
    let a = handle_at(&colony, 500);
    let b = handle_at(&colony, 800);
    colony.erase_range(a, Some(b));
    assert_eq!(colony.iter().count(), 700);
    assert_eq!(colony.size(), 700);
    assert_eq!(colony.get(a), None);

    // [400, 500): "simple range erase 2", past the point of the previous
    // erasures: iteration index 400 is element 400 and index 500 is element 800.
    let a = handle_at(&colony, 400);
    let b = handle_at(&colony, 500);
    colony.erase_range(a, Some(b));
    assert_eq!(colony.iter().count(), 600);
    assert_eq!(colony.size(), 600);

    // [4, 9): "simple range erase 3"
    let a = handle_at(&colony, 4);
    let b = handle_at(&colony, 9);
    colony.erase_range(a, Some(b));
    assert_eq!(colony.iter().count(), 595);
    assert_eq!(colony.size(), 595);

    // [begin, begin + 50): "range erase from begin()"
    let a = handle_at(&colony, 0);
    let b = handle_at(&colony, 50);
    colony.erase_range(a, Some(b));
    assert_eq!(colony.iter().count(), 545);
    assert_eq!(colony.size(), 545);

    // [index 345, end()): "range erase to end()", after which only the 345
    // elements before it remain.
    let a = handle_at(&colony, 345);
    colony.erase_range(a, None);
    assert_eq!(colony.iter().count(), 345);
    assert_eq!(colony.size(), 345);
    assert_eq!(colony.last().and_then(|h| colony.get(h)), Some(&399));
}

#[test]
fn colony_range_erase_to_end_empties_the_colony_and_releases_its_groups() {
    let mut colony: Colony<i32> = Colony::new();
    let handles: Vec<Handle> = (0..20).map(|i| colony.insert(i)).collect();
    let first = *handles.first().expect("first");

    // An empty range does nothing (tests/colony_test.cpp:764-768).
    colony.erase_range(first, Some(first));
    assert_eq!(colony.size(), 20);
    assert!(!colony.is_empty());

    // Erasing [begin, end()) releases every group: the reference's range erase
    // calls blank() when the colony empties (src/colony.h:2286-2289), whereas
    // single erasure keeps the final group's allocation (case 0,
    // src/colony.h:1898-1912).
    colony.erase_range(first, None);
    assert!(colony.is_empty());
    assert_eq!(colony.capacity(), 0);
    assert_eq!(colony.begin(), None);
    for handle in &handles {
        assert_eq!(colony.get(*handle), None);
    }

    // The colony is still usable afterwards.
    let ten = colony.insert(10);
    assert_eq!(colony.get(ten), Some(&10));
    assert_eq!(colony.size(), 1);
}

#[test]
fn colony_fill_insert_and_range_insert() {
    // Translated from "colony_insertion_methods" (tests/colony_test.cpp:813-894).
    let mut colony: Colony<i32> = Colony::new();
    let many = colony.insert_many(5000, 2);
    assert_eq!(many.len(), 5000);
    assert_eq!(colony.size(), 5000);
    // A fill insert into a colony with no groups sizes its first group to the
    // request rather than following the single-insert growth curve
    // (src/colony.h:1543-1550).
    assert_eq!(colony.capacity(), 5000);

    let inserted = colony.insert_range(vec![7; 500]);
    assert_eq!(inserted.len(), 500);
    assert_eq!(colony.size(), 5500);
    let sum: i32 = colony.iter().sum();
    assert_eq!(sum, 5000 * 2 + 500 * 7);
}

#[test]
fn colony_fill_insert_reuses_holes_before_appending() {
    // Reference: fill insert "use[s] up erased locations if available"
    // (src/colony.h:1553-1607), consuming each free-list block from its start,
    // so a run of copies lands partly in the middle of iteration order.
    let mut colony: Colony<i32> = Colony::new();
    let handles: Vec<Handle> = (0..8).map(|i| colony.insert(i)).collect();
    colony.erase(*handles.get(2).expect("2"));
    colony.erase(*handles.get(5).expect("5"));

    let inserted = colony.insert_many(3, 9);
    assert_eq!(inserted.len(), 3);
    assert_eq!(colony.size(), 9);
    // Most recently erased slot first (5, then 2), then the back.
    assert_eq!(
        colony.iter().copied().collect::<Vec<i32>>(),
        vec![0, 1, 9, 3, 4, 9, 6, 7, 9]
    );
}

#[test]
fn colony_advance_distance_and_index_conversions() {
    // Translated from "colony_basics" (tests/colony_test.cpp:71-93) and
    // "colony_insert_and_erase" (tests/colony_test.cpp:444-468).
    let mut colony: Colony<i32> = Colony::new();
    for i in 0..400 {
        colony.insert(i);
    }
    let begin = colony.begin().expect("begin");

    let plus_20 = colony.advance(begin, 20).expect("20");
    let plus_200 = colony.advance(begin, 200).expect("200");
    // Iterator + distance, and iterator - distance
    assert_eq!(colony.distance(begin, plus_20), Some(20));
    assert_eq!(colony.distance(plus_200, begin), Some(-200));
    // get_index_from_iterator / get_iterator_from_index
    assert_eq!(colony.index_of(plus_20), Some(20));
    assert_eq!(colony.at_index(20), Some(plus_20));
    assert_eq!(colony.at_index(399), colony.last());
    assert_eq!(colony.at_index(400), None);
    assert_eq!(colony.advance(begin, -1), None);
    assert_eq!(colony.advance(begin, 400), None);

    // get_iterator_from_pointer: an element reached through a reference maps
    // back to its handle (src/colony.h:3120-3150).
    let handle = colony.at_index(37).expect("37");
    let element = colony.get(handle).expect("element");
    assert_eq!(colony.handle_of(element), Some(handle));

    // next()/prev() by a distance (src/colony.h:2959-2990).
    let five = colony.advance(begin, 5).expect("5");
    assert_eq!(colony.distance(begin, five), Some(5));
    let before_last = colony
        .previous(colony.last().expect("last"))
        .expect("penultimate");
    assert_eq!(colony.index_of(before_last), Some(398));
}

#[test]
fn colony_index_lookups_survive_erasures() {
    // Translated from "colony_insert_and_erase" (tests/colony_test.cpp:444-468):
    // "check edge-case with advance when erasures present in initial group".
    let mut colony: Colony<i32> = Colony::new();
    for i in 0..1000 {
        colony.insert(i);
    }

    let it = colony
        .advance(colony.begin().expect("begin"), 2)
        .expect("2");
    assert_eq!(colony.index_of(it), Some(2));

    // Erasing the element at index 2 leaves the surviving indices unchanged.
    let erased = it;
    colony.erase(it);
    let it = colony
        .advance(colony.begin().expect("begin"), 500)
        .expect("500");
    assert_eq!(colony.index_of(it), Some(500));
    // Pointer-to-iterator and index-to-iterator both find the same element.
    assert_eq!(colony.handle_of(colony.get(it).expect("element")), Some(it));
    assert_eq!(colony.at_index(500), Some(it));
    // The erased handle has no index any more (the reference's
    // get_iterator_from_pointer() returns end() for an erased element,
    // src/colony.h:3130-3140).
    assert_eq!(colony.index_of(erased), None);
    assert_eq!(colony.advance(erased, 1), None);
}

#[test]
fn colony_group_sizes_and_capacity_match_the_reference() {
    // Translated from "colony_group_size_and_capacity"
    // (tests/colony_test.cpp:930-971), including its exact capacity numbers,
    // which follow from the reference's group allocation policy
    // (src/colony.h:982-984) and its rebuilding path (src/colony.h:1699-1719).
    let mut colony: Colony<i32> = Colony::new();
    assert!(colony.change_group_sizes(50, 100));

    colony.insert(27);
    // Change_group_sizes min-size.
    assert_eq!(colony.capacity(), 50);

    for i in 0..100 {
        colony.insert(i);
    }
    // Change_group_sizes max-size: 50 + 50 + 100 slots.
    assert_eq!(colony.capacity(), 200);

    colony.reinitialize(200, 2000);
    colony.insert(27);
    // Reinitialize min-size.
    assert_eq!(colony.capacity(), 200);

    for i in 0..3300 {
        colony.insert(i);
    }
    // Reinitialize max-size: 200 + 200 + 400 + 800 + 1600 + 2000 slots.
    assert_eq!(colony.capacity(), 5200);

    assert!(colony.change_group_sizes(500, 500));
    // Change_group_sizes resize: seven groups of 500.
    assert_eq!(colony.capacity(), 3500);

    assert!(colony.change_minimum_group_size(200));
    // Change_maximum_group_size resize: no rebuild while the groups still fit.
    assert_eq!(colony.capacity(), 3500);

    assert!(colony.change_maximum_group_size(200));
    // Now the 500-slot groups no longer fit: seventeen groups of 200.
    assert_eq!(colony.capacity(), 3400);

    // Sizes the reference asserts on are rejected here (src/colony.h:2345).
    assert!(!colony.change_group_sizes(2, 10));
    assert!(!colony.change_group_sizes(10, 5));
    assert_eq!(colony.capacity(), 3400);
}

#[test]
fn colony_shrink_to_fit_and_reserve_match_the_reference() {
    // Translated from "colony_basics" (tests/colony_test.cpp:192-196) and
    // "colony_insertion_methods" (tests/colony_test.cpp:842-893).
    let mut colony: Colony<i32> = Colony::new();
    for i in 0..400 {
        colony.insert(i);
    }
    // Erase every second element, as in the reference loop.
    let mut it = colony
        .advance(colony.begin().expect("begin"), 1)
        .expect("1");
    while let Some(after_erase) = colony.erase(it) {
        let Some(after_increment) = colony.next(after_erase) else {
            break;
        };
        it = after_increment;
    }
    assert_eq!(colony.size(), 200);

    let before = colony.capacity();
    colony.shrink_to_fit();
    assert!(colony.capacity() < before);
    assert_eq!(colony.capacity(), 200);

    // reserve() on an empty colony allocates a single group of the requested
    // size (src/colony.h:2531-2542), even after the minimum group size drops.
    colony.clear();
    assert!(colony.change_minimum_group_size(3));
    colony.reserve(1000);
    assert_eq!(colony.capacity(), 1000);
    let handle = colony.insert(1);
    assert_eq!(colony.get(handle), Some(&1));
    assert_eq!(colony.capacity(), 1000);
}

/// The reference's own fast xorshift generator, translated verbatim from
/// tests/colony_test.cpp:14-29 so that the reference's random sequences, and
/// therefore its expected sizes, are reproduced exactly.
struct XorRand {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
}

impl XorRand {
    fn new() -> Self {
        Self {
            x: 123_456_789,
            y: 362_436_069,
            z: 521_288_629,
            w: 88_675_123,
        }
    }

    fn next(&mut self) -> u32 {
        let t = self.x ^ (self.x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w = self.w ^ (self.w >> 19) ^ (t ^ (t >> 8));
        self.w
    }
}

#[test]
fn colony_sort_orders_the_elements() {
    // Translated from "colony_sort" (tests/colony_test.cpp:771-811), with the
    // reference's xor_rand() and a smaller element count.
    let mut rng = XorRand::new();
    let mut colony: Colony<i32> = Colony::new();
    colony.reserve(2000);
    for _ in 0..2000 {
        colony.insert(i32::try_from(rng.next() & 65535).expect("16 bits fit"));
    }
    let stale = colony.begin().expect("begin");

    // Less-than sort test.
    colony.sort();
    let mut previous = i32::MIN;
    for value in &colony {
        assert!(previous <= *value);
        previous = *value;
    }
    // Sorting rebuilds the colony, so handles from before do not resolve
    // (src/colony.h:3280-3320).
    assert_eq!(colony.get(stale), None);
    assert_eq!(colony.capacity(), 2000);

    // Greater-than sort test.
    colony.sort_by(|a, b| b.cmp(a));
    let mut previous = i32::MAX;
    for value in &colony {
        assert!(previous >= *value);
        previous = *value;
    }
    assert_eq!(colony.size(), 2000);
}

// The `& 7` / `& 3` masks below are the reference's own expressions
// (tests/colony_test.cpp:302, 327, 363); keeping them literal is the point of
// the translation, so the lint's bit-twiddling rewrite is declined.
#[allow(clippy::verbose_bit_mask)]
#[test]
fn colony_random_insert_and_erase_matches_the_reference() {
    // Translated from "colony_insert_and_erase" (tests/colony_test.cpp:300-373)
    // with the reference's xor_rand() and reduced element counts.
    let mut rng = XorRand::new();
    let mut colony: Colony<i32> = Colony::new();
    for i in 0..500 {
        colony.insert(i);
    }

    // "Erase alternating": the reference's `for( ...; ++it ) it = erase( it )`
    // erases every second element.
    let mut it = colony.begin();
    while let Some(handle) = it {
        let Some(after_erase) = colony.erase(handle) else {
            break;
        };
        it = colony.next(after_erase);
    }
    assert_eq!(colony.size(), 250);

    // "Erase randomly till-empty".
    loop {
        let mut it = colony.begin();
        while let Some(handle) = it {
            it = if rng.next() & 7 == 0 {
                colony.erase(handle)
            } else {
                colony.next(handle)
            };
        }
        if colony.is_empty() {
            break;
        }
        assert_eq!(colony.iter().count(), colony.size());
    }
    assert!(colony.is_empty());

    // "Random insert/erase till empty".
    colony.insert_many(500, 1);
    let mut rounds = 0;
    loop {
        let mut it = colony.begin();
        while let Some(handle) = it {
            if rng.next() & 3 == 0 {
                colony.insert(1);
                it = colony.next(handle);
            } else {
                it = colony.erase(handle);
            }
        }
        if colony.is_empty() {
            break;
        }
        assert_eq!(colony.iter().count(), colony.size());
        rounds += 1;
        assert!(rounds < 10_000, "random insert/erase did not drain");
    }
    assert!(colony.is_empty());
}

// `& 7` is the reference's own mask (tests/colony_test.cpp:327).
#[allow(clippy::verbose_bit_mask)]
#[test]
fn colony_random_erase_and_reinsert_matches_the_reference_sizes() {
    // Translated from "colony_insert_and_erase" (tests/colony_test.cpp:314-359),
    // keeping the reference's xor_rand(), its group sizes and its exact
    // expected sizes. The 45001 at the end only comes out right if insertion
    // fills holes in exactly the reference's order.
    let mut rng = XorRand::new();
    let mut colony: Colony<i32> = Colony::new();
    assert!(colony.change_minimum_group_size(10000));

    for _ in 0..30000 {
        colony.insert(1);
    }
    assert_eq!(colony.size(), 30000);

    // "Erase randomly till half-empty".
    let mut count = 0;
    loop {
        let mut it = colony.begin();
        while let Some(handle) = it {
            if rng.next() & 7 == 0 {
                it = colony.erase(handle);
                count += 1;
            } else {
                it = colony.next(handle);
            }
        }
        if count >= 15000 {
            break;
        }
    }
    assert_eq!(colony.size(), 30000 - count);
    assert_eq!(colony.iter().count(), colony.size());

    // "Size after reinsert".
    for _ in 0..count {
        colony.insert(1);
    }
    assert_eq!(colony.size(), 30000);

    // "Alternating insert/erase": every third visited element is erased, the
    // others trigger an insertion, which can extend the iteration.
    let mut counter = 0;
    let mut it = colony.begin();
    while let Some(handle) = it {
        counter += 1;
        if counter == 3 {
            counter = 0;
            it = colony.erase(handle);
        } else {
            colony.insert(1);
            it = colony.next(handle);
        }
    }
    assert_eq!(colony.size(), 45001);
}

#[test]
fn tiny_bitset_defaults_and_construction() {
    // Reference: the default constructor asks for `kMaxInlineBits`, the bits
    // that fit in one `uintptr_t` less the control byte, so 64 - 8 = 56 on a
    // 64-bit target (src/cata_bitset.h:33-35, 55-57).
    let default_bits = TinyBitset::new();
    assert_eq!(default_bits.size(), 56);
    assert_eq!(default_bits.capacity(), 56);
    assert!(default_bits.none());
    assert!(!default_bits.any());
    // The default bitset is zeroed, so not every bit is set ...
    assert!(!default_bits.all());
    assert_eq!(default_bits.count(), 0);

    // ... but a range with no bits is vacuously all-set: `and_mapper` starts at
    // `~0` and `map()` returns immediately for an empty bitset
    // (src/cata_bitset.h:203-208, 257-262).
    let mut empty = TinyBitset::with_size(0);
    assert_eq!(empty.size(), 0);
    assert!(empty.all());
    assert!(empty.none());
    assert!(!empty.any());
    // The reference asserts `idx < size()` (src/cata_bitset.h:118); an
    // out-of-range access reports "not set" here instead of asserting.
    assert!(!empty.test(0));
    assert!(!empty.set(0));

    let one = TinyBitset::with_size(1);
    assert_eq!(one.size(), 1);
    assert!(one.none());
    assert!(!one.test(0));
}

#[test]
fn tiny_bitset_bit_positions_across_word_boundaries() {
    // The reference packs 64 bits per word (src/cata_bitset.h:33-34), so these
    // are the sizes where the last word is partly used.
    for size in [0_usize, 1, 63, 64, 65, 127, 128, 129] {
        let mut bits = TinyBitset::with_size(size);
        assert_eq!(bits.size(), size);
        assert!(bits.none(), "size {size} starts clear");
        assert_eq!(bits.count(), 0);
        assert_eq!(bits.all(), size == 0, "all() on an empty range is vacuous");

        for index in 0..size {
            assert!(!bits.test(index), "size {size} bit {index} starts clear");
            assert!(bits.set(index), "size {size} bit {index} is in range");
            assert!(bits.test(index), "size {size} bit {index} is set");
            assert_eq!(bits.count(), index + 1);
            if index > 0 {
                assert!(
                    bits.test(index - 1),
                    "size {size} bit {index} kept its neighbour"
                );
            }
        }
        if size > 0 {
            assert!(bits.all(), "size {size} all bits set");
            assert!(bits.any());
            // Clearing one bit is enough to make all() false.
            let middle = size / 2;
            assert!(bits.clear(middle));
            assert!(!bits.all());
            if size > 1 {
                assert!(bits.any());
            }
            assert_eq!(bits.count(), size - 1);
            assert!(!bits.test(middle));
        }

        // Every index past the end is rejected rather than asserting
        // (src/cata_bitset.h:118).
        assert!(!bits.test(size));
        assert!(!bits.set(size));
        assert!(!bits.clear(size));
        assert!(!bits.flip(size));
    }

    // Bits on either side of a word boundary are independent.
    let mut two_words = TinyBitset::with_size(129);
    assert!(two_words.set(63) && two_words.set(64) && two_words.set(127));
    assert!(two_words.set(128));
    assert!(two_words.test(63) && two_words.test(64));
    assert!(two_words.test(127) && two_words.test(128));
    assert_eq!(two_words.count(), 4);
    assert!(two_words.clear(64));
    assert!(two_words.test(63) && !two_words.test(64));
    assert!(two_words.test(127) && two_words.test(128));
}

#[test]
fn tiny_bitset_whole_bitset_operations_ignore_bits_past_the_end() {
    // set_all()/clear_all() and the reference's mappers work word by word and
    // mask the bits past size (src/cata_bitset.h:214-233, 257-303).
    for size in [0_usize, 1, 63, 64, 65, 127, 128, 129] {
        let mut bits = TinyBitset::with_size(size);
        bits.set_all();
        assert!(bits.all(), "size {size} all after set_all");
        assert_eq!(bits.any(), size > 0);
        assert_eq!(bits.none(), size == 0);
        assert_eq!(bits.count(), size);
        for index in 0..size {
            assert!(bits.test(index), "size {size} bit {index}");
        }

        bits.clear_all();
        assert!(bits.none(), "size {size} none after clear_all");
        assert!(!bits.any());
        assert_eq!(bits.all(), size == 0);
        assert_eq!(bits.count(), 0);
        // Re-setting every bit after clear_all works the same.
        for index in 0..size {
            assert!(bits.set(index));
        }
        assert!(bits.all());

        // Flipping one bit makes all() false, except when it is the only bit.
        bits.clear_all();
        if size > 0 {
            let middle = size / 2;
            assert!(bits.flip(middle));
            assert!(bits.test(middle));
            assert_eq!(bits.count(), 1);
            assert_eq!(bits.all(), size == 1);
            assert!(bits.flip(middle));
            assert!(bits.none());
        }
    }
}

#[test]
fn tiny_bitset_resize_keeps_bits_in_range_and_zeroes_new_ones() {
    // Reference: resize() stays inline up to 56 bits and then routes through
    // resize_heap() (src/cata_bitset.h:138-148, src/cata_bitset.cpp:14-46),
    // which copies the inline word across and leaves the added heap words
    // uninitialised. This port zeroes them, so growth is deterministic; that
    // is the one divergence this test pins.
    let mut bits = TinyBitset::with_size(56);
    assert!(bits.set(0) && bits.set(55));

    bits.resize(64); // inline -> heap, across the 56-bit inline limit
    assert_eq!(bits.size(), 64);
    assert_eq!(bits.capacity(), 64);
    assert!(bits.test(0) && bits.test(55));
    assert_eq!(bits.count(), 2);

    bits.resize(65);
    assert_eq!(bits.size(), 65);
    assert_eq!(bits.count(), 2);
    assert!(!bits.test(64)); // the new bit starts clear

    assert!(bits.set(64));
    assert_eq!(bits.count(), 3);
    bits.resize(128);
    assert_eq!(bits.size(), 128);
    assert!(bits.test(0) && bits.test(55) && bits.test(64));
    assert_eq!(bits.count(), 3);

    // Shrinking drops the bits that fall outside, and growing again cannot
    // resurrect them.
    bits.resize(56);
    assert_eq!(bits.size(), 56);
    assert_eq!(bits.count(), 2);
    bits.resize(128);
    assert_eq!(bits.count(), 2);
    assert!(!bits.test(64));

    // Resizing to zero empties the range, which stays vacuously all-set.
    bits.resize(0);
    assert_eq!(bits.size(), 0);
    assert_eq!(bits.capacity(), 0);
    assert!(bits.all());
    assert!(bits.none());
}

#[test]
fn colony_clone_copies_the_elements_but_not_handle_identity() {
    // The reference's copy constructor allocates new storage for the elements
    // (src/colony.h:668-687), so pointers and iterators into the source never
    // refer to the copy's elements.
    let mut original: Colony<i32> = Colony::new();
    let handles: Vec<Handle> = (0..20).map(|i| original.insert(i)).collect();

    let copy = original.clone();
    assert_eq!(copy.size(), 20);
    assert_eq!(
        copy.iter().copied().collect::<Vec<i32>>(),
        (0..20).collect::<Vec<i32>>()
    );
    assert_eq!(copy.capacity(), original.capacity());

    for handle in &handles {
        // A handle from the source still resolves in the source ...
        assert!(original.get(*handle).is_some());
        // ... but not in the copy, whose elements live in new groups.
        assert_eq!(copy.get(*handle), None);
    }
}

#[test]
fn tiny_bitset_from_bools_sets_the_matching_bits() {
    // The reference constructs a bitset from a size only; this constructor is
    // the standard bitset's bool-list form and is marked as an addition in the
    // module docs.
    let bits = TinyBitset::from_bools(&[true, false, true, true]);
    assert_eq!(bits.size(), 4);
    assert!(bits.test(0));
    assert!(!bits.test(1));
    assert!(bits.test(2) && bits.test(3));
    assert_eq!(bits.count(), 3);

    let empty = TinyBitset::from_bools(&[]);
    assert_eq!(empty.size(), 0);
    assert!(empty.all());
    assert!(empty.none());
}

#[test]
fn colony_reverse_iteration_matches_the_reference() {
    // Translated from "colony_basics" (tests/colony_test.cpp:117-139): reverse
    // iteration counts and sums the same 400 elements, and the reverse
    // iterators agree with forward ones on positions.
    let mut colony: Colony<i32> = Colony::new();
    for _ in 0..200 {
        colony.insert(10);
        colony.insert(20);
    }

    // Reverse iterator count / access.
    let mut count = 0;
    let mut sum = 0;
    let mut it = colony.last();
    while let Some(handle) = it {
        count += 1;
        sum += *colony.get(handle).expect("element");
        it = colony.previous(handle);
    }
    assert_eq!(count, 400);
    assert_eq!(sum, 6000);

    // "Reverse iterator advance and distance test": rbegin() + 50 is index 349.
    let begin = colony.begin().expect("begin");
    let from_back = colony
        .at_index(colony.size() - 1 - 50)
        .expect("50 from the back");
    assert_eq!(colony.distance(begin, from_back), Some(349));

    // Const-reverse iteration from the second element from the back down to the
    // front: 399 elements, and 20 less than the full sum because the last
    // inserted element (a 20) is the one skipped.
    let mut count = 0;
    let mut sum = 0;
    let mut it = colony.previous(colony.last().expect("last"));
    while let Some(handle) = it {
        count += 1;
        sum += *colony.get(handle).expect("element");
        it = colony.previous(handle);
    }
    assert_eq!(count, 399);
    assert_eq!(sum, 5980);
}
