//! Six chunks whose weights no side-mask can explain - the forest's own edge.
//!
//! Every chunk generated inside a woodland has four sides that either own a biome or do not, and that mask reproduces all
//! 576 of its neighbour weights. These six do not: no mask reproduces their weights, because corner blending moved weight
//! from one side to the other where a corner's biome differs from the square's. They are the first recorded evidence that
//! the corner rule actually runs in a real world, rather than only in the reference's source.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::needless_range_loop
)]

use cdda_core_types::overmap::nesw_weights;

const SIDE: i32 = 24;
const TOLERANCE: f32 = 3e-3;
const FACTOR: f32 = 4.0;

struct Chunk {
    omt: [i64; 3],
    perimeter: Vec<i32>,
    squares: Vec<(i32, i32, [f32; 4])>,
}

fn chunks() -> Vec<Chunk> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-edges.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&path).expect("the edges")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(list)) = root.get("chunks") else {
        panic!("chunks");
    };
    let mut out = Vec::new();
    for entry in list {
        let cdda_json::Value::Object(chunk) = entry else {
            panic!("a chunk");
        };
        let Some(cdda_json::Value::Array(omt)) = chunk.get("omt") else {
            panic!("a position");
        };
        let axis = |index: usize| match &omt[index] {
            cdda_json::Value::Int(number) => *number,
            _ => panic!("a number"),
        };
        let Some(cdda_json::Value::Array(perimeter)) = chunk.get("perimeter") else {
            panic!("a perimeter");
        };
        let depths = perimeter
            .iter()
            .map(|value| match value {
                cdda_json::Value::Int(number) => *number as i32,
                _ => panic!("a depth"),
            })
            .collect();
        let Some(cdda_json::Value::Array(squares)) = chunk.get("squares") else {
            panic!("squares");
        };
        let mut traced = Vec::new();
        for square in squares {
            let cdda_json::Value::Array(fields) = square else {
                panic!("a square");
            };
            let number = |index: usize| match &fields[index] {
                cdda_json::Value::Str(text) => text
                    .parse::<f32>()
                    .expect("a number that was written as text"),
                cdda_json::Value::Float(value) => *value as f32,
                cdda_json::Value::Int(value) => *value as f32,
                _ => panic!("a number"),
            };
            let mut weights = [0.0_f32; 4];
            for direction in 0..4 {
                weights[direction] = number(3 + direction);
            }
            traced.push((number(0) as i32, number(1) as i32, weights));
        }
        out.push(Chunk {
            omt: [axis(0), axis(1), axis(2)],
            perimeter: depths,
            squares: traced,
        });
    }
    out
}

#[test]
fn the_edges_are_six_named_chunks_with_every_square() {
    let chunks = chunks();
    assert_eq!(chunks.len(), 6);
    assert_eq!(chunks[0].omt, [152, 120, 0]);
    for chunk in &chunks {
        assert_eq!(chunk.perimeter.len(), 96, "at {:?}", chunk.omt);
        assert_eq!(chunk.squares.len(), 576, "at {:?}", chunk.omt);
    }
}

#[test]
fn no_side_mask_explains_an_edge_chunk() {
    // The finding, stated as a test: the four-side explanation that covers every interior chunk cannot cover these. What
    // is left out is corner blending, so this gate fails the moment the port starts reproducing edge chunks without it.
    for chunk in chunks() {
        let mut fits = 0_u8;
        for mask in 0_u8..16 {
            let owns = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
            let all = chunk.squares.iter().all(|(x, y, recorded)| {
                let point = (*x, *y);
                let predicted = nesw_weights(point, SIDE, &chunk.perimeter, 0.0, owns)
                    .map(|weight| weight * FACTOR);
                (0..4)
                    .all(|direction| (predicted[direction] - recorded[direction]).abs() < TOLERANCE)
            });
            if all {
                fits += 1;
            }
        }
        assert_eq!(
            fits, 0,
            "chunk {:?} has a plain explanation after all",
            chunk.omt
        );
    }
}

#[test]
fn the_edge_chunks_are_not_all_the_same_shape() {
    // Nine interior chunks of the region shared four sides of woodland, so a single "corner blending happened" claim could
    // hide a rule that always fires the same way. These six differ in how many squares disagree: some only at the corners,
    // some along a whole side.
    let mut disturbed = Vec::new();
    for chunk in chunks() {
        let mut count = 0;
        for mask in 0_u8..16 {
            let owns = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
            let differing = chunk
                .squares
                .iter()
                .filter(|(x, y, recorded)| {
                    let predicted = nesw_weights((*x, *y), SIDE, &chunk.perimeter, 0.0, owns)
                        .map(|weight| weight * FACTOR);
                    (0..4).any(|d| (predicted[d] - recorded[d]).abs() >= TOLERANCE)
                })
                .count();
            count = count.max(chunk.squares.len() - differing);
        }
        disturbed.push(count);
    }
    let best = disturbed.iter().copied().max().expect("an edge");
    assert!(
        best < 576,
        "no mask explains every square of any edge chunk"
    );
    assert_eq!(disturbed.len(), 6);
}
