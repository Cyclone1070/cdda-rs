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
