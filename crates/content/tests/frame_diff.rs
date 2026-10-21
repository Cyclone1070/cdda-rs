//! Comparing two views, and saying exactly where they differ.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::frames::{Frame, FrameSquare, diff, read};

fn square(x: i32, y: i32, glyph: &str, terrain: &str, items: i32, light: f64) -> FrameSquare {
    FrameSquare {
        x,
        y,
        glyph: glyph.to_owned(),
        terrain: terrain.to_owned(),
        items,
        light,
    }
}

fn frame(squares: Vec<FrameSquare>) -> Frame {
    Frame {
        radius: 1,
        centre: (0, 0),
        squares,
    }
}

fn pair() -> (Frame, Frame) {
    let left = frame(vec![
        square(0, 0, ".", "t_floor", 0, 4.0),
        square(1, 0, "#", "t_wall", 0, 0.0),
        square(0, 1, ".", "t_floor", 1, 4.0),
    ]);
    let right = left.clone();
    (left, right)
}

#[test]
fn a_frame_compared_with_itself_has_no_differences() {
    let (left, right) = pair();
    assert!(diff(&left, &right).is_empty());
}

#[test]
fn a_changed_glyph_is_named_by_square_and_field() {
    let (left, mut right) = pair();
    right.squares[1].glyph = "|".to_owned();
    let differences = diff(&left, &right);
    assert_eq!(differences.len(), 1, "one difference, not a whole frame");
    let first = differences.first().expect("a difference");
    assert_eq!((first.x, first.y), (1, 0));
    assert_eq!(first.field, "glyph");
    assert_eq!((first.left.as_str(), first.right.as_str()), ("#", "|"));
}

#[test]
fn terrain_items_and_light_are_each_reported() {
    let (left, mut right) = pair();
    right.squares[0].terrain = "t_floor_waxed".to_owned();
    right.squares[2].items = 3;
    right.squares[2].light = 9.0;
    let fields: Vec<&str> = diff(&left, &right).iter().map(|d| d.field).collect();
    assert_eq!(fields, vec!["terrain", "items", "light"]);
}

#[test]
fn a_light_difference_too_small_to_see_is_not_a_difference() {
    let (left, mut right) = pair();
    right.squares[0].light += 0.004;
    assert!(
        diff(&left, &right).is_empty(),
        "rounding is not a disagreement"
    );
}

#[test]
fn a_square_missing_from_one_side_is_reported_as_such() {
    let (left, mut right) = pair();
    right.squares.pop();
    let differences = diff(&left, &right);
    assert_eq!(differences.len(), 1);
    assert_eq!(differences.first().map(|d| d.field), Some("square"));
}

#[test]
fn the_reference_dump_reads_back_without_loss() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/frames/start.json");
    let text = std::fs::read_to_string(path).expect("the frame dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let frame = read(&value).expect("the dump reads");
    assert_eq!(frame.radius, 20);
    assert_eq!(frame.squares.len(), 1_681);
    let here = frame
        .squares
        .iter()
        .find(|square| (square.x, square.y) == frame.centre)
        .expect("the centre");
    assert_eq!(here.terrain, "t_floor");
    // Reading the same dump twice gives the same frame, so a diff against it means something.
    let again = read(&value).expect("reads again");
    assert_eq!(again, frame);
}
