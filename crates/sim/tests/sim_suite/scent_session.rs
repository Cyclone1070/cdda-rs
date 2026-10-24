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
    let mut session_digest = 0xcbf2_9ce4_8422_2325_u64;
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
        // **The turn as one number**, over the scent layer. The square-by-square list above says where a turn differs; this says that it does, and the fold over the
        // session is the receipt. Scent is layer one, because fields are layer zero in the same digest.
        // **Like for like**: the digest covers the squares the port's window holds, so a record that lists a square outside it - the dump's window is the fields' radius
        // times six, and the port's window is its own - cannot make the two numbers differ for a reason that is not the scent.
        let theirs_inside: Vec<(i32, i32, i32)> = theirs
            .iter()
            .copied()
            .filter(|(x, y, _)| {
                (window.min_x()..=window.max_x()).contains(x) && (window.min_y()..=window.max_y()).contains(y)
            })
            .collect();
        let ours_digest = scent_digest(&ours);
        let theirs_digest = scent_digest(&theirs_inside);
        if ours_digest != theirs_digest && wrong.is_empty() {
            // A digest can only disagree here if the square-by-square comparison above missed it, which would mean a square the port reports that the record does not or
            // the other way round. Saying so is worth more than the number itself.
            let mine: std::collections::BTreeMap<(i32, i32), i32> =
                ours.iter().map(|(x, y, value)| ((*x, *y), *value)).collect();
            let reference: std::collections::BTreeMap<(i32, i32), i32> = theirs_inside
                .iter()
                .filter(|(_, _, value)| *value != 0)
                .map(|(x, y, value)| ((*x, *y), *value))
                .collect();
            let mut extra: Vec<_> = mine
                .iter()
                .filter(|(square, value)| reference.get(square) != Some(value))
                .map(|(square, value)| (square.0, square.1, *value, reference.get(square).copied().unwrap_or(-1)))
                .collect();
            extra.extend(
                reference
                    .iter()
                    .filter(|(square, _)| !mine.contains_key(square))
                    .map(|(square, value)| (square.0, square.1, -1, *value)),
            );
            let ours_nonzero = ours.iter().filter(|(_, _, value)| *value != 0).count();
            let theirs_nonzero = theirs_inside.iter().filter(|(_, _, value)| *value != 0).count();
            let mut duplicates = 0_usize;
            let mut visited = std::collections::BTreeSet::new();
            for (x, y, value) in &theirs_inside {
                if *value != 0 && !visited.insert((*x, *y)) {
                    duplicates += 1;
                }
            }
            wrong.push((
                index,
                i32::try_from(ours_nonzero).unwrap_or(-1),
                i32::try_from(theirs_nonzero).unwrap_or(-1),
                i32::try_from(duplicates).unwrap_or(-1),
                i32::try_from(extra.len()).unwrap_or(-1),
            ));
        }
        session_digest = crate::replay_digest::fold(session_digest, index, ours_digest);
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

/// One turn of scent, hashed as layer one.
fn scent_digest(squares: &[(i32, i32, i32)]) -> u64 {
    // **One value per square, and only squares that hold something.** Two things about the record force this: it lists every square of its window including the zeroes,
    // and it lists the window on **each of three levels**, so a square appears two or three times in the same turn. A digest over the raw list would depend on the dump's
    // shape rather than on the scent.
    let mut per_square: std::collections::BTreeMap<(i32, i32), i32> = std::collections::BTreeMap::new();
    for (x, y, value) in squares.iter().filter(|(_, _, value)| *value != 0) {
        per_square.insert((*x, *y), *value);
    }
    crate::replay_digest::digest(
        &per_square
            .iter()
            .map(|((x, y), value)| (1, *x, *y, 0, 0, i64::from(*value)))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn the_scent_session_has_a_receipt_too() {
    // The same receipt rule as the gas sessions: one number for a whole session, pinned so a change of behaviour is a diff rather than a drift. Scent is layer one, so
    // this number and the gas receipts are values of the same digest and could be folded together if a session ever carried both.
    let receipt = scent_receipt();
    assert_eq!(receipt, 0xaac1_3754_0710_41c0_u64, "the scent session receipt is {receipt:#018x}");
}

/// The scent session's receipt, recomputed over the **record**: the port's own replay is gated turn by turn above, and what is pinned here is the fold of the recorded
/// layer, so the number says what the session contains rather than what the port produced from it.
fn scent_receipt() -> u64 {
    let turns = turns();
    let Some(first) = turns.first() else {
        return 0;
    };
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for (index, turn) in turns.iter().enumerate().skip(1) {
        let mut theirs = turn.clone();
        theirs.sort_unstable();
        let _ = first;
        digest = crate::replay_digest::fold(digest, index, scent_digest(&theirs));
    }
    digest
}
