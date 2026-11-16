//! The turn runs as a schedule, and the order it declares is the order it executes.
//!
//! Reference: `game::do_turn` (`src/do_turn.cpp:642-655`) runs the scent, then what fell, then the
//! fields, then `monmove` - which is where creatures act - and only then rebuilds the light. The port states that order once, as system sets, so a stage
//! cannot be quietly reordered by moving a line in a function.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::turn::{TurnStage, run_turn, trace_of};

#[test]
fn the_stages_run_in_the_order_the_schedule_declares() {
    let mut world = World::new();
    run_turn(&mut world);

    assert_eq!(
        trace_of(&mut world),
        TurnStage::order().to_vec(),
        "the executed order is the declared order"
    );
}

#[test]
fn the_declared_order_is_the_one_the_reference_names() {
    // Written out by hand, because this is the assertion a reordering has to break.
    assert_eq!(
        TurnStage::order(),
        [
            // Events that have come due run before the world's own stages, the way `game::process_events` does (`do_turn.cpp`).
            TurnStage::Scheduled,
            TurnStage::Scent,
            // A sound is over before anything falls and before any creature walks: `sounds::process_sound` settles a creature's memory of a noise inside the turn that
            // made it.
            TurnStage::Sounds,
            TurnStage::Falling,
            // Vehicles move after the ground has settled under them and before the fields act: `do_turn.cpp:653-655` runs `process_falling`, `vehmove`, `process_fields`.
            TurnStage::Vehicles,
            TurnStage::Fields,
            TurnStage::Creatures,
            TurnStage::Light,
        ],
        "the due events, then scent, then what was heard, then what fell, then the fields, then the creatures, then the light"
    );
}

#[test]
fn a_turn_does_not_rebuild_its_schedule() {
    // A schedule is a thing a turn *runs*, not a thing a turn *is*. Rebuilding it every turn cost 160 ms of wall clock for 90 us of work - the playtest gate paid it for
    // 1,080 turns in one scenario and 1,070 seconds in total.
    let mut world = World::new();
    cdda_sim::turn::run_turn(&mut world);
    let after_first = cdda_sim::turn::schedules_built();

    for _ in 0..5 {
        cdda_sim::turn::run_turn(&mut world);
    }
    assert_eq!(
        cdda_sim::turn::schedules_built(),
        after_first,
        "**five more turns built no more schedules**"
    );
}

/// **Vehicles take their turn between the falling ground and the fields.** Reference: `do_turn.cpp:653-655` runs `process_falling`, then `vehmove`, then `process_fields`.
#[test]
fn the_vehicles_take_their_turn_between_falling_and_fields() {
    let order = TurnStage::order();
    let falling = order
        .iter()
        .position(|stage| *stage == cdda_sim::turn::TurnStage::Falling)
        .expect("falling runs");
    let vehicles = order
        .iter()
        .position(|stage| *stage == cdda_sim::turn::TurnStage::Vehicles)
        .expect("**the vehicles have a stage of their own**");
    let fields = order
        .iter()
        .position(|stage| *stage == cdda_sim::turn::TurnStage::Fields)
        .expect("fields run");
    assert!(falling < vehicles, "**the ground settles first**");
    assert!(
        vehicles < fields,
        "**and the fields act after the vehicles**"
    );
}
