//! The ported noise against the reference's own values.
//!
//! `corpus/overmap/reference-noise.json` holds 1,681 forest noise values dumped from the reference itself. There is
//! no judgement in this comparison: every point either matches to six decimals or the port's field is a different
//! field, and the failure names the worst point so the difference can be chased.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use cdda_core_types::noise::forest_noise;

struct Reference {
    radius: i32,
    centre: (i32, i32),
    seed: u32,
    rows: Vec<Vec<f32>>,
}

fn reference() -> Reference {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-noise.json");
    let bytes = std::fs::read(&path).expect("the reference noise dump");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump is an object");
    };
    let number = |key: &str| match root.get(key) {
        Some(cdda_json::Value::Int(n)) => *n as f64,
        Some(cdda_json::Value::Float(n)) => *n,
        _ => panic!("{key} is a number"),
    };
    let Some(cdda_json::Value::Array(centre)) = root.get("centre") else {
        panic!("a centre");
    };
    let axis = |index: usize| match &centre[index] {
        cdda_json::Value::Int(n) => *n as i32,
        cdda_json::Value::Float(n) => *n as i32,
        _ => panic!("a number"),
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    Reference {
        radius: number("radius") as i32,
        centre: (axis(0), axis(1)),
        seed: number("seed") as u32,
        rows: rows
            .iter()
            .map(|row| match row {
                cdda_json::Value::Array(cells) => cells
                    .iter()
                    .map(|cell| match cell {
                        cdda_json::Value::Float(n) => *n as f32,
                        cdda_json::Value::Int(n) => *n as f32,
                        _ => panic!("a number"),
                    })
                    .collect(),
                _ => panic!("a row"),
            })
            .collect(),
    }
}

#[test]
fn the_port_and_the_reference_produce_the_same_field() {
    let reference = reference();
    let mut worst = (0.0_f32, 0, 0);
    for (dy, row) in reference.rows.iter().enumerate() {
        for (dx, expected) in row.iter().enumerate() {
            let x = reference.centre.0 + dx as i32 - reference.radius;
            let y = reference.centre.1 + dy as i32 - reference.radius;
            let ours = forest_noise(x, y, reference.seed);
            let difference = (ours - expected).abs();
            if difference > worst.0 {
                worst = (difference, x, y);
            }
        }
    }
    assert!(
        worst.0 < 1e-6,
        "the fields differ by {} at ({}, {}) - ours {} against the reference's {}",
        worst.0,
        worst.1,
        worst.2,
        forest_noise(worst.1, worst.2, reference.seed),
        reference.rows[(worst.2 - reference.centre.1 + reference.radius) as usize]
            [(worst.1 - reference.centre.0 + reference.radius) as usize]
    );
}

#[test]
fn the_reference_field_stays_inside_the_range_the_threshold_expects() {
    let reference = reference();
    let values: Vec<f32> = reference.rows.iter().flatten().copied().collect();
    assert_eq!(values.len(), 1681);
    assert!(values.iter().all(|value| (0.0..=1.0).contains(value)));
    let forest = values.iter().filter(|value| **value > 0.25).count();
    assert_eq!(
        forest, 500,
        "the share of the reference's window above the forest threshold"
    );
}

#[test]
fn a_different_seed_is_a_different_world() {
    let reference = reference();
    let same = forest_noise(reference.centre.0, reference.centre.1, reference.seed);
    let other = forest_noise(
        reference.centre.0,
        reference.centre.1,
        reference.seed.wrapping_add(1),
    );
    assert!((same - other).abs() > 1e-3, "the seed separates worlds");
}
