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
