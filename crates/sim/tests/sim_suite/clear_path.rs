//! Whether a line of fire is clear, on the same small map the reference was given.
//!
//! Reference: `map::clear_path` (`src/map.cpp:7723-7800`), the rule a ranged attack asks before it is allowed. Three of its habits are worth more than the happy path:
//!
//! * the **last** square is not checked at all — "it's still reachable even if it is an obstacle" — so a target standing *on* a wall is a clear shot;
//! * the flat branch admits a cost **inclusively** (`cost < cost_min || cost > cost_max` fails), while the climbing branch compares **strictly**
//!   (`cost > cost_min && cost < cost_max`), so a window of a single value can never be clear when the line climbs;
//! * a climb is allowed if either column has support at the higher level, which is why the fixture has floor above one square and open air above another.
//!
//! `corpus/line/clear_path.json` records the reference's twenty answers — the ten shapes under two cost windows — on a 31-square floor with a wall column at `x + 5`.

#![allow(clippy::expect_used, clippy::panic)]

use crate::support::line_of_fire_map;

fn corpus() -> cdda_json::Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/line/clear_path.json");
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
fn every_recorded_answer_is_reproduced() {
    let map = line_of_fire_map();
    let corpus = corpus();
    let cdda_json::Value::Object(root) = &corpus else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(cases)) = root.get("cases") else {
        panic!("the recording has cases");
    };
    assert_eq!(cases.len(), 20, "ten shapes under two cost windows");
    let mut clear = 0;
    for case in cases {
        let cdda_json::Value::Object(entry) = case else {
            panic!("a case is an object");
        };
        let Some(cdda_json::Value::Str(shape)) = entry.get("shape") else {
            panic!("a case names its shape");
        };
        let from = point(entry.get("from").expect("from"));
        let to = point(entry.get("to").expect("to"));
        let range = number(entry.get("range").expect("range"));
        let cost_min = number(entry.get("cost_min").expect("cost_min"));
        let cost_max = number(entry.get("cost_max").expect("cost_max"));
        let Some(cdda_json::Value::Bool(expected)) = entry.get("clear") else {
            panic!("a case says whether it is clear");
        };
        let got = cdda_sim::clear_path(from, to, range, cost_min, cost_max, &map);
        assert_eq!(
            got, *expected,
            "{shape} with costs {cost_min}..={cost_max}: from {from:?} to {to:?} at range {range}"
        );
        if got {
            clear += 1;
        }
    }
    assert_eq!(clear, 9, "nine of the twenty answers are clear");
}

#[test]
fn a_target_on_the_wall_is_a_clear_shot() {
    let map = line_of_fire_map();
    // The walk stops before examining the last square, so the wall the target stands on is never asked about.
    assert!(cdda_sim::clear_path([0, 0, 0], [5, 0, 0], -1, 2, 2, &map));
}

#[test]
fn the_same_cost_window_that_lets_a_flat_shot_through_refuses_a_climb() {
    let map = line_of_fire_map();
    // Flat, with a single-value window: inclusive, so it passes. Climbing: strict, so it cannot.
    assert!(cdda_sim::clear_path([0, 0, 0], [4, 0, 0], -1, 2, 2, &map));
    assert!(!cdda_sim::clear_path([0, 0, 0], [3, 0, 1], -1, 2, 2, &map));
    // Widen the window by one and the climb is allowed over a floor.
    assert!(cdda_sim::clear_path([0, 0, 0], [3, 0, 1], -1, 1, 3, &map));
}
