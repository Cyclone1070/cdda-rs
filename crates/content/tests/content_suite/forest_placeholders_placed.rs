//! What the generator leaves behind: placeholders, verbatim, on every square.
//!
//! Two traces of the same run, one written by the per-square raffle and one by reading the chunk back from the very map
//! object the placement used. They agree on **576 of 576** squares: the terrain a forest chunk holds when generation
//! finishes is the placeholder the raffle chose, unreplaced. Resolution is somebody else's job - and that is why comparing
//! the raffle against a finished map's dump disagreed on half the squares for three rounds.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

use std::collections::BTreeMap;

fn lines(name: &str) -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap")
        .join(name);
    std::fs::read_to_string(&path)
        .expect("a trace")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.split_whitespace().map(str::to_owned).collect())
        .collect()
}

#[test]
fn the_terrain_the_generator_leaves_is_the_placeholder_it_chose() {
    // The whole finding in one comparison, square by square, between two files written by the same run. Not a sample and
    // not a tolerance: 576 squares, 576 identical strings.
    let features = lines("reference-chunk-placed-features.txt");
    assert_eq!(features.len(), 576);
    let placed = lines("reference-chunk-placed.txt");
    assert_eq!(placed.len(), 576);

    let mut terrain = BTreeMap::new();
    for line in &placed {
        let x: i32 = line[3].parse().expect("x");
        let y: i32 = line[4].parse().expect("y");
        terrain.insert((x, y), line[5].clone());
    }
    let mut same = 0;
    for line in &features {
        let x: i32 = line[3].parse().expect("x");
        let y: i32 = line[4].parse().expect("y");
        if terrain.get(&(x, y)) == Some(&line[6]) {
            same += 1;
        }
    }
    assert_eq!(
        same, 576,
        "{same} of 576 squares hold the placeholder they drew"
    );
}

#[test]
fn the_placeholders_are_still_placeholders_when_generation_finishes() {
    // Named rather than assumed: an unreplaced chunk is full of names starting `t_region_`, which is exactly what a port
    // must produce before its resolution pass and must not produce after it.
    let placed = lines("reference-chunk-placed.txt");
    let placeholders = placed
        .iter()
        .filter(|line| line[5].starts_with("t_region_"))
        .count();
    assert!(
        placeholders > 400,
        "{placeholders} of 576 squares hold a placeholder"
    );
    assert!(
        placeholders < 576,
        "and some squares are real terrain already"
    );
}

#[test]
fn the_two_traces_name_the_same_chunk() {
    // Without this the comparison above would be comparing two files about different chunks and passing by accident when
    // both happen to be the same woodland - which is precisely the mistake that cost three rounds.
    let features = lines("reference-chunk-placed-features.txt");
    let placed = lines("reference-chunk-placed.txt");
    let feature_chunk: Vec<String> = features[0][0..3].to_vec();
    assert!(features.iter().all(|line| line[0..3] == feature_chunk[..]));
    assert!(placed.iter().all(|line| line[0..3] == feature_chunk[..]));
    assert_eq!(feature_chunk, vec!["600", "696", "0"]);
}
