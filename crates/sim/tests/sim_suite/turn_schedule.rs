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
            TurnStage::Scent,
            TurnStage::Falling,
            TurnStage::Fields,
            TurnStage::Creatures,
            TurnStage::Light,
        ],
        "scent, then what fell, then the fields, then the creatures, then the light"
    );
}
