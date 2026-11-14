//! The squares a mover is offered when it wants to get closer to a target, in the order it gets them.
//!
//! Reference: `squares_closer_to` (`src/line.cpp:700-733`), which monster movement walks when it picks a step (`src/monmove.cpp:1128`). The **order** is part of the
//! answer: a creature takes the first square it can enter, so a port that offers the same squares in a different order is a different move. `corpus/line/closer.json`
//! holds fourteen cases - the four shapes, the same four walking backwards, the degenerate straight runs, a climb, and a mover that is already there.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::line::squares_closer_to;

fn corpus() -> cdda_json::Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/line/closer.json");
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
fn the_squares_offered_and_their_order_match_the_reference() {
    let corpus = corpus();
    let cdda_json::Value::Object(root) = &corpus else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("the recording has cases");
    };
    assert_eq!(cases.len(), 14, "four shapes, their mirrors, and the edges");
    let mut shapes = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(entry) = case else {
            panic!("a case is an object");
        };
        let Some(cdda_json::Value::Str(shape)) = entry.get("shape") else {
            panic!("a case names its shape");
        };
        shapes.insert(shape.clone());
        let from = point(entry.get("from").expect("from"));
        let to = point(entry.get("to").expect("to"));
        let Some(cdda_json::Value::Array(squares)) = entry.get("squares") else {
            panic!("a case has squares");
        };
        let expected: Vec<[i32; 3]> = squares.iter().map(point).collect();
        assert_eq!(
            squares_closer_to(from, to),
            expected,
            "{shape}: from {from:?} to {to:?}"
        );
    }
    // Every case is labelled with the shape it is meant to exercise, so the labels are the coverage check: the four shapes, their four mirrors walking backwards, the
    // three degenerate straight runs, a climb, and a mover that is already there.
    assert_eq!(
        shapes.len(),
        14,
        "the sample has to keep every labelled shape distinct, and it named {shapes:?}"
    );
}

#[test]
fn a_mover_already_at_its_target_is_offered_nothing() {
    assert!(squares_closer_to([5, 5, 0], [5, 5, 0]).is_empty());
}

#[test]
fn a_climb_is_offered_first_and_then_the_level_ones() {
    // The order is the contract: the three-dimensional step comes first, so a creature that can climb does.
    let squares = squares_closer_to([5, 5, 0], [9, 9, 3]);
    assert_eq!(squares.first(), Some(&[6, 6, 1]));
}
