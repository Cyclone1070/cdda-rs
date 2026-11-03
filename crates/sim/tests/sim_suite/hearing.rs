//! What a creature hears, and how wrong its guess is.
//!
//! Reference: `monster::hear_sound` (`src/monster.cpp:3925-3980`). Three numbers decide whether a thrown rock pulls a monster: the volume it arrives at, the band that
//! number falls in, and how long the creature keeps walking.

#![allow(clippy::expect_used, clippy::panic)]

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

/// A docile creature does not walk towards a noise.
///
/// Reference: `monster::wander_to` refuses while the monster carries `effect_docile` (`monster.cpp:1677`), so a noise cannot make a docile monster break off. The port asks the
/// same question of the same effect, through the effect store it already uses for every other duration.
#[test]
fn a_docile_creature_does_not_walk_towards_a_noise() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
    let listener = cdda_sim::creature::spawn_creature(&mut world, "the tamed one", (6, 0, 0));
    cdda_sim::creature_flags::add_flag(&mut world, listener, "HEARS");
    cdda_sim::applied_effects::add(
        &mut world,
        listener,
        cdda_sim::effects::ActiveEffect {
            id: "docile".to_owned(),
            intensity: 1,
            duration: 100,
            permanent: false,
        },
    );

    // A very loud noise, loud enough to be guessed exactly, so the only thing that can stop the walk is the effect.
    cdda_sim::sounds::make_sound(&mut world, (0, 0, 0), 30);
    cdda_sim::hearing::hear_sounds(&mut world);
    assert!(
        world.get::<cdda_sim::creature::Wander>(listener).is_none(),
        "a docile creature does not start walking towards a noise"
    );
    // It still **hears** it: the reference refuses the walk, not the hearing, and a creature that heard nothing would fail a test that checked the memory instead.
    assert!(
        world.get::<cdda_sim::hearing::HeardSound>(listener).is_some(),
        "but it does hear it"
    );
}

/// The whole chain: a noise, a monster that hears it, a walk, and a blow.
///
/// Reference: `sounds::process_sound` hands the noise over, `monster::hear_sound` ends in `wander_to`, `monster::move` walks the monster there, and `monster::attack` is what
/// happens when it arrives. Four rules from four files, and this test is the only place they are proved to **compose**: every one of them can be green on its own while the
/// chain is broken, which is the failure this port keeps meeting.
#[test]
fn a_noise_brings_a_monster_to_the_survivor_and_it_swings_when_it_arrives() {
    use cdda_sim::creature::{Health, MeleeAttack, Speed};
    use cdda_sim::terrain_table::TerrainTable;

    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(4242));
    let mut level = cdda_core_types::level::Level::new();
    for x in -1..14 {
        for y in -1..14 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let mut terrain = TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert_move_cost(1, 2);
    cdda_sim::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );

    // The survivor stands at the origin and can be hurt.
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(Health { hp: 100, max_hp: 100 })
        .insert(cdda_sim::creature::Armour { bash: 0, cut: 0 });

    // A zombie six squares away, able to hear, walking at a hundred speed, with a blow worth a couple of points.
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (6, 0, 0));
    world
        .entity_mut(zombie)
        .insert(Health { hp: 30, max_hp: 30 })
        .insert(Speed { base: 100, bonus: 0 })
        .insert(MeleeAttack(cdda_sim::melee::Attack {
            accuracy_tenths: 40,
            fixed: cdda_sim::melee::Damage::default(),
            dice: 2,
            sides: 3,
        }));
    cdda_sim::creature_flags::add_flag(&mut world, zombie, "HEARS");

    // The thud of a rock landing where the survivor stands.
    cdda_sim::sounds::make_sound(&mut world, (0, 0, 0), 30);
    let started = 6;
    for _ in 0..12 {
        cdda_sim::turn::run_turn(&mut world);
    }

    let arrived = world
        .get::<cdda_sim::Position>(zombie)
        .map(|place| (place.x, place.y));
    assert!(
        arrived.is_some_and(|(x, _)| x < started),
        "the monster walked towards the noise: it was at x = {started}, now {arrived:?}"
    );
    assert!(
        cdda_sim::creature::health_of(&world, survivor).is_some_and(|hp| hp < 100),
        "and it swung when it got there: the survivor is at {:?}",
        cdda_sim::creature::health_of(&world, survivor)
    );
}

/// A monster spawned from its type carries the type's flags, which is how it can hear at all.
///
/// Reference: `monster::monster( const mtype * )` copies `type->flags` onto the monster, and every question about what a monster can do - `can_hear`, `has_flag` - is asked of
/// that set (`monster.cpp:1318`). A port that spawned monsters without their flags gives every one of them a deaf, blind, armourless body that still has hit points.
#[test]
fn a_monster_from_its_type_carries_the_types_flags() {
    // The corpus zombie, which carries `HEARS`, `GOODHEARING`, `SEES` and the rest of the flags its data states.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let text = std::fs::read(path).expect("the monster dump exists");
    let root = cdda_json::parse(&text).expect("the dump is valid JSON");
    let cdda_json::Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    let hearing = cdda_content::monsters::load_objects(prototypes)
        .expect("the dump loads")
        .into_iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie");
    assert!(
        hearing.flags.contains("HEARS"),
        "the corpus zombie hears, which is what the test needs of it"
    );
    let (hp, stand) = (hearing.hp, (4, 0, 0));

    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 80,
        }],
    );
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_zombie").expect("a zombie");
    let zombie = cdda_sim::creature::spawn_monster_of_type(&mut world, kind, &hearing, stand, 100);

    assert!(
        cdda_sim::creature_flags::has_flag(&world, zombie, "HEARS"),
        "a zombie that hears has the flag its type gives it"
    );
    assert!(
        cdda_sim::creature_flags::has_flag(&world, zombie, "SEES"),
        "and every other flag it carries, not only the first"
    );
    // The flags it does **not** carry stay absent, which is the half a port that set every flag would fail.
    assert!(
        !cdda_sim::creature_flags::has_flag(&world, zombie, "GOODHEARING"),
        "the data does not give a zombie good hearing"
    );
    // The type's own numbers still arrive, which is what says the flags were added rather than substituted.
    assert_eq!(cdda_sim::creature::health_of(&world, zombie), Some(hp));
}
