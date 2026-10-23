//! The reference's own frame, as the gate will compare against it.
//!
//! Source: `--dump-frame` from the instrumented reference (`harness.cpp`), which walks the loaded map around the avatar and writes, per square, the glyph the game would print,
//! the terrain and furniture ids behind it, and the light level it computed. That is the reference half of the semantic frame gate: the port's `Frame` produces the other half.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_json::{Value, parse};
use std::path::Path;

fn squares() -> Vec<(i32, i32, char, f64)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/frames/reference_frame.json");
    let text = std::fs::read_to_string(path).expect("the reference frame exists");
    let value = parse(text.as_bytes()).expect("the frame is valid JSON");
    let Value::Object(root) = value else {
        panic!("the frame's root is not an object");
    };
    let Some(Value::Array(entries)) = root.get("squares") else {
        panic!("the frame has no squares");
    };
    entries
        .iter()
        .map(|entry| {
            let Value::Object(square) = entry else {
                panic!("a square is not an object");
            };
            let number = |key: &str| match square.get(key) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("a coordinate fits"),
                _ => panic!("a square has no {key}"),
            };
            let light = match square.get("light") {
                Some(Value::Float(value)) => *value,
                Some(Value::Int(value)) => f64::from(i32::try_from(*value).expect("small")),
                _ => panic!("a square has no light"),
            };
            let glyph = match square.get("glyph") {
                Some(Value::Str(text)) => text.chars().next().expect("a glyph is a character"),
                _ => panic!("a square has no glyph"),
            };
            (number("x"), number("y"), glyph, light)
        })
        .collect()
}

#[test]
fn the_reference_frame_is_a_square_of_squares_around_the_avatar() {
    // A radius of twenty is forty-one squares across, so forty-one squared: 1,681. The count is the first thing a comparison needs, because a dump that lost a row would
    // otherwise look like a different map rather than a broken dump.
    let squares = squares();
    assert_eq!(squares.len(), 1681, "forty-one by forty-one");
}

#[test]
fn every_square_says_what_is_there_and_how_lit_it_is() {
    // Both halves the gate compares: a glyph to draw, and the light the reference computed for it. A square with no light in the dump would compare equal to a dark square in the
    // port and hide a light that never got computed, so the dump's own light range is pinned: daylight over the trees.
    let squares = squares();
    assert!(squares.iter().all(|(_, _, glyph, _)| *glyph != '\0'));
    let brightest = squares
        .iter()
        .map(|(_, _, _, light)| *light)
        .fold(0.0_f64, f64::max);
    assert!(
        brightest > 50.0,
        "open daylight is bright in the reference's own numbers: {brightest}"
    );
}
