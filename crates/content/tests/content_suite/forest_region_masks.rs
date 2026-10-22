//! Thirty-five chunks of one woodland, and which of their sides own a biome.
//!
//! The neighbour weights of every square of a chunk come from that chunk's perimeter and from **which of its four sides
//! owns a biome at all** - four yes-or-nos that are the same for the whole chunk. The trace holds the weights the
//! reference actually used, so the mask can be read back out of it: of the sixteen possibilities, only one reproduces all
//! 576 weights of a chunk, and this gate derives that mask for every chunk in the region and checks the derivation is
//! unambiguous.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_possible_truncation
)]

use cdda_core_types::overmap::nesw_weights;

const SIDE: i32 = 24;
const TOLERANCE: f32 = 3e-3;
/// The density of the biome the region's chunks were generating: dense woodland.
const FACTOR: f32 = 4.0;

fn region() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-region-trace.txt");
    std::fs::read_to_string(&path).expect("the region trace")
}

fn perimeters() -> Vec<Vec<i32>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-region-perimeter.txt");
    std::fs::read_to_string(&path)
        .expect("the region perimeters")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .skip(4)
                .map(|field| field.parse().expect("a depth"))
                .collect()
        })
        .collect()
}

/// One square's recorded neighbour weights, and where it sits.
struct Square {
    point: (i32, i32),
    weights: [f32; 4],
}

/// The mask, if any, that reproduces every weight of this chunk.
fn fitting_masks(squares: &[Square], perimeter: &[i32]) -> Vec<[bool; 4]> {
    let mut fits = Vec::new();
    for mask in 0_u8..16 {
        let owns = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
        let all = squares.iter().all(|square| {
            let predicted =
                nesw_weights(square.point, SIDE, perimeter, 0.0, owns).map(|w| w * FACTOR);
            (0..4).all(|direction| {
                (predicted[direction] - square.weights[direction]).abs() < TOLERANCE
            })
        });
        if all {
            fits.push(owns);
        }
    }
    fits
}

fn squares(text: &str) -> Vec<Square> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<f32> = line
                .split_whitespace()
                .map(|field| field.parse().expect("a number"))
                .collect();
            let mut weights = [0.0_f32; 4];
            weights.copy_from_slice(&fields[3..7]);
            Square {
                point: (fields[0] as i32, fields[1] as i32),
                weights,
            }
        })
        .collect()
}

/// The mask of every chunk in the region, in the order the perimeters are written.
fn region_masks() -> Vec<[bool; 4]> {
    let text = region();
    let perimeters = perimeters();
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        perimeters.len() * 576,
        "one chunk per perimeter"
    );
    perimeters
        .iter()
        .enumerate()
        .map(|(index, perimeter)| {
            let chunk = squares(&lines[index * 576..(index + 1) * 576].join("\n"));
            let fits = fitting_masks(&chunk, perimeter);
            assert_eq!(
                fits.len(),
                1,
                "chunk {index} has {} explanations",
                fits.len()
            );
            fits[0]
        })
        .collect()
}

#[test]
fn every_chunk_of_the_region_has_exactly_one_mask_that_explains_it() {
    let masks = region_masks();
    assert_eq!(masks.len(), 35, "thirty-five chunks");
}

#[test]
fn the_masks_differ_across_the_region_rather_than_being_one_guess() {
    // A test that always produced the same mask would pass while proving nothing: it would be consistent with the port's
    // own assumptions rather than with the reference. Nine distinct masks appear across the thirty-five chunks, and the
    // most common one - all four sides wooded - covers only half of them.
    let masks = region_masks();
    let mut distinct = masks.clone();
    distinct.sort_by_key(|mask| [mask[0], mask[1], mask[2], mask[3]]);
    distinct.dedup();
    assert_eq!(distinct.len(), 9, "nine distinct masks");
    let all_wooded = masks
        .iter()
        .filter(|mask| mask.iter().all(|side| *side))
        .count();
    assert_eq!(all_wooded, 17, "seventeen chunks wooded on all four sides");
    // My first version also expected a chunk with **no** woodland beside it at all. The data says otherwise: every
    // chunk in this region touches woodland on at least one side, which is what a chunk generated inside a forest
    // should show, and the expectation was the wrong one.
    assert!(
        masks.iter().all(|mask| mask.iter().any(|side| *side)),
        "every chunk touches woodland somewhere"
    );
}
