//! A chunk's recorded inputs: the perimeter the reference computed, and the tickets it drew from it.
//!
//! A fresh world was generated with all three traces on, so these files describe **one** chunk end to end. They are the
//! inputs the port needs in order to be checked against the reference given identical inputs, rather than being scored
//! on two independent draws happening to agree.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

fn lines(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap")
        .join(name);
    std::fs::read_to_string(&path).expect("the trace")
}

#[test]
fn the_perimeter_is_ninety_six_depths() {
    let text = lines("reference-chunk-perimeter.txt");
    let fields: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(fields[0], "perimeter");
    assert_eq!(fields.len(), 97, "the word and ninety-six depths");
    let depths: Vec<i32> = fields[1..]
        .iter()
        .map(|field| field.parse().expect("a depth"))
        .collect();
    assert_eq!(depths.len(), 96, "four edges of twenty-four");
    // Measured values: between four and ten squares, which is the average of five with a deviation of one, wandering.
    let lowest = depths.iter().copied().min().unwrap_or(0);
    let highest = depths.iter().copied().max().unwrap_or(0);
    assert!(depths.iter().all(|depth| (4..=10).contains(depth)), "{lowest}..{highest}");
    assert_eq!(
        &depths[..4],
        &[9, 6, 5, 5],
        "the north edge starts at nine and settles at five"
    );
}

#[test]
fn the_chunk_drew_a_feature_ticket_for_every_square() {
    let text = lines("reference-chunk-tickets.txt");
    let count = text.lines().filter(|line| !line.trim().is_empty()).count();
    assert_eq!(count, 576, "one per square");
    let tickets: Vec<i32> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .nth(2)
                .expect("a ticket")
                .parse()
                .expect("a number")
        })
        .collect();
    let mut distinct = tickets.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct, vec![-2, -1, 0, 1, 2, 3], "the same six choices");
}

#[test]
fn the_ground_raffle_ran_where_the_feature_raffle_left_nothing() {
    let feature = lines("reference-chunk-tickets.txt");
    let ground = lines("reference-chunk-ground-tickets.txt");
    let empties = feature
        .lines()
        .filter(|line| line.split_whitespace().nth(2) == Some("-2"))
        .count();
    let ground_calls = ground
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    // The ground raffle runs for the empty ticket **and** for a pick that lands on a neighbour owning no biome, so it is
    // called more often than the empty ticket appears: 573 against 461 measured. My first version expected the two to
    // agree and the numbers corrected it - 112 squares went to the ground raffle because their winning ticket was a
    // road or a field rather than because it was empty.
    assert_eq!(ground_calls, 573);
    assert_eq!(empties, 461);
    assert!(
        ground_calls > empties,
        "the neighbour route adds {diff} squares",
        diff = ground_calls - empties
    );
}

#[test]
fn the_fresh_world_dumped_a_different_chunk_from_the_one_it_traced() {
    // A finding, not a rule: the fresh run's dump is the chunk the player stands in - open field at `[192,333]` - while
    // the traced chunk is a forest the same run generated nearby. So the trace and the dump describe **different**
    // chunks, and comparing them directly would be comparing two unrelated squares. The fix is a dump that names its
    // own overmap coordinate instead of taking the player's.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/maps/reference-forest-chunk-240.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&path).expect("the chunk")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Str(oter)) = root.get("oter") else {
        panic!("an oter");
    };
    assert_eq!(oter, "field", "the dump is open ground");
    let Some(cdda_json::Value::Array(omt)) = root.get("omt") else {
        panic!("a position");
    };
    let axis = |index: usize| match &omt[index] {
        cdda_json::Value::Int(number) => *number,
        _ => panic!("a number"),
    };
    assert_eq!((axis(0), axis(1)), (192, 333), "at the player's own square");
}
