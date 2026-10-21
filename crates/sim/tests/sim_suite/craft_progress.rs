//! Progress through a craft, one turn at a time.
//!
//! Transcribed from `craft_activity_actor::do_turn` (`src/activity_actor.cpp:4163-4240`).

#![allow(clippy::panic, clippy::expect_used)]

use cdda_sim::{CraftingWork, WorkOutcome, craft_turn};

#[test]
fn an_honest_crafter_gets_a_turn_of_progress_per_turn_of_work() {
    let mut work = CraftingWork::new(300.0);
    assert_eq!(craft_turn(&mut work, 100, 1.0), WorkOutcome::Working);
    assert!(
        (work.fraction() - 1.0 / 3.0).abs() < 1e-9,
        "a third of the way"
    );
    assert_eq!(craft_turn(&mut work, 100, 1.0), WorkOutcome::Working);
    assert_eq!(
        craft_turn(&mut work, 100, 1.0),
        WorkOutcome::Finished,
        "and done"
    );
}

#[test]
fn a_slower_crafter_needs_more_turns_for_the_same_work() {
    let mut slow = CraftingWork::new(300.0);
    slow.current_total_moves = 600.0;
    assert_eq!(craft_turn(&mut slow, 100, 0.5), WorkOutcome::Working);
    assert!(
        (slow.fraction() - 1.0 / 6.0).abs() < 1e-9,
        "half a turn's worth"
    );
}

#[test]
fn the_last_turn_does_not_overshoot() {
    let mut work = CraftingWork::new(150.0);
    assert_eq!(craft_turn(&mut work, 1000, 1.0), WorkOutcome::Finished);
    assert!(
        (work.fraction() - 1.0).abs() < 1e-9,
        "exactly finished, not past it"
    );
}

#[test]
fn a_place_that_cannot_be_crafted_in_cancels_rather_than_crawling() {
    let mut work = CraftingWork::new(300.0);
    assert_eq!(craft_turn(&mut work, 100, 0.0), WorkOutcome::Cancelled);
    assert!(work.fraction().abs() < 1e-9, "no progress was made");
    assert_eq!(
        craft_turn(&mut work, 100, -1.0),
        WorkOutcome::Cancelled,
        "nor at a negative speed"
    );
}

#[test]
fn finishing_is_reported_once_and_stays_finished() {
    let mut work = CraftingWork::new(100.0);
    assert_eq!(craft_turn(&mut work, 100, 1.0), WorkOutcome::Finished);
    assert_eq!(
        craft_turn(&mut work, 100, 1.0),
        WorkOutcome::Finished,
        "asking again still says done rather than starting over"
    );
    assert!((work.fraction() - 1.0).abs() < 1e-9);
}
