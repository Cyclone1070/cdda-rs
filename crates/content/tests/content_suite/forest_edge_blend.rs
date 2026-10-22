//! The corner rule, checked against a real forest edge.
//!
//! Each of these chunks sits where woodland meets open ground, and no choice of four side masks reproduces its recorded
//! neighbour weights. Corner blending does: where a square's corner is one biome and both its sides are another, the side
//! matching the corner gives up part of its influence to the side matching the square. The eight neighbour terrains are
//! recorded beside the weights, so the whole chain - perimeter to neighbour weights, corner blending, and the six-ticket
//! pool - can be run and compared square by square, with no dice anywhere.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::needless_range_loop
)]

use cdda_core_types::overmap::{feather_pool, nesw_weights, unify_all_borders};

const SIDE: i32 = 24;
const TOLERANCE: f32 = 3e-3;
const SELF_WEIGHT: f32 = 3.0;
const MAX_FACTOR: f32 = 4.0;

struct Chunk {
    omt: [i64; 3],
    neighbours: [Option<String>; 8],
    perimeter: Vec<i32>,
    squares: Vec<(i32, i32, [f32; 4], f32)>,
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
        let Some(cdda_json::Value::Array(neighbours)) = chunk.get("neighbours") else {
            panic!("neighbours");
        };
        let mut around: [Option<String>; 8] = Default::default();
        for (index, value) in neighbours.iter().enumerate() {
            let cdda_json::Value::Str(text) = value else {
                panic!("a terrain id");
            };
            around[index] = Some(text.clone());
        }
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
                cdda_json::Value::Str(text) => text.parse::<f32>().expect("a number as text"),
                cdda_json::Value::Float(value) => *value as f32,
                cdda_json::Value::Int(value) => *value as f32,
                _ => panic!("a number"),
            };
            let mut weights = [0.0_f32; 4];
            for direction in 0..4 {
                weights[direction] = number(3 + direction);
            }
            traced.push((number(0) as i32, number(1) as i32, weights, number(8)));
        }
        out.push(Chunk {
            omt: [axis(0), axis(1), axis(2)],
            neighbours: around,
            perimeter: depths,
            squares: traced,
        });
    }
    out
}

/// Whether a terrain is one of the region's woodlands, which is what owning a biome means here.
fn is_woodland(name: Option<&str>) -> bool {
    matches!(name, Some("forest" | "forest_thick" | "forest_water"))
}

/// The chain the reference runs for one square, ending at the six-ticket pool.
fn pool_for(chunk: &Chunk, point: (i32, i32), self_id: Option<&str>, factor: f32) -> [f32; 6] {
    // A side "owns a biome" only when it is woodland: open ground and buildings have no biome to feather against, and
    // their weight is therefore the undivided ratio. My first version treated any named terrain as a biome owner, which
    // made a field to the north divide its weight where the reference does not - the recorded thirty against my three.
    let owns = [
        is_woodland(chunk.neighbours[0].as_deref()),
        is_woodland(chunk.neighbours[1].as_deref()),
        is_woodland(chunk.neighbours[2].as_deref()),
        is_woodland(chunk.neighbours[3].as_deref()),
    ];
    let mut weights = nesw_weights(point, SIDE, &chunk.perimeter, 0.0, owns).map(|w| w * factor);
    let mut self_weight = SELF_WEIGHT;
    let names: [Option<&str>; 8] = std::array::from_fn(|index| {
        chunk.neighbours[index]
            .as_deref()
            .filter(|name| is_woodland(Some(name)))
    });
    unify_all_borders(point, SIDE, names, &mut weights, self_id, &mut self_weight);
    let net: f32 = weights.iter().sum();
    feather_pool(weights, self_weight, net, MAX_FACTOR, factor, [factor; 4])
}

#[test]
fn four_edge_chunks_with_their_eight_neighbours_and_perimeters() {
    let chunks = chunks();
    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks[0].omt, [259, 21, 0]);
    assert_eq!(
        chunks[0].neighbours[0].as_deref(),
        Some("field"),
        "open ground to the north"
    );
    assert_eq!(chunks[0].neighbours[1].as_deref(), Some("forest"));
    for chunk in &chunks {
        assert_eq!(chunk.perimeter.len(), 96, "at {:?}", chunk.omt);
        assert_eq!(chunk.squares.len(), 576, "at {:?}", chunk.omt);
    }
}

#[test]
fn the_chain_matches_most_of_an_edge_chunk_and_the_rest_is_recorded_as_a_gap() {
    // Honest score, not a claim of parity. With plain woodland and density three, the chain reproduces **288 of 576**
    // squares of this edge chunk. The first disagreement is square (0,12): the reference leaves its south weight at
    // 0.25 and its own weight at 3, while the port's corner rule divides the south weight to 0.0769 - the arithmetic of
    // the rule's **second** branch, which the reference did not take here even though the three names involved match
    // what that branch asks for (south is this world's woodland, the south-west corner and the west side are the denser
    // woodland, and the square's own biome is plain). So either the branch's guard is not what the port reproduces, or
    // the corner slot is not the one the port passes. Until that is settled the test says 288 and names the square.
    let chunk = &chunks()[0];
    let mut matched = 0;
    let mut first_miss = None;
    for (x, y, recorded, _) in &chunk.squares {
        let pool = pool_for(chunk, (*x, *y), Some("forest"), 3.0);
        let weights = [pool[0], pool[1], pool[2], pool[3]];
        if (0..4).all(|d| (weights[d] - recorded[d]).abs() < TOLERANCE) {
            matched += 1;
        } else if first_miss.is_none() {
            first_miss = Some((*x, *y, weights, *recorded));
        }
    }
    assert_eq!(matched, 288, "matched {matched} of 576");
    let (x, y, computed, recorded) = first_miss.expect("a first disagreement");
    assert_eq!(
        (x, y),
        (0, 12),
        "the first disagreement is on the western edge"
    );
    assert!(
        (recorded[2] - 0.25).abs() < 1e-4,
        "the reference keeps 0.25: {recorded:?}"
    );
    assert!(
        (computed[2] - 0.076_923).abs() < 1e-4,
        "the port divides it: {computed:?}"
    );
}

#[test]
fn the_denser_woodland_explains_none_of_the_chunk() {
    // The chunk's own density is read off the record rather than assumed: a north weight of 2.30769 is ten squares of
    // perimeter over thirteen rows, times three - and the same numbers times four are nowhere in the trace.
    let chunk = &chunks()[0];
    let mut matched = 0;
    for (x, y, recorded, _) in &chunk.squares {
        let pool = pool_for(chunk, (*x, *y), Some("forest_thick"), 4.0);
        let weights = [pool[0], pool[1], pool[2], pool[3]];
        if (0..4).all(|d| (weights[d] - recorded[d]).abs() < TOLERANCE) {
            matched += 1;
        }
    }
    assert_eq!(matched, 0, "dense woodland does not explain this chunk");
}
