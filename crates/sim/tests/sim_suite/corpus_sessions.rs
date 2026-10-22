//! Every fixture in the manifest replays, turn by turn, without a test written for it.
//!
//! This is the point of the machinery in [`crate::reference_session`]: a new rule gets a new fixture, not a new test. `corpus/fields/replayable.txt` names
//! the sessions whose dice are the fields' own; each one is loaded with its terrain grid, its prototypes and its recorded dice, played turn by turn, and
//! compared with what the reference dumped after every turn.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use crate::reference_session::Session;

fn manifest() -> Vec<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields/replayable.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the manifest at {}: {error}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[test]
fn every_fixture_in_the_manifest_replays_turn_by_turn() {
    let sessions = manifest();
    assert!(
        !sessions.is_empty(),
        "the manifest names at least one session"
    );
    for name in sessions {
        let session = Session::load(&name);
        let name = session.name.clone();
        let replay = session.replay_fires();
        assert_eq!(
            replay.turns.len() + 1,
            session.turns.len(),
            "{name}: one answer per turn after the first"
        );
        for turn in 1..session.turns.len() {
            assert_eq!(
                replay.turns.get(turn - 1),
                Some(&session.fires_at(turn)),
                "{name}: the fires the reference has at turn {turn}"
            );
        }
        assert_eq!(
            replay.dice_left, 0,
            "{name}: and the session spent exactly the dice the reference spent"
        );
    }
}
