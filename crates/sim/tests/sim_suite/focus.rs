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
        world
            .entity_mut(survivor)
            .insert(cdda_sim::creature::Pain(pain));
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
    assert_eq!(
        cdda_sim::skills::sleepiness_cap(150, 0),
        150,
        "an awake creature has no ceiling"
    );
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 191), 80, "tired");
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 383), 60, "dead tired");
    assert_eq!(cdda_sim::skills::sleepiness_cap(150, 575), 40, "exhausted");
    assert_eq!(
        cdda_sim::skills::sleepiness_cap(150, 1000),
        20,
        "barely awake"
    );
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
    assert!(
        short > 0,
        "an hour of being awake leaves a creature sleepy, and it was {short}"
    );
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

/// A sleeping creature recovers, and it wakes when it has slept enough.
///
/// Reference: `Character::update_needs` (`character.cpp:5181-5193`) and `calc_sleep_recovery_rate` (`character.cpp:5321-5345`): the rate is one plus an accelerated
/// share, and a creature that would fall below minus twenty is woken instead.
#[test]
fn sleep_takes_sleepiness_away_and_wakes_a_creature_that_has_had_enough() {
    use bevy_ecs::prelude::*;
    assert!(
        cdda_sim::needs::recovery_rate(0) > 1.0,
        "sleep recovers faster than it accumulates"
    );
    assert!(
        cdda_sim::needs::recovery_rate(4) > cdda_sim::needs::recovery_rate(0),
        "and deeper sleep recovers faster still"
    );

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::needs::mod_sleepiness(&mut world, survivor, 400);
    let mut rolls = cdda_core_types::rng::Rolls::new(5);
    let mut turns = 0;
    while cdda_sim::needs::sleepiness_of(&world, survivor) > 0 && turns < 200 {
        let _ = cdda_sim::needs::sleep_off(
            &mut world,
            survivor,
            cdda_sim::needs::recovery_rate(0),
            5,
            &mut rolls,
        );
        turns += 1;
    }
    assert!(
        cdda_sim::needs::sleepiness_of(&world, survivor) <= 0,
        "a night's sleep takes four hundred points away, and it took {turns} spans"
    );

    // On down past the bottom, the reference wakes the creature and takes twenty-five.
    cdda_sim::needs::mod_sleepiness(&mut world, survivor, -15);
    let before = cdda_sim::needs::sleepiness_of(&world, survivor);
    let outcome = cdda_sim::needs::sleep_off(
        &mut world,
        survivor,
        cdda_sim::needs::recovery_rate(0),
        5,
        &mut rolls,
    );
    assert_eq!(outcome, cdda_sim::needs::Slept::Woke);
    assert_eq!(
        cdda_sim::needs::sleepiness_of(&world, survivor),
        before - 25,
        "the waking costs twenty-five from wherever it stood"
    );
}

/// A creature that a mutation says sleeps less really does get sleepy more slowly.
///
/// Reference: `Character::calc_needs_rates` (`character.cpp:5313-5316`) passes hunger, sleepiness and thirst through the enchantment cache, so the cache is not a
/// description of a creature - it is one of the numbers its clock runs on. The port read the stored rates straight into `update_needs` until this test.
#[test]
fn a_sleepiness_enchantment_slows_the_clock() {
    use bevy_ecs::prelude::*;

    let after_an_hour = |halved: bool| {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        world
            .entity_mut(survivor)
            .insert(cdda_sim::needs::NeedsRatesOf(cdda_sim::needs::NeedsRates {
                kcal: 2_400.0,
                hunger: 1.0,
                thirst: 1.0,
                sleepiness: 1.0,
            }));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(17));
        if halved {
            let text = "{\"mutations\":[{\"id\":\"LIGHT_SLEEPER\",\"valid\":true,\
                \"enchant_values\":[{\"value\":\"SLEEPINESS\",\"add\":0,\"multiply\":-0.5}]}]}";
            let Ok(root) = cdda_json::parse(text.as_bytes()) else {
                return 0;
            };
            let Some((data, _)) = cdda_content::mutations::from_dump(&root) else {
                return 0;
            };
            let categories = cdda_content::mutations::Categories::default();
            cdda_sim::mutation_data::install(&mut world, data.clone(), categories.clone());
            cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "LIGHT_SLEEPER");
        }
        for minute in 0..=60 {
            cdda_sim::calendar::set_turn(&mut world, minute * 60);
            cdda_sim::turn::run_turn(&mut world);
        }
        cdda_sim::needs::sleepiness_of(&world, survivor)
    };

    let plain = after_an_hour(false);
    let halved = after_an_hour(true);
    assert!(plain > 0, "an hour awake makes a creature sleepy: {plain}");
    assert!(
        halved * 2 <= plain + 2,
        "and half the rate is about half the sleepiness: {halved} against {plain}"
    );
}
