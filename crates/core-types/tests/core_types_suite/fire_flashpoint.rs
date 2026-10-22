//! The flashpoint scan only runs while the fire has fuel banked.
//!
//! Reference: `map_field.cpp:1178-1210`. The loop's own condition is
//! `count != neighs.size() && cur.get_field_age() < 0_turns`, and it is re-checked after every neighbour because the flashpoint **pays** for what it does:
//! five minutes of the fire's own life per neighbouring fire it wakes up. A fire with nothing banked (`age >= 0`) does not scan at all - so it also counts
//! no neighbours, and the smoke and hot-air gates it feeds see a count of zero.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{Neighbour, spread_phase};
use cdda_core_types::rng::{Recording, Rolls};

/// A ring whose first two squares are on fire and the rest are bare.
fn ring_with_two_fires() -> [Neighbour; 8] {
    let mut neighbours = [Neighbour::default(); 8];
    for neighbour in neighbours.iter_mut().take(2) {
        *neighbour = Neighbour {
            has_fire: true,
            fire_intensity: 1,
            fire_age: 60,
            ..Neighbour::default()
        };
    }
    neighbours
}

#[test]
fn a_fire_with_nothing_banked_scans_nothing_and_counts_nothing() {
    // Age zero: the flashpoint die succeeds and the scan start is drawn - the reference draws it before it looks at the loop condition - and then the loop
    // stops on its first check, so no neighbour is looked at and none is counted.
    let neighbours = ring_with_two_fires();
    let mut rolls =
        Rolls::replaying(Recording::parse("int 0 2 0\nint 0 7 7").expect("a recording"));
    let phase = spread_phase(2, 0, true, &neighbours, &mut rolls);
    assert!(phase.boosted.is_empty(), "nothing was woken up");
    assert_eq!(phase.spent, 0, "and nothing was paid for");
    assert_eq!(
        phase.adjacent, 0,
        "a fire that never looked cannot count its neighbours - which is why the hot-air gate asks a different question next"
    );
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn a_banked_fire_wakes_its_neighbours_and_pays_for_each() {
    // Ten minutes banked: five minutes each is short of zero, so both burning neighbours are woken.
    let neighbours = ring_with_two_fires();
    let mut rolls =
        Rolls::replaying(Recording::parse("int 0 2 0\nint 0 7 7").expect("a recording"));
    let phase = spread_phase(2, -600, true, &neighbours, &mut rolls);
    assert_eq!(
        phase.boosted,
        vec![0, 1],
        "the scan starts at seven and wraps to the first two squares"
    );
    assert_eq!(phase.spent, 600, "five minutes for each of them");
    assert_eq!(phase.adjacent, 2, "and both were counted");
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn the_scan_stops_when_the_fire_spends_its_last_minute() {
    // Sixty seconds banked and two neighbours to wake: the first costs more than the fire has, so the loop stops after it and the second is never counted.
    let neighbours = ring_with_two_fires();
    let mut rolls =
        Rolls::replaying(Recording::parse("int 0 2 0\nint 0 7 7").expect("a recording"));
    let phase = spread_phase(2, -60, true, &neighbours, &mut rolls);
    assert_eq!(
        phase.boosted,
        vec![0],
        "one neighbour, because after paying for it the fuel is gone"
    );
    assert_eq!(phase.adjacent, 1, "and the one after it was never reached");
}
