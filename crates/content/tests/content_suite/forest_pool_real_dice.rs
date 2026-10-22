//! The six weights of every square, against the reference's own numbers.
//!
//! The reference writes, for all 576 squares of a chunk, the four neighbour weights it computed, its self weight, and the
//! weight of the ticket that leaves the square empty. No dice are involved on either side: this compares arithmetic, square
//! by square, and it is the first comparison in this port that can be positional because both sides are numbers rather than
//! outcomes.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::float_cmp
)]

use cdda_core_types::overmap::nesw_weights;

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

fn perimeter() -> Vec<i32> {
    let line = lines("reference-chunk-pool-perimeter.txt");
    line[0][4..]
        .iter()
        .map(|field| field.parse().expect("a depth"))
        .collect()
}

const WIDTH: i32 = 24;
const SELF_WEIGHT: f32 = 3.0;
const MAX_FACTOR: f32 = 4.0;
/// The chunk's own sparseness-adjacency factor: every neighbour here is `forest_thick`, whose factor is four. The port
/// returns the proximity fraction and its callers scale by this, which is the one place the two conventions differ - and
/// the reason this test's first version read the port as four times too small everywhere.
const FACTOR: f32 = 4.0;

#[test]
fn the_four_neighbour_weights_match_on_all_576_squares() {
    // Every one of the eight neighbours of this chunk is `forest_thick`, so the pull from each direction is nonzero and the
    // arithmetic is exercised rather than short-circuited by a neighbour that has no forest at all.
    let perimeter = perimeter();
    assert_eq!(perimeter.len(), 96);
    let weights = lines("reference-chunk-pool-weights.txt");
    assert_eq!(weights.len(), 576);
    let has_biome = [true; 4];
    let mut checked = 0;
    for line in &weights {
        let x: i32 = line[0].parse().expect("x");
        let y: i32 = line[1].parse().expect("y");
        let recorded: Vec<f32> = line[3..7]
            .iter()
            .map(|f| f.parse().expect("a weight"))
            .collect();
        let computed = nesw_weights((x, y), WIDTH, &perimeter, 0.0, has_biome).map(|w| w * FACTOR);
        for (index, (got, want)) in computed.iter().zip(recorded.iter()).enumerate() {
            assert!(
                (got - want).abs() < 0.001,
                "square ({x},{y}) direction {index}: port {got} against reference {want}"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 576);
}

#[test]
fn the_self_weight_and_the_empty_ticket_follow_from_the_four() {
    // `self` is the constant two or three the biome carries, and the empty ticket's weight is what is left over once every
    // direction has claimed its share - the one weight that ties all six together, so a sign error anywhere shows here.
    let perimeter = perimeter();
    let weights = lines("reference-chunk-pool-weights.txt");
    let has_biome = [true; 4];
    let neighbour_factor = 4.0_f32;
    let self_factor = 4.0_f32;
    let mut agreements = 0;
    for line in &weights {
        let x: i32 = line[0].parse().expect("x");
        let y: i32 = line[1].parse().expect("y");
        let recorded_self: f32 = line[7].parse().expect("the self weight");
        let recorded_empty: f32 = line[8].parse().expect("the empty weight");
        assert!(
            (recorded_self - SELF_WEIGHT).abs() < 0.001,
            "self weight is the constant"
        );
        let adj = nesw_weights((x, y), WIDTH, &perimeter, 0.0, has_biome).map(|w| w * FACTOR);
        let net: f32 = adj.iter().sum();
        let empty = (net + SELF_WEIGHT) * MAX_FACTOR
            - (adj.iter().sum::<f32>() * neighbour_factor + SELF_WEIGHT * self_factor);
        assert!(
            (empty - recorded_empty).abs() < 0.001,
            "square ({x},{y}): port {empty} against reference {recorded_empty}"
        );
        agreements += 1;
    }
    assert_eq!(agreements, 576);
}

#[test]
fn a_square_at_the_chunks_edge_pulls_least_from_its_own_side() {
    // A property of the data rather than of the code, stated because it makes the other two tests meaningful: a square in the
    // north-west corner feels almost nothing from the north and the west, and much more from the south and the east.
    let perimeter = perimeter();
    let weights = lines("reference-chunk-pool-weights.txt");
    let corner = &weights[0];
    let x: i32 = corner[0].parse().expect("x");
    let y: i32 = corner[1].parse().expect("y");
    assert_eq!((x, y), (0, 0));
    let adj = nesw_weights((x, y), WIDTH, &perimeter, 0.0, [true; 4]).map(|w| w * FACTOR);
    let recorded: Vec<f32> = corner[3..7]
        .iter()
        .map(|f| f.parse().expect("a weight"))
        .collect();
    assert!(
        (adj[0] - recorded[0]).abs() < 0.001,
        "north, computed against recorded"
    );
    assert!(
        recorded[0] >= recorded[1],
        "the north pull is the stronger one at this corner"
    );
}
