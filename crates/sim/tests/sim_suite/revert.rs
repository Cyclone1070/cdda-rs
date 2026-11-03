//! A place can be told to remember itself, and put itself back later.
//!
//! Reference: `f_revert_location` (`npctalk.cpp:4884-4918`) copies the four submaps around a place with `submap::get_revert_submap`
//! (`submap.cpp:352-362`) and schedules a `REVERT_SUBMAP` that copies the tiles back (`submap::revert_submap`, `submap.cpp:322-347`). The copy is taken
//! **when the effect runs**, so a change made after it is undone by the revert and a change made after the revert is not.

use bevy_ecs::prelude::*;

/// A two-by-two level of one terrain code, with a piece of furniture on the far corner.
fn level() -> cdda_core_types::level::Level {
    let mut level = cdda_core_types::level::Level::new();
    for y in 0..2 {
        for x in 0..2 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    level
}

#[test]
fn a_remembered_square_is_put_back_the_way_it_was() {
    let mut world = World::new();
    cdda_sim::load_level(&mut world, &level());
    cdda_sim::square::set_furniture(&mut world, (1, 1, 0), 7);
    assert_eq!(cdda_sim::square::terrain_at(&world, (1, 1, 0)), Some(1), "the floor is a floor");

    let total = cdda_sim::square::squares(&world).len();
    let remembered = cdda_sim::revert::take(&mut world, (0, 0, 0));
    assert_eq!(remembered, total, "every square of the level is within one revert's reach");

    // A mapgen update changes the house: the floor becomes rubble and the counter is taken away.
    cdda_sim::square::set_terrain(&mut world, (1, 1, 0), 2);
    cdda_sim::square::set_furniture(&mut world, (1, 1, 0), 0);
    assert_eq!(cdda_sim::square::terrain_at(&world, (1, 1, 0)), Some(2), "the change landed");

    let restored = cdda_sim::revert::restore(&mut world);
    assert_eq!(restored, total, "and every square is put back");
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(1),
        "the floor is a floor again"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (1, 1, 0)),
        Some(7),
        "and the counter the update removed is back"
    );
    assert!(
        !cdda_sim::revert::has_snapshot(&world),
        "with nothing left waiting to be put back"
    );
}

#[test]
fn a_square_outside_the_reach_is_not_remembered() {
    let mut world = World::new();
    // Two submaps along the x axis, so the level reaches past the revert's twelve-square radius.
    let mut far = cdda_core_types::level::Level::new();
    let beyond = cdda_sim::revert::REVERT_RADIUS * 2;
    far.set_terrain(beyond, 0, 1, 0);
    cdda_sim::load_level(&mut world, &far);
    let total = cdda_sim::square::squares(&world).len();
    let remembered = cdda_sim::revert::take(&mut world, (0, 0, 0));
    assert!(
        remembered < total,
        "a square further than the reach is left alone: {remembered} of {total}"
    );
}
