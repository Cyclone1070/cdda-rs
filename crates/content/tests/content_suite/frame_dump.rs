//! The reference's own view of the world, as glyphs.
//!
//! This is the first gate that compares what a player would *see* rather than what a number says. The
//! port cannot draw yet; what it can do today is prove the dump is complete and self-consistent, so that
//! when the renderer arrives the comparison has something trustworthy to compare against.

#![allow(clippy::expect_used, clippy::panic)]

fn frame() -> serde_free::Frame {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/frames/start.json");
    let text = std::fs::read_to_string(path).expect("the frame dump");
    serde_free::read(&text)
}

/// A minimal reader for the frame dump, kept here so the test does not depend on the sim's types yet.
mod serde_free {
    use cdda_json::{Object, Value};

    pub struct Frame {
        pub radius: i32,
        pub centre: (i32, i32),
        pub squares: Vec<Square>,
    }

    pub struct Square {
        pub x: i32,
        pub y: i32,
        pub glyph: String,
        pub ter: String,
        pub light: f64,
    }

    // A test-only reader of a fixture whose shape the tests themselves assert; the casts and indexes
    // below are guarded by those assertions.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        clippy::indexing_slicing,
        clippy::unwrap_used
    )]
    pub fn read(text: &str) -> Frame {
        let value = cdda_json::parse(text.as_bytes()).expect("valid json");
        let Value::Object(root) = value else {
            panic!("the frame dump is an object");
        };
        let number = |object: &Object, key: &str| -> i64 {
            match object.get(key) {
                Some(Value::Int(value)) => *value,
                Some(Value::Float(value)) => *value as i64,
                _ => panic!("{key} is a number"),
            }
        };
        let radius = number(&root, "radius") as i32;
        let Value::Array(centre) = root.get("centre").expect("centre") else {
            panic!("centre is a pair");
        };
        let centre = match (&centre[0], &centre[1]) {
            (Value::Int(x), Value::Int(y)) => (*x as i32, *y as i32),
            _ => panic!("centre is a pair of numbers"),
        };
        let Value::Array(squares) = root.get("squares").expect("squares") else {
            panic!("squares is a list");
        };
        let squares = squares
            .iter()
            .map(|entry| {
                let Value::Object(square) = entry else {
                    panic!("a square is an object");
                };
                let text = |key: &str| match square.get(key) {
                    Some(Value::Str(value)) => value.clone(),
                    _ => panic!("{key} is a string"),
                };
                let light = match square.get("light") {
                    Some(Value::Float(value)) => *value,
                    Some(Value::Int(value)) => *value as f64,
                    _ => panic!("light is a number"),
                };
                Square {
                    x: number(square, "x") as i32,
                    y: number(square, "y") as i32,
                    glyph: text("glyph"),
                    ter: text("ter"),
                    light,
                }
            })
            .collect();
        Frame {
            radius,
            centre,
            squares,
        }
    }
}

#[test]
fn the_frame_covers_every_square_of_the_view() {
    let frame = frame();
    let side = usize::try_from(frame.radius * 2 + 1).expect("a positive radius");
    assert_eq!(frame.squares.len(), side * side, "a full square of view");
}

#[test]
fn every_square_has_a_glyph_and_a_terrain() {
    let frame = frame();
    for square in &frame.squares {
        assert!(
            !square.glyph.is_empty(),
            "square {},{} has no glyph",
            square.x,
            square.y
        );
        assert!(
            !square.ter.is_empty(),
            "square {},{} has no terrain",
            square.x,
            square.y
        );
    }
}

#[test]
fn every_square_is_there_exactly_once() {
    let frame = frame();
    let mut seen = std::collections::BTreeSet::new();
    for square in &frame.squares {
        assert!(
            seen.insert((square.x, square.y)),
            "square {},{} appears twice",
            square.x,
            square.y
        );
    }
    assert_eq!(seen.len(), frame.squares.len());
}

#[test]
fn the_player_stands_indoors_in_the_light() {
    let frame = frame();
    let here = frame
        .squares
        .iter()
        .find(|square| (square.x, square.y) == frame.centre)
        .expect("the centre is in the frame");
    assert_eq!(here.ter, "t_floor", "the starting shelter has a floor");
    assert!(
        here.light >= 3.5,
        "and it is lit at the start of the game: {}",
        here.light
    );
}
