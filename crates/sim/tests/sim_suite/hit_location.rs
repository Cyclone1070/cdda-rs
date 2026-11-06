//! Which part of a body a blow lands on.
//!
//! Source: `anatomy::select_body_part` (`src/anatomy.cpp:208-256`) and `weighted_float_list::pick_ent`
//! (`src/weighted_list.h`), transcribed case by case. The draw is a fraction rather than a die so each case states
//! the **boundary** it is testing: a rule that picked with `>` instead of `>=`, or that forgot to triple a lower
//! limb, moves the boundary and these tests notice.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_sim::hit_location::{BodyPart, hit_weights, pick_by_weight, select_body_part};

fn part(name: &str, hit_size: f32, hit_difficulty: f32) -> BodyPart {
    BodyPart {
        name: name.to_owned(),
        hit_size,
        hit_difficulty,
        upper: false,
        lower: false,
        ..BodyPart::default()
    }
}

fn torso_and_arm() -> Vec<BodyPart> {
    vec![part("torso", 10.0, 0.0), part("arm", 5.0, 0.0)]
}

#[test]
fn the_first_entry_whose_weight_reaches_the_draw_wins() {
    // Total fifteen: the draw is scaled to it, and `>=` means the torso takes everything up to its own weight.
    assert_eq!(pick_by_weight(&[10.0, 5.0], 0.0), Some(0), "the very bottom is the first");
    assert_eq!(pick_by_weight(&[10.0, 5.0], 0.5), Some(0), "half of fifteen is seven and a half");
    assert_eq!(
        pick_by_weight(&[10.0, 5.0], 2.0 / 3.0),
        Some(0),
        "two thirds of fifteen is exactly ten, and `>=` keeps it on the boundary"
    );
    assert_eq!(
        pick_by_weight(&[10.0, 5.0], 0.7),
        Some(1),
        "past the boundary the arm takes it"
    );
}

#[test]
fn a_list_of_zero_weights_picks_nothing() {
    assert_eq!(pick_by_weight(&[0.0, 0.0], 0.5), None, "the reference returns null");
    assert_eq!(pick_by_weight(&[], 0.5), None, "and an empty body is the same");
}

#[test]
fn a_part_too_small_or_too_large_for_the_blow_is_not_a_target() {
    let parts = torso_and_arm();
    assert_eq!(
        hit_weights(&parts, 6, -1, true, 0),
        vec![10.0],
        "a minimum of six discards the arm"
    );
    assert_eq!(
        hit_weights(&parts, 0, 6, true, 0),
        vec![5.0],
        "and a maximum of six discards the torso"
    );
    assert_eq!(
        select_body_part(&parts, 6, -1, true, 0, 0.99).map(|found| found.name.clone()),
        Some("torso".to_owned()),
        "so the answer is never a part the filter removed"
    );
}

#[test]
fn an_attacker_that_cannot_reach_high_loses_the_head_and_triples_the_legs() {
    let head = BodyPart {
        upper: true,
        ..part("head", 1.0, 0.0)
    };
    let leg = BodyPart {
        lower: true,
        ..part("leg", 5.0, 0.0)
    };
    let parts = vec![part("torso", 10.0, 0.0), head, leg];

    assert_eq!(
        hit_weights(&parts, 0, -1, false, 0),
        vec![10.0, 15.0],
        "the head is gone and the leg's five became fifteen"
    );
    // Ten of twenty-five is exactly a draw of 0.4, so the boundary sits there and `>=` keeps 0.4 on the torso.
    assert_eq!(
        select_body_part(&parts, 0, -1, false, 0, 0.4).map(|found| found.name.clone()),
        Some("torso".to_owned()),
        "exactly ten of twenty-five belongs to the torso"
    );
    assert_eq!(
        select_body_part(&parts, 0, -1, false, 0, 0.39).map(|found| found.name.clone()),
        Some("torso".to_owned())
    );
    assert_eq!(
        select_body_part(&parts, 0, -1, false, 0, 0.41).map(|found| found.name.clone()),
        Some("leg".to_owned()),
        "and past it the tripled leg takes the rest"
    );
}

#[test]
fn a_better_roll_favours_the_parts_that_are_hard_to_reach() {
    let parts = vec![part("torso", 10.0, 0.0), part("eye", 5.0, 1.0)];
    assert_eq!(
        hit_weights(&parts, 0, -1, true, 4),
        vec![10.0, 20.0],
        "a roll of four against difficulty one makes the eye twenty: five times four"
    );
    assert_eq!(
        hit_weights(&parts, 0, -1, true, 0),
        vec![10.0, 5.0],
        "and no roll leaves the sizes alone"
    );
    assert_eq!(
        select_body_part(&parts, 0, -1, true, 4, 0.3).map(|found| found.name.clone()),
        Some("torso".to_owned()),
        "ten of thirty is a third, so 0.3 of the draw is nine"
    );
    assert_eq!(
        select_body_part(&parts, 0, -1, true, 4, 0.4).map(|found| found.name.clone()),
        Some("eye".to_owned()),
        "past a third the eye takes the rest"
    );
}
