//! Cutting up a corpse over several turns.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    ButcherAction, ButcherState, ButcheryStep, CorpseOf, CreatureSize, ItemTypeId, MonsterKindId,
    butchery_moves_left, butchery_turn, drop_item, start_butchery,
};

const CORPSE: ItemTypeId = ItemTypeId(3_000);

fn state(action: ButcherAction) -> ButcherState {
    ButcherState {
        size: CreatureSize::Medium,
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

fn body(world: &mut World) -> Entity {
    let item = drop_item(world, (0, 0), CORPSE, 1);
    world.entity_mut(item).insert(CorpseOf {
        kind: MonsterKindId(1),
        monster_volume_ml: 62_499,
    });
    item
}

#[test]
fn starting_a_butchery_costs_the_time_the_rule_says() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let moves = start_butchery(&mut world, corpse, state(ButcherAction::Quick)).expect("starts");
    assert_eq!(moves, 107_999, "the same number the rule computes");
    assert_eq!(butchery_moves_left(&world, corpse), Some(107_999));
}

#[test]
fn work_is_spent_a_turn_at_a_time() {
    let mut world = World::new();
    let corpse = body(&mut world);
    start_butchery(&mut world, corpse, state(ButcherAction::Quick)).expect("starts");
    assert_eq!(
        butchery_turn(&mut world, corpse, 100),
        ButcheryStep::Working
    );
    assert_eq!(butchery_moves_left(&world, corpse), Some(107_899));
}

#[test]
fn the_last_turn_finishes_the_work_and_takes_the_activity_off_the_body() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let total = start_butchery(&mut world, corpse, state(ButcherAction::Quick)).expect("starts");
    assert_eq!(
        butchery_turn(&mut world, corpse, total),
        ButcheryStep::Finished
    );
    assert_eq!(
        butchery_moves_left(&world, corpse),
        None,
        "nothing left to do"
    );
}

#[test]
fn a_turn_with_no_moves_does_nothing_at_all() {
    let mut world = World::new();
    let corpse = body(&mut world);
    start_butchery(&mut world, corpse, state(ButcherAction::Quick)).expect("starts");
    assert_eq!(butchery_turn(&mut world, corpse, 0), ButcheryStep::Idle);
    assert_eq!(
        butchery_moves_left(&world, corpse),
        Some(107_999),
        "unchanged"
    );
}

#[test]
fn something_that_is_not_a_corpse_cannot_be_butchered() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (0, 0), ItemTypeId(1_000), 1);
    assert_eq!(
        start_butchery(&mut world, rock, state(ButcherAction::Quick)),
        None
    );
    assert_eq!(butchery_turn(&mut world, rock, 100), ButcheryStep::Idle);
}

#[test]
fn finishing_is_reported_once_and_not_twice() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let total = start_butchery(&mut world, corpse, state(ButcherAction::Quick)).expect("starts");
    assert_eq!(
        butchery_turn(&mut world, corpse, total),
        ButcheryStep::Finished
    );
    assert_eq!(
        butchery_turn(&mut world, corpse, total),
        ButcheryStep::Idle,
        "the activity is gone, so there is nothing to finish twice"
    );
}
