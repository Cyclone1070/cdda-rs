//! How long cutting up a corpse takes.
//!
//! Every expected number here is computed from the reference's own order of operations (`butcher_time_to_cut`,
//! `src/butchery.cpp`): a base by size, shortened by the tool with a floor, multiplied by the action, then adjusted
//! by proficiency and helpers as fractions. Change the order and the numbers move, which is the point of pinning
//! them.

#![allow(clippy::panic)]

use cdda_sim::{ButcherAction, ButcherState, CreatureSize, butcher_time_to_cut};

fn state(size: CreatureSize, action: ButcherAction) -> ButcherState {
    ButcherState {
        size,
        action,
        tool_factor: 0,
        field_dressed: false,
        field_dress_failed: false,
        quartered: false,
        instant_bleed: false,
        butchering_basic: 0.0,
        butchering_advanced: 0.0,
        skinning_basic: 0.0,
        helpers: 0,
    }
}

#[test]
fn a_person_quickly_cut_up_with_a_bare_hands_tool() {
    // 450 base, no tool, quick multiplies by 2.4 with no proficiency: 107,999 after truncation.
    assert_eq!(
        butcher_time_to_cut(state(CreatureSize::Medium, ButcherAction::Quick)),
        107_999
    );
}

#[test]
fn the_same_corpse_skinned_takes_less() {
    // 450 * 100 * 2 = 90,000 before the skinning adjustment, which is zero practice: 30,000 for a tiny one.
    assert_eq!(
        butcher_time_to_cut(state(CreatureSize::Tiny, ButcherAction::Skin)),
        30_000
    );
}

#[test]
fn a_workshop_job_costs_more_when_the_corpse_was_not_dressed() {
    let mut dressed = state(CreatureSize::Medium, ButcherAction::Full);
    dressed.field_dressed = true;
    let mut undressed = state(CreatureSize::Medium, ButcherAction::Full);
    undressed.field_dressed = false;
    assert_eq!(butcher_time_to_cut(dressed), 324_000);
    assert_eq!(
        butcher_time_to_cut(undressed),
        486_000,
        "a half-done job costs more"
    );
}

#[test]
fn a_failed_field_dressing_counts_as_not_done() {
    let mut failed = state(CreatureSize::Medium, ButcherAction::Full);
    failed.field_dressed = true;
    failed.field_dress_failed = true;
    assert_eq!(butcher_time_to_cut(failed), 486_000);
}

#[test]
fn quartering_and_dismembering_have_floors() {
    assert_eq!(
        butcher_time_to_cut(state(CreatureSize::Medium, ButcherAction::Quarter)),
        11_250
    );
    assert_eq!(
        butcher_time_to_cut(state(CreatureSize::Medium, ButcherAction::Dismember)),
        4_500
    );
}

#[test]
fn dissecting_is_the_longest_job() {
    assert_eq!(
        butcher_time_to_cut(state(CreatureSize::Huge, ButcherAction::Dissect)),
        1_080_000
    );
}

#[test]
fn bleeding_is_instant_for_a_butcher_who_does_that() {
    let mut state = state(CreatureSize::Medium, ButcherAction::Bleed);
    state.instant_bleed = true;
    assert_eq!(butcher_time_to_cut(state), 1, "one move, not one turn");
}

#[test]
fn a_good_tool_saves_time_but_never_more_than_three_quarters() {
    let mut sharp = state(CreatureSize::Medium, ButcherAction::Quick);
    sharp.tool_factor = 75;
    assert_eq!(
        butcher_time_to_cut(sharp),
        26_999,
        "a quarter of the work left"
    );
    let mut absurd = state(CreatureSize::Medium, ButcherAction::Quick);
    absurd.tool_factor = 1_000;
    assert_eq!(
        butcher_time_to_cut(absurd),
        26_999,
        "the floor is a quarter, however good the knife"
    );
}

#[test]
fn practice_shortens_the_work_and_a_master_pays_no_penalty() {
    let mut master = state(CreatureSize::Medium, ButcherAction::Quick);
    master.butchering_basic = 1.0;
    master.butchering_advanced = 1.0;
    master.skinning_basic = 1.0;
    assert_eq!(
        butcher_time_to_cut(master),
        45_000,
        "just the 2.4 factor work"
    );
}

#[test]
fn helpers_shorten_the_work_and_three_is_the_most_that_counts() {
    let mut helped = state(CreatureSize::Medium, ButcherAction::Skin);
    helped.quartered = true;
    helped.helpers = 3;
    assert_eq!(butcher_time_to_cut(helped), 15_749);
    let mut crowded = helped;
    crowded.helpers = 9;
    assert_eq!(
        butcher_time_to_cut(crowded),
        15_749,
        "a crowd of helpers is no better than three"
    );
}
