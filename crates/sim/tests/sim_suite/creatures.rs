//! A creature is an entity standing on a square, like everything else.
//!
//! Reference: `Creature` (`src/creature.h`) and its two subclasses are a class hierarchy with a
//! virtual `move`; the port does not take the hierarchy, only what a creature *has*: a place, a body,
//! a pool of action points, and the name it is known by. Everything that happens to a creature is a
//! system that reads those components, and nothing about it lives in a side table.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::{
    Creature, Moves, StepOutcome, bank_a_turn, combined_move_cost, creature_at, give_moves,
    run_cost, scaled_speed, set_speed, spawn_creature, try_step,
};
use cdda_sim::{Position, singletons};

#[test]
fn a_creature_stands_on_a_square_and_the_index_knows_it() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));

    assert_eq!(
        creature_at(&world, (5, 5, 0)),
        vec![you],
        "the creature is found where it stands"
    );
    assert!(
        world
            .get::<Position>(you)
            .is_some_and(|position| (position.x, position.y, position.z) == (5, 5, 0)),
        "and it carries the square rather than being filed under it"
    );
    assert!(
        singletons::index(&world).is_some_and(|index| index.at_level(5, 5, 0).contains(&you)),
        "the index is built from the store, so a creature in the store is in the index"
    );
}

#[test]
fn action_points_are_the_creature_s_own() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));
    let other = spawn_creature(&mut world, "a dog", (6, 5, 0));

    give_moves(&mut world, you, 100);
    assert_eq!(moves_of(&world, you), 100, "the points were given to it");
    assert_eq!(moves_of(&world, other), 0, "and not to the dog beside it");
}

#[test]
fn a_creature_carries_its_own_name_and_place_in_the_world() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));
    let dog = spawn_creature(&mut world, "a dog", (5, 5, 0));

    assert_eq!(
        creature_at(&world, (5, 5, 0)).len(),
        2,
        "two creatures can stand on one square"
    );
    let names: Vec<String> = [you, dog]
        .iter()
        .filter_map(|entity| world.get::<Creature>(*entity).map(|one| one.name.clone()))
        .collect();
    assert_eq!(
        names,
        vec!["the survivor", "a dog"],
        "each keeps its own name"
    );
}

#[test]
fn a_creature_banks_its_speed_in_action_points_each_turn() {
    let mut world = World::new();
    let quick = spawn_creature(&mut world, "a runner", (5, 5, 0));
    let slow = spawn_creature(&mut world, "a shambler", (6, 5, 0));
    set_speed(&mut world, quick, 100, 0);
    set_speed(&mut world, slow, 50, 0);

    bank_a_turn(&mut world);

    assert_eq!(
        moves_of(&world, quick),
        100,
        "the quick one banks its whole speed"
    );
    assert_eq!(moves_of(&world, slow), 50, "the slow one banks half of it");

    bank_a_turn(&mut world);
    assert_eq!(
        moves_of(&world, quick),
        200,
        "and points accumulate while unspent"
    );
}

/// Speed as the reference scales it: `mon.speed *= MONSTER_SPEED / 100.0` on an `int`
/// (`monstergenerator.cpp:346`), so the product truncates toward zero.
#[test]
fn the_monster_speed_option_scales_speed_and_truncates() {
    assert_eq!(
        scaled_speed(100, 100),
        100,
        "a hundred percent is unchanged"
    );
    assert_eq!(scaled_speed(100, 150), 150, "a half again");
    assert_eq!(scaled_speed(100, 33), 33, "a third, truncated");
    assert_eq!(scaled_speed(130, 75), 97, "97.5 truncates to 97");
    assert_eq!(
        scaled_speed(70, 100),
        70,
        "and a speed that needs no scaling keeps its value"
    );
}

/// What a step costs, as the reference works it out.
///
/// Reference: `Character::run_cost` (`src/character.cpp:10363-10374`): the cost is divided by the
/// square root of two on a diagonal, effects are applied, and it is multiplied by the root again
/// before the result is truncated to an integer. With no effects the round trip is *almost* the
/// identity, and the truncation is what makes it not quite: a hundred divides to 70.7106781 and comes
/// back as 99.9999999, which is 99. That one-point difference is the reference's behaviour, so it is
/// what the test pins - writing the "obvious" identity here would have quietly corrected upstream.
#[test]
fn a_diagonal_step_round_trips_through_the_root_of_two_and_truncates() {
    for cost in (100..=400).step_by(50) {
        let straight = run_cost(cost, false);
        let diagonal = run_cost(cost, true);
        assert_eq!(straight, cost, "a straight step is the square's cost");
        assert!(
            diagonal <= straight,
            "a diagonal is never dearer than a straight step"
        );
        assert!(
            straight - diagonal <= 1,
            "and never more than a point cheaper: {cost} came back as {diagonal}"
        );
    }
    assert_eq!(
        run_cost(100, true),
        99,
        "a hundred comes back as ninety-nine"
    );
}

#[test]
fn a_creature_moves_when_it_can_pay_for_the_step() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));
    give_moves(&mut world, you, 100);

    let outcome = try_step(&mut world, you, (6, 5, 0), 100);

    assert_eq!(
        outcome,
        StepOutcome::Moved,
        "a hundred moves buys a hundred-move step"
    );
    assert_eq!(
        creature_at(&world, (6, 5, 0)),
        vec![you],
        "it stands where it stepped"
    );
    assert!(
        creature_at(&world, (5, 5, 0)).is_empty(),
        "and no longer where it was: the index learned about it"
    );
    assert_eq!(moves_of(&world, you), 0, "the step spent every point");
}

#[test]
fn a_creature_without_the_points_stays_where_it_is() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));
    give_moves(&mut world, you, 99);

    let outcome = try_step(&mut world, you, (6, 5, 0), 100);

    assert_eq!(
        outcome,
        StepOutcome::TooSlow,
        "ninety-nine cannot pay for a hundred"
    );
    assert_eq!(creature_at(&world, (5, 5, 0)), vec![you], "it did not move");
    assert_eq!(
        moves_of(&world, you),
        99,
        "and it was not charged for trying"
    );
}

/// What entering a square costs, which is the map's answer and not the creature's.
///
/// Reference: `map::combined_movecost` (`src/map.cpp:2466-2488`). Both squares are charged for, the
/// two are added with the caller's modifier, and the sum is scaled by how many axes differ:
/// `{ 0, 50, 71, 100 }` divided by two. So a floor costs 2 to stand on, and a step between two floors
/// is `(2 + 2) * 50 / 2 = 100` moves - one turn - while a diagonal is `(2 + 2) * 71 / 2 = 142`. That
/// settles the unit question the last round left open: the JSON's `move_cost` is a hundredth of a turn
/// per point, and 71 is the integer stand-in for a hundred over the square root of two.
#[test]
fn a_step_costs_both_squares_scaled_by_how_far_it_goes() {
    assert_eq!(combined_move_cost(2, 2, 0, 1), 100, "floor to floor");
    assert_eq!(
        combined_move_cost(2, 2, 0, 2),
        142,
        "floor to floor, diagonally"
    );
    assert_eq!(
        combined_move_cost(2, 2, 0, 3),
        200,
        "and up or down a level too"
    );
    assert_eq!(
        combined_move_cost(3, 2, 0, 1),
        125,
        "a rougher square costs more"
    );
    assert_eq!(
        combined_move_cost(3, 2, 0, 2),
        177,
        "177.5 truncates, because every factor here is an integer"
    );
    assert_eq!(
        combined_move_cost(2, 2, 10, 1),
        350,
        "the caller's modifier adds to both"
    );
}

/// The action points a creature has left.
fn moves_of(world: &World, entity: Entity) -> i32 {
    world.get::<Moves>(entity).map_or(0, |moves| moves.left)
}
