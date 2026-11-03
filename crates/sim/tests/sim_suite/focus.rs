//! The morale a mind actually rests at, once the body is in pain.
//!
//! Reference: `Character::calc_focus_equilibrium` (`character.cpp:3852-3920`) and `Character::update_mental_focus` (`character.cpp:3929-3934`).

/// Pain is taken off morale, and a masochist only minds it past twenty.
///
/// Reference: `character.cpp:3872-3880`: a plain creature subtracts the whole perceived pain; a masochist subtracts `pain - 20` and only when that is positive.
#[test]
fn pain_is_taken_off_morale_and_a_masochist_minds_only_the_excess() {
    assert_eq!(cdda_sim::skills::effective_morale(60, 0, false), 60);
    assert_eq!(
        cdda_sim::skills::effective_morale(60, 25, false),
        35,
        "a plain creature loses the whole pain"
    );
    assert_eq!(
        cdda_sim::skills::effective_morale(60, 20, true),
        60,
        "a masochist feels nothing up to twenty"
    );
    assert_eq!(
        cdda_sim::skills::effective_morale(60, 45, true),
        35,
        "and only the part past twenty above it"
    );
}

/// A painful creature's focus drifts towards a lower equilibrium, and the turn stage is what does it.
///
/// Reference: `Character::update_mental_focus` adds a tenth of the gap each minute, so a creature in pain settles below a comfortable one, and the stage that calls it is
/// `reset_stats`'s minute tick.
#[test]
fn a_creature_in_pain_settles_at_a_lower_focus() {
    use bevy_ecs::prelude::*;

    let settled_focus = |pain: i32| {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        world.entity_mut(survivor).insert(cdda_sim::creature::Pain(pain));
        // The reference's `once_every( 1_minutes )` is a minute of game time, so the calendar has to be there for the stage to run at all.
        for minute in 0..=60 {
            cdda_sim::calendar::set_turn(&mut world, minute * 60);
            cdda_sim::turn::run_turn(&mut world);
        }
        world
            .get::<cdda_sim::skills::Focus>(survivor)
            .map_or(0, |focus| focus.0)
    };
    let comfortable = settled_focus(0);
    let hurting = settled_focus(50);
    assert!(
        hurting < comfortable,
        "a creature at fifty pain settles below one at none: {hurting} against {comfortable}"
    );
    assert!(hurting > 0, "and not at nothing: {hurting}");
}

/// A tired mind settles lower, and the ceiling steps down with each sleepiness level.
///
/// Reference: `Character::focus_equilibrium_sleepiness_cap` (`character.cpp:3838-3850`) against `sleepiness_levels` (`character.h:151-156`): 191 caps at eighty, 383 at
/// sixty, 575 at forty and 1000 at twenty.
#[test]
fn a_tired_creature_cannot_settle_above_its_ceiling() {
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 0), 150, "an awake creature has no ceiling");
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 191), 80, "tired");
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 383), 60, "dead tired");
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 575), 40, "exhausted");
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 1000), 20, "barely awake");
    assert_eq!(
        cdda_sim::skills::sleepiness_cap(50, 1000),
        20,
        "and it caps what is above it rather than sparing it"
    );
    assert_eq!(
        cdda_sim::skills::sleepiness_cap(10, 1000),
        10,
        "while an equilibrium already below the ceiling is left alone"
    );
}

/// A creature's sleepiness climbs with the clock and stops at the cap.
///
/// Reference: `Character::update_needs` (`character.cpp:5155-5175`): `roll_remainder( rates.sleepiness * rate_multiplier )` each five minutes, only while awake and
/// below 1050.
#[test]
fn sleepiness_climbs_with_the_clock_and_stops_at_the_cap() {
    use bevy_ecs::prelude::*;

    let sleepiness_after = |minutes: i64| {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        world
            .entity_mut(survivor)
            .insert(cdda_sim::needs::NeedsRatesOf(cdda_sim::needs::NeedsRates {
                kcal: 0.0,
                hunger: 1.0,
                thirst: 1.0,
                sleepiness: 1.0,
            }));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
        for step in 0..=minutes / 5 {
            cdda_sim::calendar::set_turn(&mut world, step * 300);
            cdda_sim::turn::run_turn(&mut world);
        }
        cdda_sim::needs::sleepiness_of(&world, survivor)
    };
    let short = sleepiness_after(60);
    assert!(short > 0, "an hour of being awake leaves a creature sleepy, and it was {short}");
    assert!(
        (55..=65).contains(&short),
        "five a step for twelve steps is about sixty, and it was {short}"
    );
    // Five points every five minutes reaches the cap of 1050 after about seventeen hours awake.
    assert_eq!(
        sleepiness_after(1_200),
        cdda_sim::needs::MAX_SLEEPINESS,
        "and enough time at the cap leaves it there"
    );
}

/// A creature with no needs clock, or one that is asleep, does not climb.
///
/// Reference: the `!asleep && !debug_ls` guard in `update_needs` (`character.cpp:5159`).
#[test]
fn a_sleeping_creature_or_one_with_no_needs_does_not_climb() {
    use bevy_ecs::prelude::*;
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let rates = cdda_sim::needs::NeedsRates {
        kcal: 0.0,
        hunger: 1.0,
        thirst: 1.0,
        sleepiness: 1.0,
    };
    let mut rolls = cdda_core_types::rng::Rolls::new(1);
    cdda_sim::needs::climb_sleepiness(&mut world, survivor, rates, 5, false, false, &mut rolls);
    assert_eq!(
        cdda_sim::needs::sleepiness_of(&world, survivor),
        0,
        "a sleeping creature does not get sleeper"
    );
    cdda_sim::needs::climb_sleepiness(&mut world, survivor, rates, 5, true, true, &mut rolls);
    assert_eq!(
        cdda_sim::needs::sleepiness_of(&world, survivor),
        0,
        "and neither does one with no needs at all"
    );
}
