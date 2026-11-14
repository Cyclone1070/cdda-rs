//! The three-dimensional line walk, branch by branch.
//!
//! Reference: `line_to( tripoint, tripoint, t, t2 )` and its `bresenham` (`src/line.cpp:73-215, 241-257`). The walk has **ten** branches: three for a flat line, split by
//! which of the two level axes dominates, and seven for one that climbs, split by which axis dominates and which two are equal. The two starting error terms decide
//! which neighbour is taken first when a step is exactly on a tie. A gate that only walks a diagonal proves nothing about the other nine, so `corpus/line/walks.json`
//! names one pair per branch, then repeats several of them with error terms of both signs.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::line::line_to_3d;

fn corpus() -> cdda_json::Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/line/walks.json");
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

fn string(value: Option<&cdda_json::Value>) -> String {
    let Some(cdda_json::Value::Str(text)) = value else {
        panic!("expected a string, found {value:?}");
    };
    text.clone()
}

#[test]
fn every_branch_of_the_three_dimensional_walk_matches_the_reference() {
    let corpus = corpus();
    let cdda_json::Value::Object(root) = &corpus else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("the recording has cases");
    };
    assert_eq!(
        cases.len(),
        20,
        "twenty walks: every branch, then repeats with error terms"
    );
    let mut branches = std::collections::BTreeSet::new();
    for case in cases {
        let cdda_json::Value::Object(entry) = case else {
            panic!("a case is an object");
        };
        let branch = string(entry.get("branch"));
        branches.insert(branch.clone());
        let from = point(entry.get("from").expect("from"));
        let to = point(entry.get("to").expect("to"));
        let t = number(entry.get("t").expect("t"));
        let t2 = number(entry.get("t2").expect("t2"));
        let Some(cdda_json::Value::Array(cells)) = entry.get("cells") else {
            panic!("a case has cells");
        };
        let expected: Vec<[i32; 3]> = cells.iter().map(point).collect();
        assert_eq!(
            line_to_3d(from, to, t, t2),
            expected,
            "{branch}: from {from:?} to {to:?} with errors {t}/{t2}"
        );
    }
    // Every walk is labelled with the branch it is meant to exercise, so the labels are the coverage check: ten branches, plus the label the empty walk carries.
    assert_eq!(
        branches.len(),
        11,
        "the sample has to cover all ten branches plus the empty case, and it covered {branches:?}"
    );
}

#[test]
fn a_line_that_goes_nowhere_is_the_origin_itself() {
    assert_eq!(line_to_3d([7, 7, 0], [7, 7, 0], 0, 0), vec![[7, 7, 0]]);
}

#[test]
fn the_error_terms_change_which_neighbour_is_taken_first() {
    // The two recorded `flat-x` walks with `t = 3` and `t = -3` differ in their very first step, which is the point of passing the terms through rather than
    // normalising them.
    let positive = line_to_3d([10, 10, 0], [16, 12, 0], 3, 0);
    let negative = line_to_3d([10, 10, 0], [16, 12, 0], -3, 0);
    assert_ne!(positive, negative);
    assert_eq!(positive.first(), Some(&[11, 11, 0]));
    assert_eq!(negative.first(), Some(&[11, 10, 0]));
}
