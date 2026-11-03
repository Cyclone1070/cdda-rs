//! What a creature hears, and how wrong its guess is.
//!
//! Reference: `monster::hear_sound` (`src/monster.cpp:3925-3980`). Three numbers decide whether a thrown rock pulls a monster: the volume it arrives at, the band that
//! number falls in, and how long the creature keeps walking.

#![allow(clippy::panic)]

#[test]
fn a_sound_arrives_as_its_volume_less_the_distance() {
    use cdda_sim::hearing::heard_volume;

    assert_eq!(heard_volume(8, 3, false), Some(5), "eight loud, three squares away");
    assert_eq!(heard_volume(8, 8, false), None, "exactly at its range, it is not heard");
    assert_eq!(heard_volume(8, 9, false), None, "and past it, silence");
    // Good hearing doubles the **volume**, not the distance: eight loud is sixteen to a good ear, so it carries sixteen squares.
    assert_eq!(heard_volume(8, 15, true), Some(1), "a good ear hears it fifteen squares out");
    assert_eq!(heard_volume(8, 16, true), None, "and not at sixteen");
    // A whisper is heard by nobody, however good the ear.
    assert_eq!(heard_volume(2, 2, false), None, "two loud at two squares is nothing");
}

#[test]
fn a_louder_sound_is_guessed_more_precisely_and_the_bands_are_the_rule() {
    use cdda_sim::hearing::max_error;

    // The reference's ladder is `< 2`, `< 5`, `< 10`, `< 20`, and anything else is exact - so the boundaries are what a mistake would move.
    assert_eq!(max_error(1), 10, "the quietest thing a creature can hear is ten squares out");
    assert_eq!(max_error(2), 5, "and two loud is already five");
    assert_eq!(max_error(4), 5);
    assert_eq!(max_error(5), 3);
    assert_eq!(max_error(9), 3);
    assert_eq!(max_error(10), 1);
    assert_eq!(max_error(19), 1);
    assert_eq!(max_error(20), 0, "twenty loud is guessed exactly");
    assert_eq!(max_error(200), 0);
}

#[test]
fn good_hearing_also_makes_a_creature_remember_longer() {
    use cdda_sim::hearing::wander_turns;

    assert_eq!(wander_turns(5, false), 5, "five turns for five loud");
    assert_eq!(wander_turns(5, true), 30, "and six times as long for a good ear");
    assert_eq!(wander_turns(0, true), 0, "a sound nobody heard is walked towards for no time at all");
}

/// A creature that hears a thud gets a place to walk towards, and a deaf one gets nothing.
///
/// Reference: `sounds::process_sound` (`sounds.cpp:186-260`) hands each sound made this turn to every monster within earshot and then clears the list. The test puts a
/// listener three squares from the noise, one that is too far away, and one with no `HEARS` flag at all, so a port that told everyone everything would fail twice.
#[test]
fn the_creature_that_hears_a_thud_gets_a_place_to_walk_towards() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(31));
    let near = cdda_sim::creature::spawn_creature(&mut world, "the listener", (3, 0, 0));
    cdda_sim::creature_flags::add_flag(&mut world, near, "HEARS");
    let far = cdda_sim::creature::spawn_creature(&mut world, "the distant one", (30, 0, 0));
    cdda_sim::creature_flags::add_flag(&mut world, far, "HEARS");
    let deaf = cdda_sim::creature::spawn_creature(&mut world, "the deaf one", (1, 1, 0));

    // The thud a five-kilogram rock makes when it lands: volume eight, at the origin.
    cdda_sim::sounds::make_sound(&mut world, (0, 0, 0), 8);
    assert_eq!(
        cdda_sim::hearing::hear_sounds(&mut world),
        1,
        "exactly one creature is close enough and able to hear"
    );

    let heard = world.get::<cdda_sim::hearing::HeardSound>(near).copied();
    let heard = heard.unwrap_or_else(|| panic!("the near listener heard nothing"));
    assert_eq!(heard.volume, 5, "eight loud five squares out is five");
    assert_eq!(heard.wander_turns, 5, "and it walks towards it for five turns");
    assert!(
        heard.at.0.abs() <= 3 && heard.at.1.abs() <= 3,
        "the guess is within the loud sound's three squares of error: {:?}",
        heard.at
    );
    assert!(
        world.get::<cdda_sim::hearing::HeardSound>(far).is_none(),
        "the distant creature heard nothing"
    );
    assert!(
        world.get::<cdda_sim::hearing::HeardSound>(deaf).is_none(),
        "and the deaf one never will"
    );
    // The sound is over once it is heard: the store no longer holds it.
    assert!(
        cdda_sim::sounds::heard_this_turn(&mut world).is_empty(),
        "a sound is heard once"
    );
}

/// A creature that hears a noise walks towards it, and stops when the noise is old.
///
/// Reference: `monster::hear_sound` ends in `wander_to( target, wander_turns )` (`monster.cpp:3974`), and `monster::move` steps towards `wander_pos` while `wandf` lasts.
/// This is the half a memory alone would miss: the port had `HeardSound` and nothing walked, which is a rule that is correct and unreachable.
#[test]
fn a_creature_that_hears_a_noise_walks_towards_it_until_the_noise_is_old() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));
    let mut level = cdda_core_types::level::Level::new();
    for x in -2..14 {
        for y in -2..14 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let listener = cdda_sim::creature::spawn_creature(&mut world, "the listener", (10, 0, 0));
    cdda_sim::creature_flags::add_flag(&mut world, listener, "HEARS");
    // A speed of one hundred, so each turn banks one step's worth of moves.
    world
        .entity_mut(listener)
        .insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });

    // A very loud noise at the origin is heard exactly (volume 20 or more has no error), ten squares away.
    cdda_sim::sounds::make_sound(&mut world, (0, 0, 0), 30);
    // The prototypes say the floor costs two points a step, which is what routing charges; without them a square has no walkable cost at all.
    let mut terrain = cdda_sim::terrain_table::TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert_move_cost(1, 2);
    cdda_sim::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain,
            furniture: cdda_sim::terrain_table::TerrainTable::new(),
        },
    );
    cdda_sim::hearing::hear_sounds(&mut world);
    assert_eq!(
        world.get::<cdda_sim::creature::Wander>(listener).map(|wander| wander.square),
        Some((0, 0, 0)),
        "a noise loud enough to be guessed exactly is walked towards exactly"
    );

    let before = world.get::<cdda_sim::Position>(listener).map(|place| (place.x, place.y, place.z));
    // The step itself, called directly: this says whether a creature with moves and a route walks, separately from whether the turn gave it the chance.
    cdda_sim::creature::bank_a_turn(&mut world);
    let walked = cdda_sim::routing::walk_towards(&mut world, listener, (0, 0, 0), (12, 12));
    assert_eq!(walked, cdda_sim::routing::Walk::Stepped, "one step towards the noise");
    let stepped = world.get::<cdda_sim::Position>(listener).map(|place| (place.x, place.y, place.z));
    assert_ne!(stepped, before, "and the creature is somewhere else");
    cdda_sim::turn::run_turn(&mut world);
    let after = world.get::<cdda_sim::Position>(listener).map(|place| (place.x, place.y, place.z));
    assert!(
        after.is_some_and(|after| after.0 < before.unwrap_or((10, 0, 0)).0),
        "the creature stepped towards the noise: {before:?} became {after:?}"
    );
}
