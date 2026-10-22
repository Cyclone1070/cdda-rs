//! The fixture and the loader agree before anything is replayed on them.
//!
//! `corpus/fields/fire-floor-offset.json` carries the terrain of every square around the action, and `corpus/prototypes/terrain.json` carries what each
//! terrain's flags are. A replay that reads the wrong terrain is not a replay, so this guard asks the two questions the rest of the machinery assumes: the
//! grid is the size the dump claims, and the flags the port derives for the ground under the fire are the ones the reference's own prototype lists.
//!
//! The replay itself lives in [`crate::corpus_sessions`], which runs **every** fixture the manifest names rather than this one.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use crate::reference_session::{facts_for, terrain_flags};
use cdda_sim::FireMap;

#[test]
fn the_terrain_the_recording_names_is_the_terrain_the_port_reads() {
    let flags = terrain_flags();
    let ground = facts_for("t_floor", &flags);
    assert!(ground.flammable_hard, "t_floor is FLAMMABLE_HARD");
    assert!(!ground.flammable, "and not plain FLAMMABLE");
    let wall = facts_for("t_wall_w", &flags);
    assert!(
        wall.flammable,
        "t_wall_w is FLAMMABLE, which is the branch that fuels a fire on one turn rather than on a die"
    );
    assert!(!wall.flammable_hard, "but not FLAMMABLE_HARD");

    let session = crate::reference_session::Session::load("fire-floor-offset");
    assert_eq!(session.grid.len(), 49, "seven by seven around the fire");
    assert_eq!(
        session.turns.len(),
        7,
        "the state before the first turn, and six after it"
    );
    let floor = session
        .grid
        .iter()
        .filter(|(_, _, _, id, _, _)| id == "t_floor")
        .count();
    assert_eq!(
        floor, 41,
        "the room is six by six inside its walls, and the fire stands on one of them"
    );
    let map = session.map();
    let fire = session.fires_at(0);
    assert_eq!(fire.len(), 1, "the session starts with one fire: {fire:?}");
    let (x, y, _, _) = fire[0];
    assert_eq!(
        map.ground((x, y, 0)),
        cdda_core_types::fire::Ground::FlammableHard,
        "and it stands on t_floor, which the prototype says is FLAMMABLE_HARD"
    );
}
