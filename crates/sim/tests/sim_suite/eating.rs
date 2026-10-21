//! Eating one serving, and what it spends.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{EatError, GroundItem, ItemTypeId, MealTally, count_on_square, drop_item, eat};

/// meat: 46 nutr, no quench, one charge a serving.
/// soup: 30 nutr, 20 quench, five charges a serving.
fn nutrition(kind: ItemTypeId) -> (i32, i32, i32) {
    match kind.0 {
        1 => (46, 0, 1),
        2 => (30, 20, 5),
        _ => (0, 0, 0),
    }
}

#[test]
fn a_bite_of_food_costs_one_charge_and_feeds() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 4);
    let meal = eat(&mut world, (5, 5), ItemTypeId(1), nutrition).expect("there is meat");
    assert_eq!(meal.nutr, 46);
    assert_eq!(meal.charges, 1);
    assert_eq!(
        count_on_square(&mut world, (5, 5), ItemTypeId(1)),
        3,
        "three left"
    );
}

#[test]
fn a_serving_of_soup_costs_five_charges() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(2), 12);
    let meal = eat(&mut world, (5, 5), ItemTypeId(2), nutrition).expect("there is soup");
    assert_eq!(meal.charges, 5, "one serving is five charges");
    assert_eq!(count_on_square(&mut world, (5, 5), ItemTypeId(2)), 7);
    assert_eq!(meal.quench, 20);
}

#[test]
fn the_last_serving_empties_the_square() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    assert!(eat(&mut world, (5, 5), ItemTypeId(1), nutrition).is_ok());
    assert!(
        world.query::<&GroundItem>().iter(&world).next().is_none(),
        "nothing left"
    );
}

#[test]
fn a_partial_serving_is_not_a_serving() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(2), 3);
    let error = eat(&mut world, (5, 5), ItemTypeId(2), nutrition).expect_err("three is not five");
    assert_eq!(
        error,
        EatError::NotEnoughCharges {
            available: 3,
            needed: 5
        }
    );
    assert_eq!(
        count_on_square(&mut world, (5, 5), ItemTypeId(2)),
        3,
        "nothing was spent"
    );
}

#[test]
fn a_rock_is_not_food() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(9), 1);
    assert_eq!(
        eat(&mut world, (5, 5), ItemTypeId(9), nutrition),
        Err(EatError::NotFood)
    );
}

#[test]
fn an_empty_square_has_nothing_to_eat() {
    let mut world = World::new();
    assert_eq!(
        eat(&mut world, (5, 5), ItemTypeId(1), nutrition),
        Err(EatError::NotThere)
    );
}

#[test]
fn the_tally_adds_up_over_a_meal() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 2);
    drop_item(&mut world, (5, 5), ItemTypeId(2), 5);
    let mut tally = MealTally::default();
    tally.eat(eat(&mut world, (5, 5), ItemTypeId(1), nutrition).expect("meat"));
    tally.eat(eat(&mut world, (5, 5), ItemTypeId(2), nutrition).expect("soup"));
    assert_eq!(
        tally,
        MealTally {
            nutr: 76,
            quench: 20
        }
    );
}
