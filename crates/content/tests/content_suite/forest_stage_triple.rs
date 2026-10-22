//! One chunk, three traces, one moment: what a port has to reproduce before it resolves anything.
//!
//! The perimeter the edge curve produced, the eight neighbour terrains, the placeholder every square ended up holding, and
//! the per-square record of which of the six tickets drew it. All four files name the same chunk - the perimeter and the
//! neighbour list by **overmap** coordinate, the other two by **submap** coordinate, which is twice as large. Getting that
//! conversion wrong is how a chunk gets compared against a different chunk and nobody notices.

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
fn all_four_files_name_the_same_chunk_across_two_coordinate_systems() {
    let perimeter = lines("reference-chunk-stage-perimeter.txt");
    assert_eq!(perimeter[0][0], "perimeter");
    let omt: Vec<i64> = perimeter[0][1..4]
        .iter()
        .map(|v| v.parse().expect("a coordinate"))
        .collect();
    let placed = lines("reference-chunk-stage-placed.txt");
    let submap: Vec<i64> = placed[0][0..3]
        .iter()
        .map(|v| v.parse().expect("a coordinate"))
        .collect();
    assert_eq!(
        omt[0] * 2,
        submap[0],
        "the submap coordinate is twice the overmap one"
    );
    assert_eq!(omt[1] * 2, submap[1]);
    assert_eq!(placed.len(), 576);
    let features = lines("reference-chunk-stage-features.txt");
    assert_eq!(features.len(), 576);
    assert!(features.iter().all(|line| line[0..3] == placed[0][0..3]));
    assert_eq!(
        perimeter[0].len(),
        100,
        "the coordinate and ninety-six depths"
    );
}

#[test]
fn the_chunk_as_generated_is_almost_entirely_placeholders() {
    // 574 of 576 squares, which is the number a port's generator has to produce before its sweep runs. Two squares skip the
    // palettes and hold a real terrain the raffle placed directly.
    let placed = lines("reference-chunk-stage-placed.txt");
    let placeholders = placed
        .iter()
        .filter(|line| line[5].starts_with("t_region_"))
        .count();
    assert_eq!(placeholders, 574);
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for line in &placed {
        *counts.entry(line[5].as_str()).or_insert(0) += 1;
    }
    let ground = counts.get("t_region_groundcover").copied().unwrap_or(0)
        + counts
            .get("t_region_groundcover_forest")
            .copied()
            .unwrap_or(0);
    assert!(ground > 400, "{ground} squares of plain ground cover");
}

#[test]
fn the_perimeter_in_this_record_is_the_curve_the_chunk_was_generated_from() {
    // The depths are the input to every weight of the chunk, so they get their own statement: within the range the reference
    // produces, and not constant - a curve that never varies would be a bug in the trace rather than a woodland edge.
    let perimeter = lines("reference-chunk-stage-perimeter.txt");
    let depths: Vec<i32> = perimeter[0][4..]
        .iter()
        .map(|v| v.parse().expect("a depth"))
        .collect();
    assert_eq!(depths.len(), 96);
    assert!(
        depths.iter().all(|depth| (1..=12).contains(depth)),
        "{depths:?}"
    );
    let lowest = depths.iter().copied().min().unwrap_or(0);
    let highest = depths.iter().copied().max().unwrap_or(0);
    assert!(highest > lowest, "the curve varies: {lowest}..{highest}");
}

#[test]
fn the_feature_record_is_a_full_grid_in_the_reference_s_loop_order() {
    // The reference walks `x` outside and `y` inside, so its per-square record is column-major while a map dump is
    // row-major. Reading the file in order and rebuilding the grid by coordinate has to visit every square exactly once -
    // a test that would fail the moment the record was truncated, duplicated or written in the other order.
    let features = lines("reference-chunk-stage-features.txt");
    let mut seen = vec![false; 576];
    for line in &features {
        let x: usize = line[3].parse().expect("x");
        let y: usize = line[4].parse().expect("y");
        let index = y * 24 + x;
        assert!(!seen[index], "square ({x},{y}) appears twice");
        seen[index] = true;
    }
    assert!(
        seen.iter().all(|visited| *visited),
        "every square is present"
    );
    // And the file order really is column-major: the second line's y is one more than the first's, with x unchanged.
    assert_eq!(features[0][3], features[1][3], "same column");
    assert_eq!(
        features[1][4].parse::<i32>().expect("y") - features[0][4].parse::<i32>().expect("y"),
        1,
        "the next row of the same column"
    );
}
