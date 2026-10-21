//! Items in a fire: how many go, and what the fire gets out of them.
//!
//! Transcribed from the item loop in `field_processor_fd_fire` (`src/map_field.cpp:980-1031`).

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    BurnDataView, Container, GroundItem, ItemBurnPlan, ItemBurnTotals, ItemFireRules, ItemTypeId,
    MaterialPortion, burn_items_in_fire, contents, drop_item, insert_into, items_at,
};

fn level(fuel: f64, burn: f64) -> BurnDataView {
    BurnDataView {
        immune: false,
        volume_per_turn: 0.0,
        fuel,
        smoke: 1.0,
        burn,
    }
}

fn wooden(volume_ml: f64, by_charges: bool) -> ItemBurnPlan {
    ItemBurnPlan {
        materials: vec![MaterialPortion {
            portions: 1,
            levels: [level(1.0, 3.0); 3],
        }],
        volume_ml,
        liquid: false,
        material_total: 1,
        by_charges,
        stack_size: 1,
    }
}

#[test]
fn a_small_item_is_finished_and_leaves_the_square() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(7), 1);
    let mut rolls = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        3,
        false,
        &ItemFireRules {
            plan_of: |_| Some(wooden(250.0, false)),
            irremovable: |_| false,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(totals.consumed, 1, "one item gone");
    assert!(items_at(&mut world, (5, 5)).is_empty(), "and off the map");
    assert!(totals.fuel > 0.0, "the fire was fed");
}

#[test]
fn a_fire_eats_at_most_two_items_per_point_of_intensity() {
    let mut world = World::new();
    for _ in 0..10 {
        drop_item(&mut world, (5, 5), ItemTypeId(7), 1);
    }
    let mut rolls = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        2,
        false,
        &ItemFireRules {
            plan_of: |_| Some(wooden(250.0, false)),
            irremovable: |_| false,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(totals.consumed, 4, "intensity 2 eats four");
    assert_eq!(items_at(&mut world, (5, 5)).len(), 6, "and leaves the rest");
}

#[test]
fn a_big_item_survives_the_turn_but_remembers_burning() {
    let mut world = World::new();
    let entity = drop_item(&mut world, (5, 5), ItemTypeId(7), 1);
    let mut rolls = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        1,
        false,
        &ItemFireRules {
            plan_of: |_| Some(wooden(10_000.0, false)),
            irremovable: |_| false,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(totals.consumed, 0, "ten litres does not vanish in one turn");
    let burnt = world.get::<GroundItem>(entity).map_or(0, |item| item.burnt);
    assert_eq!(
        burnt, 3,
        "but it has burned three measures of the hundred and twenty it needs"
    );
}

#[test]
fn an_item_with_no_burn_plan_is_left_alone() {
    let mut world = World::new();
    let entity = drop_item(&mut world, (5, 5), ItemTypeId(7), 5);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        3,
        false,
        &ItemFireRules {
            plan_of: |_| None,
            irremovable: |_| false,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(
        totals,
        ItemBurnTotals {
            smoke: 0.0,
            fuel: 0.0,
            consumed: 0,
            detonated: 0
        }
    );
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "and no dice are drawn"
    );
    assert_eq!(
        world.get::<GroundItem>(entity).map(|item| item.charges),
        Some(5)
    );
}

#[test]
fn a_stack_loses_charges_rather_than_vanishing() {
    let mut world = World::new();
    let entity = drop_item(&mut world, (5, 5), ItemTypeId(7), 100);
    let mut rolls = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        1,
        false,
        &ItemFireRules {
            plan_of: |_| {
                Some(ItemBurnPlan {
                    materials: vec![MaterialPortion {
                        portions: 1,
                        levels: [level(1.0, 0.03); 3],
                    }],
                    volume_ml: 250.0,
                    liquid: false,
                    material_total: 1,
                    by_charges: true,
                    stack_size: 1,
                })
            },
            irremovable: |_| false,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(totals.consumed, 0);
    let charges = world
        .get::<GroundItem>(entity)
        .map_or(0, |item| item.charges);
    assert!(
        (99..=100).contains(&charges),
        "at most one charge lost, got {charges}"
    );
}

#[test]
fn a_fireproof_item_sits_there_through_the_whole_thing() {
    let mut world = World::new();
    let entity = drop_item(&mut world, (5, 5), ItemTypeId(7), 3);
    let mut plan = wooden(250.0, true);
    for material in &mut plan.materials {
        for level in &mut material.levels {
            level.immune = true;
        }
    }
    let mut rolls = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        3,
        false,
        &ItemFireRules {
            plan_of: |_| Some(plan.clone()),
            irremovable: |_| false,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(
        totals,
        ItemBurnTotals {
            smoke: 0.0,
            fuel: 0.0,
            consumed: 0,
            detonated: 0
        }
    );
    assert_eq!(
        world.get::<GroundItem>(entity).map(|item| item.charges),
        Some(3)
    );
}

#[test]
fn a_container_that_burns_away_spills_what_was_inside_it() {
    let mut world = World::new();
    let wooden_box = drop_item(&mut world, (5, 5), ItemTypeId(7), 1);
    world
        .entity_mut(wooden_box)
        .insert(Container { capacity_ml: 1000 });
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(3), 1);
    insert_into(&mut world, wooden_box, rock, |_| 100).expect("the rock fits");
    let mut rolls = Rolls::new(1);
    // Only the box burns: the rock's plan is unknown, so it is left alone.
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        3,
        false,
        &ItemFireRules {
            plan_of: |kind| {
                if kind == ItemTypeId(7) {
                    Some(wooden(250.0, false))
                } else {
                    None
                }
            },
            irremovable: |_| false,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(totals.consumed, 1, "the box is gone");
    assert!(
        items_at(&mut world, (5, 5)).contains(&rock),
        "the rock is on the floor"
    );
    assert!(contents(&mut world, wooden_box).is_empty());
}

#[test]
fn an_irremovable_item_stays_inside_a_container_that_burns_away() {
    let mut world = World::new();
    let box_entity = drop_item(&mut world, (5, 5), ItemTypeId(7), 1);
    world
        .entity_mut(box_entity)
        .insert(Container { capacity_ml: 1000 });
    let tablet = drop_item(&mut world, (5, 5), ItemTypeId(8), 1);
    insert_into(&mut world, box_entity, tablet, |_| 100).expect("it fits");
    let mut rolls = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        3,
        false,
        &ItemFireRules {
            plan_of: |kind| {
                if kind == ItemTypeId(7) {
                    Some(wooden(250.0, false))
                } else {
                    None
                }
            },
            irremovable: |held| held == tablet,
            explodes: |_| false,
        },
        &mut rolls,
    );
    assert_eq!(totals.consumed, 1, "the box burned");
    assert!(
        !items_at(&mut world, (5, 5)).contains(&tablet),
        "the tablet did not spill"
    );
    assert!(
        world.get::<GroundItem>(tablet).is_none(),
        "an irremovable item is erased with its container, as the reference does"
    );
}

#[test]
fn something_that_goes_off_in_a_fire_never_burns() {
    let mut world = World::new();
    let bomb = drop_item(&mut world, (5, 5), ItemTypeId(11), 1);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        3,
        false,
        &ItemFireRules {
            // Even with a plan that would burn it to nothing, detonation wins.
            plan_of: |_| Some(wooden(1.0, false)),
            irremovable: |_| false,
            explodes: |held| held == bomb,
        },
        &mut rolls,
    );
    assert_eq!(totals.detonated, 1, "it went off");
    assert_eq!(totals.consumed, 0, "and did not burn");
    assert!(
        totals.smoke.abs() < f64::EPSILON,
        "a detonation is not a fire"
    );
    assert!(
        items_at(&mut world, (5, 5)).is_empty(),
        "it left the square"
    );
    assert_eq!(
        rolls.rng(0, 1_000),
        control.rng(0, 1_000),
        "no dice were drawn: nothing burned"
    );
}

#[test]
fn a_detonation_spills_what_the_item_held() {
    let mut world = World::new();
    let bomb = drop_item(&mut world, (5, 5), ItemTypeId(11), 1);
    world
        .entity_mut(bomb)
        .insert(Container { capacity_ml: 1000 });
    let shrapnel = drop_item(&mut world, (5, 5), ItemTypeId(4), 1);
    insert_into(&mut world, bomb, shrapnel, |_| 10).expect("it fits");
    let mut rolls = Rolls::new(1);
    let totals = burn_items_in_fire(
        &mut world,
        (5, 5),
        1,
        false,
        &ItemFireRules {
            plan_of: |_| None,
            irremovable: |_| false,
            explodes: |held| held == bomb,
        },
        &mut rolls,
    );
    assert_eq!(totals.detonated, 1);
    assert!(
        items_at(&mut world, (5, 5)).contains(&shrapnel),
        "the contents are on the floor"
    );
}
