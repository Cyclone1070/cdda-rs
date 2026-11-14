//! Where a projectile that keeps its heading for a distance arrives, and where the line goes from there.
//!
//! Reference: `move_along_line` and `continue_line` (`src/line.cpp:343-414`). The offset is `distance * slope` truncated back to an integer, so the arithmetic is not the
//! obvious one: a diagonal step of one truncates to no movement at all, which `corpus/line/move_along.json` records and the test counts, so the truncation rule cannot
//! quietly become a rounding rule.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::line::{continue_line, move_along_line};

fn corpus() -> cdda_json::Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/line/move_along.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

fn number(value: &cdda_json::Value) -> i32 {
    let cdda_json::Value::Int(whole) = value else {
        panic!("expected a whole number, found {value:?}");
    };
    i32::try_from(*whole).expect("fits")
}

fn point(value: &cdda_json::Value) -> [i32; 3] {
    let cdda_json::Value::Array(axis) = value else {
        panic!("expected a point, found {value:?}");
    };
    let mut out = [0; 3];
    for (slot, component) in out.iter_mut().zip(axis) {
        *slot = number(component);
    }
    out
}

#[test]
fn the_projectile_lands_where_the_reference_puts_it() {
    let corpus = corpus();
    let cdda_json::Value::Object(root) = &corpus else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("the recording has cases");
    };
    assert_eq!(cases.len(), 13, "thirteen headings");
    let mut truncations = 0;
    for case in cases {
        let cdda_json::Value::Object(entry) = case else {
            panic!("a case is an object");
        };
        let Some(cdda_json::Value::Str(shape)) = entry.get("shape") else {
            panic!("a case names its shape");
        };
        let from = point(entry.get("from").expect("from"));
        let to = point(entry.get("to").expect("to"));
        let distance = number(entry.get("distance").expect("distance"));
        let moved = point(entry.get("moved").expect("moved"));
        let Some(cdda_json::Value::Array(continued)) = entry.get("continued") else {
            panic!("a case has a continued line");
        };
        let expected: Vec<[i32; 3]> = continued.iter().map(point).collect();
        assert_eq!(
            move_along_line(from, from, to, distance),
            moved,
            "{shape}: the landing point"
        );
        assert_eq!(
            continue_line(from, to, distance),
            expected,
            "{shape}: the line from there"
        );
        if distance > 0 && moved == from {
            truncations += 1;
        }
    }
    assert!(
        truncations >= 1,
        "the sample has to contain a walk that truncates to nothing, or the truncation rule is untested"
    );
}

#[test]
fn a_diagonal_step_of_one_truncates_to_no_movement() {
    // The slope is 0.7071 and one step of it is 0.7071, which truncates to zero: the projectile does not move, and that is what the reference records.
    assert_eq!(
        move_along_line([5, 5, 0], [5, 5, 0], [15, 15, 0], 1),
        [5, 5, 0]
    );
}

#[test]
fn a_heading_with_no_direction_does_not_move() {
    // The reference asserts on this (`slope_of`, `src/line.cpp:345`); the port returns the cell unchanged rather than dividing by zero, which is the only
    // difference between them here and the reason it is written down.
    assert_eq!(
        move_along_line([5, 5, 0], [5, 5, 0], [5, 5, 0], 4),
        [5, 5, 0]
    );
    // The continued line is the walk from `to` to `to` again, which is the reference's `numCells == 0` branch: the cell it starts from.
    assert_eq!(continue_line([5, 5, 0], [5, 5, 0], 4), vec![[5, 5, 0]]);
}
