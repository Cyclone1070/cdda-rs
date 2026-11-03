//! What a creature is feeling, how it stacks, and how it fades.
//!
//! Reference: `player_morale` (`morale.cpp:130-240`). Until this file the rules were exercised only through dialogue effects that happen to add morale, so a
//! change to the stacking curve or the fade could have gone unnoticed.

use bevy_ecs::prelude::*;

fn point(kind: &str, bonus: i32) -> cdda_sim::morale::MoralePoint {
    cdda_sim::morale::MoralePoint {
        kind: kind.to_owned(),
        bonus,
        max_bonus: 0,
        duration: 0,
        decay_start: 0,
        age: 0,
        capped: false,
    }
}

/// Two good things stack by the square root of the sum of their squares, and two of opposite sign add up plainly.
///
/// Reference: `player_morale::add` (`morale.cpp:160-200`): the same-sign bonus is `sqrt( held^2 + new^2 )` - so three small good things never add up to one big one -
/// while a point of the **other** sign is simply added, because a bad thing does not dilute a good one.
#[test]
fn same_sign_morale_stacks_by_the_square_root_and_opposite_sign_adds() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    // Both points are **timed**, because a permanent one replaces the bonus instead of stacking with it - the reference's own first branch
    // (`morale.cpp:188-189`: `if( new_duration == 0_turns ) { sqrt_of_sum_of_squares = new_bonus; }`).
    let mut three = point("morale_food_good", 3);
    three.duration = 600;
    three.decay_start = 300;
    let mut four = point("morale_food_good", 4);
    four.duration = 600;
    four.decay_start = 300;
    cdda_sim::morale::add(&mut world, survivor, three);
    cdda_sim::morale::add(&mut world, survivor, four);
    assert_eq!(
        cdda_sim::morale::net_bonus_of(&world, survivor),
        5,
        "three and four of the same kind make five, not seven"
    );

    cdda_sim::morale::add(&mut world, survivor, point("morale_pain", -2));
    assert_eq!(
        cdda_sim::morale::net_bonus_of(&world, survivor),
        3,
        "the pain is taken off plainly: five less two"
    );
    assert_eq!(cdda_sim::morale::points_of(&world, survivor).len(), 2);
}

/// A point fades from its decay start and is dropped when its time is up.
///
/// Reference: `morale_point::get_net_bonus` (`morale.cpp:136-143`), which scales the bonus along `logarithmic_range` between `decay_start` and `duration`, and
/// `player_morale::decay` (`morale.cpp`), which erases the points that have run out.
#[test]
fn morale_fades_and_then_runs_out() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let mut fading = point("morale_food_good", 20);
    fading.duration = 100;
    fading.decay_start = 50;
    cdda_sim::morale::add(&mut world, survivor, fading);

    assert_eq!(
        cdda_sim::morale::net_bonus_of(&world, survivor),
        20,
        "before the decay starts the point is worth all of itself"
    );
    for _ in 0..75 {
        cdda_sim::morale::age_morale(&mut world);
    }
    let faded = cdda_sim::morale::net_bonus_of(&world, survivor);
    assert!(
        (0..20).contains(&faded),
        "halfway through the fade the point is worth less than its bonus and more than nothing, and it was {faded}"
    );
    for _ in 0..30 {
        cdda_sim::morale::age_morale(&mut world);
    }
    assert!(
        cdda_sim::morale::points_of(&world, survivor).is_empty(),
        "a point whose time is up is dropped, not left at zero"
    );
    assert_eq!(cdda_sim::morale::net_bonus_of(&world, survivor), 0);
}

/// Stacking a new point keeps the longer of the two times, and a capped point takes the new time outright.
///
/// Reference: `morale_point::pick_time` (`morale.cpp:206-211`) and the capped branch of `player_morale::add`: a capped point **replaces** the duration instead of
/// choosing between them, because its cap is what it is worth.
#[test]
fn stacking_keeps_the_longer_time_unless_the_point_is_capped() {
    assert_eq!(
        cdda_sim::morale::pick_time(100, 10, 200, true),
        200,
        "the longer of what is left and what is new"
    );
    assert_eq!(
        cdda_sim::morale::pick_time(100, 10, 40, true),
        90,
        "a shorter new time does not cut the one already running"
    );
    assert_eq!(
        cdda_sim::morale::pick_time(100, 10, 200, false),
        90,
        "and when the signs disagree only what is left survives"
    );

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let mut first = point("morale_pain", -10);
    first.duration = 500;
    first.decay_start = 250;
    cdda_sim::morale::add(&mut world, survivor, first);
    let mut second = point("morale_pain", -10);
    second.duration = 40;
    second.decay_start = 20;
    second.capped = true;
    cdda_sim::morale::add(&mut world, survivor, second);
    let held = cdda_sim::morale::points_of(&world, survivor);
    assert_eq!(held.len(), 1);
    assert_eq!(
        held.first().map(|point| point.duration),
        Some(40),
        "a capped point takes the new time outright"
    );
}

/// A morale type can be taken away whole, and a type that is not felt is not a failure.
///
/// Reference: `Character::remove_morale`, reached by `f_lose_morale` (`npctalk.cpp:7197-7204`).
#[test]
fn a_morale_type_is_removed_whole() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::morale::add(&mut world, survivor, point("morale_food_good", 5));
    cdda_sim::morale::add(&mut world, survivor, point("morale_pain", -3));
    assert!(cdda_sim::morale::remove_kind(&mut world, survivor, "morale_pain"));
    assert_eq!(cdda_sim::morale::net_bonus_of(&world, survivor), 5);
    assert!(
        !cdda_sim::morale::remove_kind(&mut world, survivor, "morale_pain"),
        "removing what is not felt answers that there was nothing to remove"
    );
}

/// The fade curve starts at one and ends at zero, which is what makes a run-out point worth nothing.
///
/// Reference: `logarithmic_range` (`cata_utility.cpp:166-195`), clamped at both ends.
#[test]
fn the_fade_curve_runs_from_one_to_zero() {
    assert!((cdda_sim::morale::logarithmic_range(0, 100, 0) - 1.0).abs() < 1e-9);
    assert!(cdda_sim::morale::logarithmic_range(0, 100, 100).abs() < 1e-9);
    let middle = cdda_sim::morale::logarithmic_range(0, 100, 50);
    assert!(middle > 0.0 && middle < 1.0, "halfway is neither end: {middle}");
    // Past the end it stays at zero rather than going negative.
    assert!(cdda_sim::morale::logarithmic_range(0, 100, 150).abs() < 1e-9);
}
