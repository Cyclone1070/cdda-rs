//! Scent diffusion replayed against a recording.
//!
//! The M2 exit gate asks for an environment session that covers **scent diffusion**, and until this round no session could: the map's scent layer is only ever
//! filled by a character walking, so every recorded fixture carried an empty one. `--place-scent` puts a value on the player's square and the session now runs the
//! game loop's own `scent_map::update` between turns, so the record spreads the way the reference's window spreads it.
//!
//! Reference: `scent_map::update` (`src/scent_map.cpp:208-267`) - which diffuses and does **not** decay - and the per-turn call in `do_turn.cpp:642-648`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use cdda_core_types::scent::scent_window;
use std::path::Path;

/// One square's scent, per turn, exactly as the reference reports it.
type Squares = Vec<(i32, i32, i32)>;

fn whole(value: &cdda_json::Value) -> i32 {
    match value {
        cdda_json::Value::Int(number) => i32::try_from(*number).expect("a coordinate"),
        cdda_json::Value::Float(number) => *number as i32,
        other => panic!("a whole number: {other:?}"),
    }
}

fn turns() -> Vec<Squares> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields/scent-session.json");
    let text = std::fs::read_to_string(&path).expect("the recorded session");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(turns)) = root.get("turns") else {
        panic!("turns")
    };
    turns
        .iter()
        .map(|turn| {
            let cdda_json::Value::Object(turn) = turn else {
                panic!("a turn")
            };
            match turn.get("scent") {
                Some(cdda_json::Value::Array(entries)) => entries
                    .iter()
                    .map(|entry| match entry {
                        cdda_json::Value::Array(triple) if triple.len() == 3 => {
                            (whole(&triple[0]), whole(&triple[1]), whole(&triple[2]))
                        }
                        other => panic!("a scent square: {other:?}"),
                    })
                    .collect(),
                _ => Vec::new(),
            }
        })
        .collect()
}

/// The squares the port's window reports, in the same shape the record uses.
fn window_squares(window: &cdda_core_types::scent::ScentWindow) -> Squares {
    let mut out = Vec::new();
    for y in window.min_y()..=window.max_y() {
        for x in window.min_x()..=window.max_x() {
            let value = window.get(x, y);
            if value > 0 {
                out.push((x, y, value));
            }
        }
    }
    out.sort_unstable();
    out
}

#[test]
fn a_recorded_scent_layer_diffuses_the_way_the_reference_says() {
    // **The whole point of the fixture.** One square seeded on turn zero and nothing else: every value after that is the window's own arithmetic, so a port that
    // rounds differently, weights a neighbour differently or diffuses in the wrong direction shows up on the very first turn and keeps showing up.
    let turns = turns();
    let first = turns.first().expect("a first turn");
    assert_eq!(
        first.len(),
        1,
        "turn zero is the seeded square alone: {first:?}"
    );
    let (cx, cy, seed) = first[0];
    let mut window = scent_window((cx, cy));
    window.set(cx, cy, seed);
    let mut compared = 0;
    let mut wrong: Vec<(usize, i32, i32, i32, i32)> = Vec::new();
    for (index, recorded) in turns.iter().enumerate().skip(1) {
        // **No decay.** The window's diffusion alone reproduces the record turn for turn, so the decay that `decay_fields_and_scent`
        // (`map.cpp:3418-3420`) applies is not part of what this session shows: the record's centre falls 500, 100, 60, 40, 30 - the geometric
        // sequence of a pure diffusion of a single peak - and subtracting a point anywhere makes the ring one too low.
        window.diffuse();
        let ours = window_squares(&window);
        let mut theirs = recorded.clone();
        theirs.sort_unstable();
        // **Only the squares that differ**, because a whole-layer dump hides a single square in eighty-one rows of agreement.
        if ours != theirs && wrong.len() < 8 {
            let mine: std::collections::BTreeMap<(i32, i32), i32> =
                ours.iter().map(|(x, y, value)| ((*x, *y), *value)).collect();
            let reference: std::collections::BTreeMap<(i32, i32), i32> =
                theirs.iter().map(|(x, y, value)| ((*x, *y), *value)).collect();
            for square in mine.keys().chain(reference.keys()) {
                let (ours_here, theirs_here) =
                    (mine.get(square).copied().unwrap_or(0), reference.get(square).copied().unwrap_or(0));
                if ours_here != theirs_here && wrong.len() < 8 {
                    wrong.push((index, square.0, square.1, ours_here, theirs_here));
                }
            }
        }
        compared += 1;
    }
    assert!(compared >= 10, "every recorded turn was diffused: {compared}");
    assert!(
        wrong.is_empty(),
        "the port's scent disagrees with the recording (turn, x, y, ours, theirs): {wrong:#?}"
    );
}

#[test]
fn the_recorded_layer_actually_diffuses() {
    // A fixture whose scent never moved would make the gate above pass on a layer that stands still, which is exactly what every session carried before scent
    // could be placed. The record has to gain squares and lose height as it goes.
    let turns = turns();
    let areas: Vec<usize> = turns.iter().map(Vec::len).collect();
    assert!(
        areas.first().copied().unwrap_or(0) == 1,
        "the session starts from one square: {areas:?}"
    );
    assert!(
        areas.last().copied().unwrap_or(0) > 9,
        "and the scent has spread by the last turn: {areas:?}"
    );
    let peak = turns
        .first()
        .and_then(|turn| turn.first())
        .map(|square| square.2)
        .expect("the seeded square");
    let last_peak = turns
        .last()
        .and_then(|turn| turn.iter().map(|square| square.2).max())
        .expect("a last turn");
    assert!(
        last_peak < peak,
        "the centre has thinned from {peak} to {last_peak}"
    );
}
