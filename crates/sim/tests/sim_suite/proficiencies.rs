//! Learning a proficiency by practising it.
//!
//! Reference: `proficiency_set::practice` (`proficiency.cpp:306-345`).

use bevy_ecs::prelude::*;

/// Practice adds up, and the day it reaches the time it takes, the proficiency is known.
///
/// Reference: `proficiency_set::practice` (`proficiency.cpp:306-345`): the practised time is the sum of what has been practised, and learning **removes** the
/// progress - `learn` is followed by the erase, so a known proficiency has nothing practised toward it.
#[test]
fn practice_adds_up_until_the_proficiency_is_learned() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    // Eight hours to learn it, and a craft that practised an hour.
    let eight_hours = 8 * 60 * 60;
    assert!(!cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 60 * 60, eight_hours));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_tailor"),
        60 * 60,
        "an hour of practice is an hour"
    );
    assert!(!cdda_sim::proficiencies::knows(&world, survivor, "prof_tailor"));

    // Seven more hours reaches the eight, and that practice is the one that teaches it.
    assert!(cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 7 * 60 * 60, eight_hours));
    assert!(cdda_sim::proficiencies::knows(&world, survivor, "prof_tailor"));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_tailor"),
        0,
        "a learned proficiency has no progress left: the reference erases it"
    );

    // Practising something already known changes nothing and learns nothing again.
    assert!(!cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 60 * 60, eight_hours));
    assert!(!cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 0, eight_hours));
}

/// A craft's time is **divided** between the proficiencies it teaches.
///
/// Reference: `craft_proficiency_gain` (`crafting.cpp:1036-1090`): the subjects are the proficiencies the crafter does not know yet, the craft's time is divided
/// between them (`time / subjects.size()`), and each gets that share times `learning_time_mult / time_multiplier`.
#[test]
fn a_craft_divides_its_time_between_what_it_teaches() {
    let mut world = World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let eight_hours = 8 * 60 * 60;
    let teaches = [
        cdda_sim::proficiencies::Taught {
            id: "prof_tailor",
            time_multiplier: 1.0,
            learning_time_mult: 1.0,
            time_to_learn: eight_hours,
        },
        cdda_sim::proficiencies::Taught {
            id: "prof_knitting",
            time_multiplier: 1.0,
            learning_time_mult: 1.0,
            time_to_learn: eight_hours,
        },
    ];

    // Eight hours of work split two ways is four hours each, which is not enough to learn either.
    assert!(!cdda_sim::proficiencies::gain_from_craft(
        &mut world,
        crafter,
        &teaches,
        eight_hours,
        &|_| true
    ));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_tailor"),
        4 * 60 * 60
    );
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_knitting"),
        4 * 60 * 60
    );

    // Eight more hours is another four each, and both are learned by it.
    assert!(cdda_sim::proficiencies::gain_from_craft(
        &mut world,
        crafter,
        &teaches,
        eight_hours,
        &|_| true
    ));
    assert!(cdda_sim::proficiencies::knows(&world, crafter, "prof_tailor"));
    assert!(cdda_sim::proficiencies::knows(&world, crafter, "prof_knitting"));

    // A proficiency the crafter cannot learn is not practised and does not take a share of the time.
    let refused = [cdda_sim::proficiencies::Taught {
        id: "prof_gun_smith",
        time_multiplier: 1.0,
        learning_time_mult: 1.0,
        time_to_learn: eight_hours,
    }];
    assert!(!cdda_sim::proficiencies::gain_from_craft(
        &mut world,
        crafter,
        &refused,
        60 * 60,
        &|_| false
    ));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_gun_smith"),
        0,
        "a proficiency with prerequisites it does not meet is not practised at all"
    );
}
