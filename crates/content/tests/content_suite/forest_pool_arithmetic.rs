//! The empty ticket's weight, recomputed from the recorded five, across a whole chunk.
//!
//! The trace records the four neighbour weights, this square's own weight and the empty ticket's weight for every square.
//! The empty one is not free: the reference computes it as the total **scaled up by the maximum density of any biome in
//! the region** minus what the neighbours and this square contribute at their own densities.
//!
//! Those densities belong to the chunk's four neighbours, so they are the same for every square of the chunk. That turns
//! 576 recorded squares into 576 equations in four unknowns - which is the strongest check available: if the port's
//! arithmetic matches the reference's, exactly one choice of densities satisfies all of them at once.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::float_cmp,
    clippy::cast_possible_truncation
)]

use cdda_core_types::overmap::{feather_pool, nesw_weights};

const MAX_FACTOR: f32 = 4.0;
const SELF_FACTOR: f32 = 4.0;
const SELF_WEIGHT: f32 = 3.0;
const TOLERANCE: f32 = 1e-3;

fn squares() -> Vec<([f32; 4], f32, f32)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-pools-trace.txt");
    let text = std::fs::read_to_string(&path).expect("the trace");
    let mut out = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<f32> = line
            .split_whitespace()
            .map(|field| field.parse().expect("a number"))
            .collect();
        let mut neighbours = [0.0_f32; 4];
        neighbours.copy_from_slice(&fields[3..7]);
        out.push((neighbours, fields[7], fields[8]));
    }
    out
}

/// The densities a forest biome contributes: woods, dense woods, and wet woods.
const DENSITIES: [f32; 4] = [0.0, 2.0, 3.0, 4.0];

#[test]
fn one_set_of_neighbour_densities_explains_every_square_of_the_chunk() {
    let squares = squares();
    assert_eq!(squares.len(), 576);
    let mut fits = Vec::new();
    for north in DENSITIES {
        for east in DENSITIES {
            for south in DENSITIES {
                for west in DENSITIES {
                    let factors = [north, east, south, west];
                    let all = squares.iter().all(|(neighbours, self_weight, empty)| {
                        let net: f32 = neighbours.iter().sum();
                        let pool = feather_pool(
                            *neighbours,
                            *self_weight,
                            net,
                            MAX_FACTOR,
                            SELF_FACTOR,
                            factors,
                        );
                        (pool[5] - empty).abs() < TOLERANCE
                    });
                    if all {
                        fits.push(factors);
                    }
                }
            }
        }
    }
    assert_eq!(
        fits.len(),
        1,
        "expected exactly one set of densities to explain the chunk, got {fits:?}"
    );
    // The one that fits: woodland to the north, east and south, and **nothing** to the west - the neighbour that owns no
    // biome at all, which is also why its density in this tuple is zero. A chunk of dense woods fenced on three sides.
    assert_eq!(fits[0], [4.0, 4.0, 4.0, 0.0]);
}

#[test]
fn the_neighbour_weights_follow_from_the_perimeter_and_which_sides_own_a_biome() {
    // The second half of the chain: the four neighbour weights before the corner unification blends them. `nesw_weights`
    // takes the recorded perimeter, this square's distance from each edge, and **which** sides own a biome - the last of
    // which is unknown, so it is inferred here from 576 recorded squares. Exactly one of the sixteen masks explains them.
    let perimeter: Vec<i32> = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/overmap/reference-forest-pools-perimeter.txt"),
    )
    .expect("the perimeter")
    .split_whitespace()
    .skip(4)
    .map(|field| field.parse().expect("a depth"))
    .collect();
    assert_eq!(perimeter.len(), 96);

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-pools-trace.txt");
    let text = std::fs::read_to_string(&path).expect("the trace");
    let mut fits = Vec::new();
    for mask in 0_u8..16 {
        let owns = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
        let mut all = true;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let fields: Vec<f32> = line
                .split_whitespace()
                .map(|field| field.parse().expect("a number"))
                .collect();
            let point = (fields[0] as i32, fields[1] as i32);
            let predicted =
                nesw_weights(point, 24, &perimeter, 0.0, owns).map(|weight| weight * 4.0);
            for direction in 0..4 {
                if (predicted[direction] - fields[3 + direction]).abs() > 3e-3 {
                    all = false;
                    break;
                }
            }
            if !all {
                break;
            }
        }
        if all {
            fits.push(owns);
        }
    }
    assert_eq!(fits.len(), 1, "expected one mask, got {fits:?}");
    assert_eq!(fits[0], [true, true, true, false], "no biome to the west");
}

#[test]
fn the_self_weight_is_the_reference_s_own_scalar_throughout() {
    for (_, self_weight, _) in squares() {
        assert!(
            (self_weight - SELF_WEIGHT).abs() < f32::EPSILON,
            "this square's own weight is {self_weight}, not {SELF_WEIGHT}"
        );
    }
}

#[test]
fn the_recorded_empty_ticket_is_never_the_whole_pool() {
    // A square whose empty ticket carried everything would mean the neighbours contributed nothing to the subtraction,
    // which cannot happen while every neighbour weight is non-negative and the total is scaled by four.
    for (neighbours, self_weight, empty) in squares() {
        let total: f32 = neighbours.iter().sum::<f32>() + self_weight;
        assert!(empty < total * MAX_FACTOR, "{empty} against {total}");
    }
}
