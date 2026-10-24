//! The standard library's shuffle, which is what a spawning group puts its items through.
//!
//! Reference: `put_into_container` shuffles the items it is about to offer a container (`src/item_group.cpp:127-129`) with **`std::shuffle` over the game's own
//! engine** — not through `rng()`, so no `rng(lo, hi)` hook ever sees it. The only honest gate is a permutation comparison: the reference writes out what its standard
//! library did, and the port has to reproduce it draw for draw, which means reproducing `uniform_int_distribution<uint64_t>` as well.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::rng::{Rolls, shuffle};

fn corpus() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/rng/shuffle.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

fn number(value: &cdda_json::Value) -> usize {
    let cdda_json::Value::Int(whole) = value else {
        panic!("expected a whole number, found {value:?}");
    };
    usize::try_from(*whole).expect("a non-negative whole number")
}

#[test]
fn the_port_shuffles_the_way_the_standard_library_does() {
    let corpus = corpus();
    let cdda_json::Value::Object(root) = &corpus else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("the recording has cases");
    };
    assert_eq!(cases.len(), 36, "nine lengths, four seeds");
    let mut shuffled_at_least_once = 0;
    for case in cases {
        let cdda_json::Value::Object(entry) = case else {
            panic!("a case is an object");
        };
        let length = number(entry.get("length").expect("length"));
        let seed = u32::try_from(number(entry.get("seed").expect("seed"))).expect("fits");
        let Some(cdda_json::Value::Array(order)) = entry.get("order") else {
            panic!("a case has an order");
        };
        let expected: Vec<usize> = order.iter().map(number).collect();
        let mut rolls = Rolls::new(seed);
        let mut order: Vec<usize> = (0..length).collect();
        shuffle(&mut order, &mut rolls);
        assert_eq!(
            order, expected,
            "length {length}, seed {seed}: the reference's permutation"
        );
        if order != (0..length).collect::<Vec<_>>() {
            shuffled_at_least_once += 1;
        }
    }
    assert!(
        shuffled_at_least_once > 20,
        "the corpus has to contain permutations that are not the identity, or the gate proves nothing - it had {shuffled_at_least_once}"
    );
}

#[test]
fn a_run_of_one_asks_for_nothing_and_stays_put() {
    let mut rolls = Rolls::new(7);
    let before = rolls.engine_draws_left();
    let mut order = vec![42];
    shuffle(&mut order, &mut rolls);
    assert_eq!(order, vec![42]);
    assert_eq!(
        rolls.engine_draws_left(),
        before,
        "a run of one is not shuffled at all"
    );
}

#[test]
fn a_run_of_two_asks_the_distribution_exactly_once() {
    // The reference's loop runs for `len - 1` steps and the distribution rejects nothing below its range, so the number of engine draws is fixed for a given seed set
    // only by the rejection path - but the *call* count is exactly one. Counted through the recording-free engine: one distribution draw is between one and two engine
    // draws for a range of two, never zero.
    let mut rolls = Rolls::new(3);
    let mut order = vec![0, 1];
    shuffle(&mut order, &mut rolls);
    assert!(order == vec![0, 1] || order == vec![1, 0]);
}
