//! A place can be told to remember itself, and put itself back later.
//!
//! Reference: `f_revert_location` (`npctalk.cpp:4884-4918`) copies the four submaps around a place with `submap::get_revert_submap`
//! (`submap.cpp:352-362`) and schedules a `REVERT_SUBMAP` that copies the tiles back (`submap::revert_submap`, `submap.cpp:322-347`). The copy is taken
//! **when the effect runs**, so a change made after it is undone by the revert and a change made after the revert is not.

// The data entries in these tests are literals: a parse that fails is a test that cannot run, and saying so with a message is the point.
#![allow(clippy::expect_used)]

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
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(1),
        "the floor is a floor"
    );

    let total = cdda_sim::square::squares(&world).len();
    let remembered = cdda_sim::revert::take(&mut world, (0, 0, 0));
    assert_eq!(
        remembered, total,
        "every square of the level is within one revert's reach"
    );

    // A mapgen update changes the house: the floor becomes rubble and the counter is taken away.
    cdda_sim::square::set_terrain(&mut world, (1, 1, 0), 2);
    cdda_sim::square::set_furniture(&mut world, (1, 1, 0), 0);
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(2),
        "the change landed"
    );

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

/// The data's `revert_location` remembers a place and puts it back when the event comes due.
///
/// Reference: `f_revert_location` (`npctalk.cpp:4884-4918`) with the `REVERT_SUBMAP` case in `timed_event.cpp:336-340`: the place variable holds the square, the copy is
/// taken when the effect runs, and the tiles go back when the event is due.
#[test]
fn the_data_can_revert_a_place() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    cdda_sim::calendar::set_turn(&mut world, 0);
    cdda_sim::load_level(&mut world, &level());
    cdda_sim::square::set_terrain(&mut world, (6, 6, 0), 1);
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (6, 6, 0));

    // The place variable the data writes: `{"global_val": "portal_dungeon"}`.
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, speaker, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        holder,
        "portal_dungeon",
        cdda_sim::dialogue_values::Value::Position { x: 6, y: 6, z: 0 },
    );

    let effect = cdda_content::dialogue::Effect::RevertLocation {
        scope: "global_val".to_owned(),
        name: "portal_dungeon".to_owned(),
        after: cdda_content::dialogue::EffectDuration::Turns(Some(5)),
        key: None,
    };
    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        speaker,
        None,
        std::slice::from_ref(&effect),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the effect runs"
    );
    assert!(
        cdda_sim::revert::has_snapshot(&world),
        "and the place is remembered"
    );
    assert!(
        cdda_sim::timed_events::text_of(&world, "REVERT_SUBMAP").is_some(),
        "with the event queued under the reference's own name"
    );

    // The house changes before the event comes due.
    cdda_sim::square::set_terrain(&mut world, (6, 6, 0), 2);
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (6, 6, 0)),
        Some(2),
        "the change landed"
    );

    cdda_sim::revert::restore(&mut world);
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (6, 6, 0)),
        Some(1),
        "and the floor is a floor again"
    );
}

/// **A `revert_location` event names its square, and puts back only that square's part of the map.** Reference: `f_revert_location` (`npctalk.cpp:4884-4918`) schedules
/// `REVERT_SUBMAP` **with the map square**, and `submap::revert_submap` (`submap.cpp:322-347`) reverts that submap: two reverts pending in different places are two
/// events, and firing one must not fire the other.
///
/// **The level is loaded rather than built square by square**, because a square is a `Square` entity and the loader is what makes one: `set_terrain` on bare ground
/// makes ground that `square::squares` cannot see, which is a latent inconsistency this test found and `docs/parity.md` records.
#[test]
fn a_revert_event_puts_back_only_its_own_part_of_the_map() {
    let mut world = World::new();
    // A level whose far corner is twenty squares away, so the two reverts cannot reach each other.
    let mut wide = cdda_core_types::level::Level::new();
    for y in 0..24 {
        for x in 0..24 {
            wide.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &wide);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (2, 2, 0));
    let near = (2, 2, 0);
    let far = (21, 21, 0);
    // **A second revert is pending elsewhere**: its copy was taken and its square has already changed. Nothing about the first revert may touch it.
    cdda_sim::revert::take(&mut world, far);
    cdda_sim::square::set_terrain(&mut world, far, 2);
    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "spot",
        cdda_sim::dialogue_values::Value::Position {
            x: near.0,
            y: near.1,
            z: near.2,
        },
    );
    cdda_sim::calendar::set_turn(&mut world, 0);
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"revert_location": {"u_val": "spot"}, "time_in_future": "5 turns"}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        cdda_sim::timed_events::place_of(&world, "REVERT_SUBMAP"),
        Some(near),
        "**the event names the square it will put back**"
    );
    // **Changed after the effect took its copy**, which is the reference's order: the copy is taken when the effect runs, so this change is one the revert undoes.
    cdda_sim::square::set_terrain(&mut world, near, 2);
    cdda_sim::calendar::set_turn(&mut world, 6);
    cdda_sim::timed_events::process_due(&mut world, survivor);
    assert_eq!(
        cdda_sim::square::terrain_at(&world, near),
        Some(1),
        "**its own square came back**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, far),
        Some(2),
        "**and the other place was left as the update made it**"
    );
    assert!(
        cdda_sim::revert::has_snapshot(&world),
        "**the other revert is still waiting for its own turn**"
    );
}
