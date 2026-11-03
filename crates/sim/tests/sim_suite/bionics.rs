//! The machinery a creature carries inside it.
//!
//! Reference: `Character::has_bionic` (`character.cpp`) and `f_has_bionics` (`condition.cpp:946-949`). Until this file the two rules were exercised only through a
//! dialogue effect that installs a bionic and a condition that asks about it.

use bevy_ecs::prelude::*;

/// Installing is idempotent, and the three questions about a list answer differently.
///
/// Reference: `Character::add_bionic` refuses a bionic already installed, `has_bionic` asks about one, and `f_has_bionics` adds the `ANY` form the data uses for
/// "has any bionic at all".
#[test]
fn a_bionic_is_installed_once_and_asked_for_three_ways() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    assert!(!cdda_sim::bionics::has(&world, survivor, "ANY"), "a creature starts with nothing inside it");
    assert!(!cdda_sim::bionics::has(&world, survivor, "bio_power_storage"));
    assert!(cdda_sim::bionics::bionics_of(&world, survivor).is_empty());

    cdda_sim::bionics::install(&mut world, survivor, "bio_power_storage");
    cdda_sim::bionics::install(&mut world, survivor, "bio_power_storage");
    assert_eq!(
        cdda_sim::bionics::bionics_of(&world, survivor),
        vec!["bio_power_storage".to_owned()],
        "installing the same bionic twice puts it in the list once"
    );
    assert!(cdda_sim::bionics::has(&world, survivor, "bio_power_storage"));
    assert!(!cdda_sim::bionics::has(&world, survivor, "bio_synlungs"));
    assert!(
        cdda_sim::bionics::has(&world, survivor, "ANY"),
        "ANY holds once there is one inside"
    );
}

/// A creature that has never been touched answers the questions without needing a component.
///
/// Reference: `Character`'s bionic list is always there and empty; the port's component appears when something is installed, so every reader must answer for its
/// absence - which is what makes a zombie and a survivor with no implants behave alike.
#[test]
fn a_creature_with_no_implants_answers_for_its_absence() {
    let mut world = World::new();
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (0, 0, 0));
    assert!(
        world.get::<cdda_sim::bionics::Bionics>(zombie).is_none(),
        "nothing is installed, so there is nothing to store"
    );
    assert!(!cdda_sim::bionics::has(&world, zombie, "bio_power_storage"));
    assert!(!cdda_sim::bionics::has(&world, zombie, "ANY"));
    assert!(cdda_sim::bionics::bionics_of(&world, zombie).is_empty());
}
