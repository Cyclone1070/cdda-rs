//! A creature that reaches its goal hits what is standing there.
//!
//! Reference: `monster::move_to` refuses a square another creature holds, and `monster::attack` is what a
//! monster does instead - the swing is the move. These tests drive the **turn**, not `strike` directly, so
//! what is checked is that the behaviour is wired into the declared stage rather than existing as a function
//! nobody calls.

#![allow(clippy::panic, clippy::expect_used)]

// The mending rates are compared exactly: they are the reference's own constants (0.02, 0.005), and a rate that arrives a hair off is a port that reordered the branches.
#![allow(clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;
use cdda_sim::creature::{
    MeleeAttack, creature_at, health_of, set_goal, set_health, set_speed, spawn_creature,
};
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Prototypes, singletons};

const FLOOR: u32 = 1;

/// A one-row corridor of floor, with the world's dice seeded so the same seed gives the same fight.
fn corridor(length: i32, seed: u32) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..length {
        level.set_terrain(x, 0, FLOOR, FLOOR);
    }
    cdda_sim::load_level(&mut world, &level);

    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert_move_cost(FLOOR, 2);
    cdda_sim::set_prototypes(
        &mut world,
        Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );
    singletons::put_rolls(&mut world, Rolls::new(seed));
    world
}

/// Runs one creature stage, which is what asks creatures to act.
fn a_turn(world: &mut World) {
    cdda_sim::turn::run_turn(world);
}

fn zombie(world: &mut World, square: (i32, i32, i32), hp: i32) -> Entity {
    let entity = spawn_creature(world, "a zombie", square);
    set_health(world, entity, hp);
    set_speed(world, entity, 100, 0);
    // Two dice of three faces, at skill four - the corpus zombie's own numbers.
    world.entity_mut(entity).insert(MeleeAttack(cdda_sim::melee::Attack {
        accuracy_tenths: 40,
        fixed: cdda_sim::melee::Damage::default(),
        dice: 2,
        sides: 3,
    }));
    entity
}

#[test]
fn a_creature_whose_goal_is_in_reach_hits_instead_of_walking() {
    let mut world = corridor(3, 1);
    let zombie_entity = zombie(&mut world, (0, 0, 0), 20);
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    set_health(&mut world, survivor, 30);
    set_goal(&mut world, zombie_entity, (1, 0, 0));

    a_turn(&mut world);

    let taken = 30 - health_of(&world, survivor).expect("the survivor is alive");
    assert!(
        (2..=6).contains(&taken),
        "two dice of three faces take two to six, took {taken}"
    );
    assert_eq!(
        creature_at(&world, (0, 0, 0)),
        vec![zombie_entity],
        "and the zombie stayed where it was: the swing was its move"
    );
}

#[test]
fn a_creature_whose_goal_is_far_away_walks_toward_it() {
    let mut world = corridor(4, 1);
    let zombie_entity = zombie(&mut world, (0, 0, 0), 20);
    let survivor = spawn_creature(&mut world, "the survivor", (3, 0, 0));
    set_health(&mut world, survivor, 30);
    set_goal(&mut world, zombie_entity, (3, 0, 0));

    a_turn(&mut world);

    assert_eq!(
        creature_at(&world, (1, 0, 0)),
        vec![zombie_entity],
        "it stepped one square closer"
    );
    assert_eq!(
        health_of(&world, survivor),
        Some(30),
        "and nobody was hurt at that distance"
    );
}

#[test]
fn a_creature_with_no_attack_never_hurts_anyone() {
    let mut world = corridor(3, 1);
    let walker = spawn_creature(&mut world, "a walker", (0, 0, 0));
    set_health(&mut world, walker, 20);
    set_speed(&mut world, walker, 100, 0);
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    set_health(&mut world, survivor, 30);
    set_goal(&mut world, walker, (1, 0, 0));

    a_turn(&mut world);

    assert_eq!(
        health_of(&world, survivor),
        Some(30),
        "a creature with no melee attack cannot wound anyone, however close it stands"
    );
}

#[test]
fn a_creature_with_no_goal_does_nothing() {
    let mut world = corridor(3, 1);
    let zombie_entity = zombie(&mut world, (0, 0, 0), 20);
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    set_health(&mut world, survivor, 30);

    a_turn(&mut world);

    assert_eq!(creature_at(&world, (0, 0, 0)), vec![zombie_entity], "it stayed");
    assert_eq!(health_of(&world, survivor), Some(30), "and nobody was hit");
}

/// A dodging creature is harder to hit, and the number comes from its type.
///
/// Reference: `monster::get_dodge_base()` is `type->sk_dodge` (`monster.cpp:2643-2646`), and
/// `Creature::deal_melee_attack` takes that off the attacker's roll at five points each - which is the same
/// five that accuracy is multiplied by, so a point of dodge cancels a point of skill exactly.
#[test]
fn a_dodging_creature_takes_points_off_the_attacker_s_roll() {
    let mut world = World::new();
    let dodger = spawn_creature(&mut world, "a dodger", (0, 0, 0));
    let stumbler = spawn_creature(&mut world, "a stumbler", (1, 0, 0));
    world.entity_mut(dodger).insert(cdda_sim::creature::Dodge { points: 3 });

    let swinging = cdda_sim::melee::Attack {
        accuracy_tenths: 40,
        fixed: cdda_sim::melee::Damage::default(),
        dice: 2,
        sides: 3,
    };
    let mut rolls = Rolls::new(1);
    let against_dodger = cdda_sim::melee::strike(
        &mut rolls,
        swinging,
        cdda_sim::creature::defence_of(&world, dodger),
    );
    let mut rolls = Rolls::new(1);
    let against_stumbler = cdda_sim::melee::strike(
        &mut rolls,
        swinging,
        cdda_sim::creature::defence_of(&world, stumbler),
    );

    assert_eq!(
        against_stumbler.spread - against_dodger.spread,
        15,
        "three points of dodge at five each, on the same seed"
    );
}

/// A monster mends over time, and a zombie mends four times as fast as plain flesh.
///
/// Reference: `monster::regenerate` (`monster.cpp:4094-4103`): `regen = type->regenerates` when the data states one; otherwise `0.02 * type->hp` an hour for a monster that
/// `REVIVES` and is not `DORMANT`, otherwise `0.005 * type->hp` an hour for flesh or veggy. The heal is `roll_remainder( regen * dt )`, so a slow rate lands as a whole hit
/// point once in a while rather than as a fraction that never arrives.
#[test]
fn a_monster_mends_at_the_rate_its_type_and_its_body_give_it() {
    use cdda_sim::regeneration::mending_per_hour;

    // Four times the flesh rate, because it revives.
    let zombie = mending_per_hour(80, true, false, &["flesh"], 0.0);
    // The same body without the flag mends at the plain flesh rate, which is the half a port that only read the flag would miss.
    let plain = mending_per_hour(80, false, false, &["flesh"], 0.0);
    assert!(
        (zombie - 4.0 * plain).abs() < 1e-6,
        "a reviving body mends four times as fast: {zombie} against {plain}"
    );
    // A stated rate wins over both fallbacks.
    assert!((mending_per_hour(80, true, false, &["flesh"], 50.0) - 50.0).abs() < 1e-6);
    // Nothing flesh, no flag, no rate: nothing mends.
    assert_eq!(mending_per_hour(80, false, false, &["steel"], 0.0), 0.0);
    // A dormant reviver loses the **reviving** rate, not its flesh: `DORMANT` is part of the first branch's condition, so it falls through to the second and mends at 0.005.
    // The reference's own order is what makes the difference, and a port that read the flag as "stops mending" would be wrong in a way that only shows up in play.
    assert!(
        (mending_per_hour(80, true, true, &["flesh"], 0.0) - 0.4).abs() < 1e-6,
        "a dormant reviver mends like plain flesh"
    );
    // A dormant body that is not flesh at all mends by nothing.
    assert_eq!(mending_per_hour(80, true, true, &["steel"], 0.0), 0.0);
}

/// The rate lands as whole hit points, once in a while, and the total over an hour is the rate itself.
#[test]
fn mending_arrives_in_whole_hit_points_over_time() {
    use cdda_sim::regeneration::mend_over;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(9));
    // A shoggoth's rate: fifty an hour, which is one hit point every seventy-two turns.
    let mut mended = 0;
    for _ in 0..3_600 {
        mended += mend_over(&mut world, 50.0, 1);
    }
    assert!(
        (45..=55).contains(&mended),
        "an hour at fifty an hour is about fifty hit points, drawn as whole ones: {mended}"
    );
}

/// A wounded monster mends as the turns pass, and a whole one does not mend past its ceiling.
///
/// Reference: `monster::regenerate` is called from `monster::process_turn`, which the turn runs for every creature (`monster.cpp:4094`), and `Creature::heal` refuses to go
/// over `get_hp_max()`. The rate is a **component** in this port: it is worked out once, from the type and its body, when the monster is placed.
#[test]
fn a_wounded_monster_mends_as_the_turns_pass_and_never_past_its_ceiling() {
    use cdda_sim::creature::Health;
    use cdda_sim::regeneration::Mending;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(5));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);

    // A rate of one hour's worth per turn, so the arithmetic of `roll_remainder` cannot hide the wire: every turn mends exactly one point.
    let hurt = cdda_sim::creature::spawn_creature(&mut world, "the hurt one", (1, 1, 0));
    world
        .entity_mut(hurt)
        .insert(Health { hp: 40, max_hp: 80 })
        .insert(Mending(3_600.0));
    let whole = cdda_sim::creature::spawn_creature(&mut world, "the whole one", (2, 2, 0));
    world
        .entity_mut(whole)
        .insert(Health { hp: 80, max_hp: 80 })
        .insert(Mending(3_600.0));

    cdda_sim::turn::run_turn(&mut world);
    assert_eq!(
        world.get::<Health>(hurt).map(|health| health.hp),
        Some(41),
        "one turn of a mending rate worth an hour is one hit point"
    );
    assert_eq!(
        world.get::<Health>(whole).map(|health| health.hp),
        Some(80),
        "and a whole creature stays whole"
    );

    // Over a hundred turns the wounded one gains a hundred points' worth, which its ceiling stops at eighty.
    for _ in 0..100 {
        cdda_sim::turn::run_turn(&mut world);
    }
    assert_eq!(
        world.get::<Health>(hurt).map(|health| health.hp),
        Some(80),
        "mending stops at the ceiling rather than passing it"
    );
}

/// A monster placed from its type arrives mending at the rate the type and its body give it.
///
/// Reference: `monster::regenerate` reads the type's flags and materials every turn; the port decides the rate once, at the placing, so this is where the two rules meet -
/// `regeneration::mending_per_hour` and `spawn_monster_of_type`. A zombie states no rate of its own, so its rate is the reviving fallback: `0.02 * 80`.
#[test]
fn a_monster_placed_from_its_type_carries_the_rate_its_body_gives_it() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let bytes = std::fs::read(path).expect("the monster dump exists");
    let root = cdda_json::parse(&bytes).expect("the dump is valid JSON");
    let cdda_json::Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    let monster_type = cdda_content::monsters::load_objects(prototypes)
        .expect("the dump loads")
        .into_iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie");
    let mut world = World::new();
    let zombie = cdda_sim::creature::spawn_monster_of_type(
        &mut world,
        cdda_sim::MonsterKindId(7),
        &monster_type,
        (2, 2, 0),
        100,
    );

    let rate = world
        .get::<cdda_sim::regeneration::Mending>(zombie)
        .map(|mending| mending.0);
    assert!(
        rate.is_some_and(|rate| (rate - 0.02 * f64::from(monster_type.hp)).abs() < 1e-9),
        "a zombie mends at the reviving rate: {rate:?} for {} hit points",
        monster_type.hp
    );
}
