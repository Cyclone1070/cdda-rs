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
        .insert(Mending { per_hour: 3_600.0, type_speed: 100, type_hp: 80 });
    let whole = cdda_sim::creature::spawn_creature(&mut world, "the whole one", (2, 2, 0));
    world
        .entity_mut(whole)
        .insert(Health { hp: 80, max_hp: 80 })
        .insert(Mending { per_hour: 3_600.0, type_speed: 100, type_hp: 80 });

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
        .map(|mending| mending.per_hour);
    assert!(
        rate.is_some_and(|rate| (rate - 0.02 * f64::from(monster_type.hp)).abs() < 1e-9),
        "a zombie mends at the reviving rate: {rate:?} for {} hit points",
        monster_type.hp
    );
}

/// A wounded monster's speed comes back with its body, and a whole one gets all of it.
///
/// Reference: `monster::regenerate` (`monster.cpp:4104-4115`): while the monster's base speed is under its type's, mending **restores speed too** - all of it when the hit points
/// are back, and `max( healed * type_speed / type_hp, 1 )` at a time before that. A monster that only ever regained hit points would walk slowly for the rest of its life.
#[test]
fn a_monster_gets_its_speed_back_with_its_body() {
    use cdda_sim::regeneration::speed_back;

    // The arguments are (base speed, the type's speed, what mending healed, the type's hit points, hit points now).
    // Everything back at once when the body is whole.
    assert_eq!(speed_back(30, 70, 70, 80, 80), Some(70), "a whole body walks at its own speed");
    // And gradually while it is not: ten healed of a type's eighty puts back ten eightieths of seventy, which is eight whole points.
    assert_eq!(speed_back(30, 70, 10, 80, 40), Some(38), "eight points for ten healed");
    // A mending that heals nothing still gives a point - the reference's own floor.
    assert_eq!(speed_back(30, 70, 0, 80, 40), Some(31), "at least one point");
    // Nothing to restore means nothing changes, so the caller can skip the write.
    assert_eq!(speed_back(70, 70, 40, 80, 40), None, "already as fast as its type");
}

/// A slowed monster walks faster again as its body mends, in the turn stage that mends it.
///
/// Reference: `monster::regenerate` restores base speed as it heals (`monster.cpp:4104-4115`). This is the **wire**, not the rule: the test drives the turn and reads the speed
/// afterwards, so a port that computed the new speed and threw it away would fail here.
#[test]
fn a_slowed_monster_walks_faster_again_as_it_mends() {
    use cdda_sim::creature::{Health, Speed};
    use cdda_sim::regeneration::Mending;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(3));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);

    let hurt = cdda_sim::creature::spawn_creature(&mut world, "the slowed one", (1, 1, 0));
    world
        .entity_mut(hurt)
        .insert(Health { hp: 40, max_hp: 80 })
        .insert(Speed { base: 30, bonus: 0 })
        .insert(Mending { per_hour: 3_600.0, type_speed: 70, type_hp: 80 });

    cdda_sim::turn::run_turn(&mut world);
    assert_eq!(
        world.get::<Speed>(hurt).map(|speed| speed.base),
        Some(31),
        "one healed point of eighty is one eightieth of seventy, which floors at one point"
    );

    // Forty turns of mending put all of it back, and never more than the type's own speed.
    for _ in 0..40 {
        cdda_sim::turn::run_turn(&mut world);
    }
    assert_eq!(
        world.get::<Speed>(hurt).map(|speed| speed.base),
        Some(70),
        "and a whole body walks as fast as its type"
    );
}

/// One blow is one rule with two callers, and a creature with nothing to hit with does not swing.
///
/// Reference: `Creature::deal_melee_attack` is what both a monster's move-into-a-creature and a character's attack end in. The port keeps that as `creature::strike_at`, and this
/// gate pins both halves: a creature with an attack takes hit points off its victim, and one without an attack answers **nothing** rather than a zero, so a caller can tell
/// "missed" from "could not swing".
#[test]
fn a_blow_takes_hit_points_and_a_creature_with_nothing_to_hit_with_cannot_swing() {
    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(21));
    let attacker = cdda_sim::creature::spawn_creature(&mut world, "the attacker", (0, 0, 0));
    let victim = cdda_sim::creature::spawn_creature(&mut world, "the victim", (1, 0, 0));
    cdda_sim::creature::set_health(&mut world, victim, 100);

    // Nothing to hit with: no `MeleeAttack`, so no swing at all.
    assert_eq!(
        cdda_sim::creature::strike_at(&mut world, attacker, victim),
        None,
        "a creature with no attack does not swing"
    );

    // Two dice of three at skill four, the corpus zombie's own numbers, and a victim with no armour: the blow lands for something.
    world
        .entity_mut(attacker)
        .insert(cdda_sim::creature::MeleeAttack(cdda_sim::melee::Attack {
            accuracy_tenths: 40,
            fixed: cdda_sim::melee::Damage::default(),
            dice: 2,
            sides: 3,
        }));
    let before = cdda_sim::creature::health_of(&world, victim);
    let dealt = cdda_sim::creature::strike_at(&mut world, attacker, victim);
    let after = cdda_sim::creature::health_of(&world, victim);
    assert!(dealt.is_some(), "and with one it swings");
    assert!(
        after <= before,
        "the victim is not healed by being hit: {before:?} became {after:?}"
    );
}

/// A monster that reaches the survivor inside a turn takes hit points off it.
///
/// This is the half of the fight that has never been proved **through the schedule**: every other gate in this file swings out of band, and the port's own playtest found that a
/// zombie standing beside the survivor for three turns left it at full health. Reference: `monster::move_to` refuses a square another creature holds, and `monster::attack` is
/// what it does instead - so the swing happens in the creature stage, with the dice the schedule has.
#[test]
fn a_monster_that_reaches_the_survivor_hurts_it_in_the_turn() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(11));
    let mut level = Level::new();
    for x in 0..5 {
        for y in 0..5 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);

    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world
        .entity_mut(zombie)
        .insert(cdda_sim::creature::MeleeAttack(cdda_sim::melee::Attack {
            accuracy_tenths: 40,
            fixed: cdda_sim::melee::Damage::default(),
            dice: 2,
            sides: 3,
        }))
        .insert(cdda_sim::creature::Speed { base: 100, bonus: 0 })
        .insert(cdda_sim::creature::Goal { square: (0, 0, 0) });

    let before = cdda_sim::creature::health_of(&world, survivor).unwrap_or(0);
    for _ in 0..4 {
        cdda_sim::turn::run_turn(&mut world);
    }
    let after = cdda_sim::creature::health_of(&world, survivor).unwrap_or(0);
    assert!(
        after < before,
        "the survivor stood next to a zombie for four turns and lost nothing: {before} became {after}"
    );
}

/// When a corpse is ready to get up again.
///
/// Reference: `item::ready_to_revive` (`item.cpp:8877-8912`). The age is worked out in whole hours and **damage makes it younger** - `age_in_hours /= damage_level + 1`, so a battered
/// corpse revives sooner - and then `rez_factor = 48 - age_in_hours`: a corpse older than six hours is ready when the factor has run out, or on a one-in-`rez_factor` chance. The
/// six-hour floor is the rule's whole shape: a corpse killed a minute ago does not stand up.
#[test]
fn a_corpse_gets_up_when_its_age_says_so() {
    use cdda_sim::regeneration::ready_to_revive;

    // Younger than six hours: never, whatever the dice say.
    for seed in 0..20 {
        let mut rolls = Rolls::new(seed);
        assert!(
            !ready_to_revive(5, 0, &mut rolls),
            "a corpse five hours old stays down (seed {seed})"
        );
    }

    // Past forty-eight hours the factor is gone, so the answer is yes with no dice spent at all.
    let mut rolls = Rolls::new(3);
    assert!(ready_to_revive(48, 0, &mut rolls), "two days is long enough");
    assert!(ready_to_revive(72, 0, &mut rolls), "and longer still");

    // Damage makes a corpse younger: twelve hours at damage level one is six, which is not past the floor.
    let mut rolls = Rolls::new(5);
    assert!(
        !ready_to_revive(12, 1, &mut rolls),
        "an hour of age is halved by each level of damage"
    );
    assert!(
        ready_to_revive(14, 1, &mut rolls),
        "and fourteen hours at one level is seven, which is past it"
    );

    // Between seven and forty-seven hours the answer is a die: over many draws both answers appear, which is what makes it a chance rather than a rule.
    //
    // The draws come from **one** stream rather than one per seed: the engine's first output for a small seed is the same small number every time, so a hundred fresh seeds all
    // answer alike. A stream that advances is what a real turn looks like.
    let mut rolls = Rolls::new(1);
    let mut got_up = 0;
    let mut stayed_down = 0;
    for _ in 0..400 {
        if ready_to_revive(7, 0, &mut rolls) {
            got_up += 1;
        } else {
            stayed_down += 1;
        }
    }
    assert!(
        got_up > 0 && stayed_down > 0,
        "a seven-hour corpse is a one-in-forty-one chance: {got_up} up, {stayed_down} down"
    );
}

/// A reviving corpse gets up once its age says so, and nothing else does.
///
/// Reference: `game::revive_corpse` (`game.cpp:5718-5760`) destroys the corpse and places the monster it was. The gate drives the **system**, not the rule: a corpse two days old
/// whose type revives must become a creature again on its own square, a fresh corpse must stay a corpse, and one whose type does not revive must stay a corpse however old it is.
#[test]
fn a_reviving_corpse_gets_up_and_nothing_else_does() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(4));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::calendar::set_turn(&mut world, 48 * 3_600);

    let corpse_kind = {
        let mut kind = cdda_sim::item_registry::Kind {
            id: "corpse",
            volume_ml: 250,
            weight_g: 1_000,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        };
        kind.pockets = &[];
        cdda_sim::item_registry::set_item_types(&mut world, &[kind]);
        cdda_sim::item_registry::id_of(&mut world, "corpse").expect("the corpse kind")
    };

    // Two days old and reviving; one hour old and reviving; two days old and not.
    let mut place = |square: (i32, i32), revives: bool, died_on: i64| {
        let corpse = cdda_sim::drop_item(&mut world, square, corpse_kind, 1);
        world.entity_mut(corpse).insert(cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(0),
            died_on,
            revives,
            monster_volume_ml: 62_500,
        });
        corpse
    };
    let old_riser = place((0, 0), true, 0);
    let fresh = place((1, 0), true, 48 * 3_600 - 3_600);
    let old_stayer = place((2, 0), false, 0);

    let mut spawned = Vec::new();
    let risen = cdda_sim::creature::revive_corpses(&mut world, |world, kind, square| {
        spawned.push((kind, square));
        cdda_sim::creature::spawn_creature(world, "the risen", square);
        true
    });

    assert_eq!(risen, 1, "exactly one corpse got up");
    assert_eq!(
        spawned,
        vec![(cdda_sim::MonsterKindId(0), (0, 0, 0))],
        "the two-day-old reviving corpse, on its own square"
    );
    assert!(world.get_entity(old_riser).is_err(), "and its corpse is gone");
    assert!(world.get_entity(fresh).is_ok(), "an hour old stays down");
    assert!(world.get_entity(old_stayer).is_ok(), "and one that does not revive stays down for ever");
}

/// A monster killed in a turn leaves a corpse behind, and the corpse knows what it was.
///
/// Reference: `monster::die` puts a corpse on the map (`monster.cpp`), and `item::corpse_volume` measures it from the monster's own volume. The gate drives the **stage**: when the
/// content has installed what a corpse is, the creature stage's sweep leaves one on the square where the monster fell - with the kind, the volume and the turn it died on.
#[test]
fn a_monster_killed_in_a_turn_leaves_a_corpse() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(13));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::calendar::set_turn(&mut world, 500);

    let corpse_item = {
        let kind = cdda_sim::item_registry::Kind {
            id: "corpse",
            volume_ml: 250,
            weight_g: 1_000,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        };
        cdda_sim::item_registry::set_item_types(&mut world, &[kind]);
        cdda_sim::item_registry::id_of(&mut world, "corpse").expect("the corpse kind")
    };
    cdda_sim::creature::set_corpse_kind(
        &mut world,
        corpse_item,
        std::collections::BTreeMap::from([(cdda_sim::MonsterKindId(0), 62_500)]),
    );

    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (2, 2, 0));
    world
        .entity_mut(zombie)
        .insert(cdda_sim::creature::Health { hp: 0, max_hp: 80 })
        .insert(cdda_sim::creature::CreatureKind(cdda_sim::MonsterKindId(0)))
        .insert(cdda_sim::creature_flags::CreatureFlags(
            ["REVIVES"].iter().map(|flag| (*flag).to_owned()).collect(),
        ));

    cdda_sim::turn::run_turn(&mut world);

    assert!(world.get_entity(zombie).is_err(), "the dead creature is gone");
    let corpses: Vec<(Entity, cdda_sim::CorpseOf)> = {
        let mut query = world.query::<(Entity, &cdda_sim::CorpseOf)>();
        query
            .iter(&world)
            .map(|(entity, corpse)| (entity, *corpse))
            .collect()
    };
    assert_eq!(corpses.len(), 1, "one corpse lies where it fell");
    let Some((_, corpse)) = corpses.first() else {
        panic!("a corpse was expected")
    };
    assert_eq!(corpse.kind, cdda_sim::MonsterKindId(0), "and knows what it was");
    assert_eq!(corpse.monster_volume_ml, 62_500, "and how big it was");
    assert_eq!(corpse.died_on, 500, "and when it died, which is what its age counts from");
    assert!(corpse.revives, "and that it can get up again");
}

/// A monster's blow carries its type's armour penetration, so a tough hide does not stop it.
///
/// Reference: `monster::melee_attack` adds the dice as **bash** with `melee_dice_ap` as penetration (`monster.cpp:2145`), and `get_effective_resist` clamps at zero
/// (`damage.cpp:724-727`). The gate reads the **component** the spawn writes, so a spawn that forgot the penetration fails here even though every rule still composes.
#[test]
fn a_monster_spawned_from_its_type_carries_that_type_s_armour_piercing() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..3 {
        for y in 0..3 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_piercer",
            hp: 40,
        }],
    );
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_piercer").expect("the kind");

    // The type is the corpus's own zombie with one field changed, so the fixture cannot drift from the data it stands for.
    let mut monster_type = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
        let text = std::fs::read(path).expect("the monster dump exists");
        let value = cdda_json::parse(&text).expect("the dump is valid JSON");
        let cdda_json::Value::Object(root) = value else {
            panic!("the dump's root is not an object")
        };
        let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
            panic!("the dump has no prototypes")
        };
        cdda_content::monsters::load_objects(prototypes)
            .expect("the dump loads")
            .into_iter()
            .find(|one| one.id == "mon_zombie")
            .expect("the corpus has a zombie")
    };
    monster_type.id = "mon_piercer".to_owned();
    monster_type.melee_dice_ap = 5;
    let monster = cdda_sim::creature::spawn_monster_of_type(&mut world, kind, &monster_type, (1, 1, 0), 100);

    let attack = world
        .get::<cdda_sim::creature::MeleeAttack>(monster)
        .expect("a spawned monster can hit")
        .0;
    assert_eq!(attack.dice, 2, "two dice");
    assert_eq!(attack.sides, 3, "of three faces");
    assert_eq!(
        attack.fixed.penetration, 5,
        "and the type's own armour piercing"
    );
    assert_eq!(
        attack.fixed.bash, 0,
        "the flat part of the blow stays zero, because the data states none"
    );
}

/// A monster whose data states a flat part to its blow hits for that part **as well as** its dice.
///
/// Reference: `monster::melee_attack` starts from `type->melee_damage` and then adds the dice as bash (`monster.cpp:2149-2152`). The gate reads the corpus, not a hand-built type:
/// `mon_crow_mutant_small` states a cut of two and no bash, so a spawn that dropped the flat part would leave the blow with dice alone.
#[test]
fn a_monster_whose_data_states_a_flat_blow_hits_for_it() {
    use bevy_ecs::prelude::*;

    let types = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
        let text = std::fs::read(path).expect("the monster dump exists");
        let value = cdda_json::parse(&text).expect("the dump is valid JSON");
        let cdda_json::Value::Object(root) = value else {
            panic!("the dump's root is not an object")
        };
        let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
            panic!("the dump has no prototypes")
        };
        cdda_content::monsters::load_objects(prototypes).expect("the dump loads")
    };
    let Some(crow) = types
        .iter()
        .find(|one| one.id == "mon_crow_mutant_small")
    else {
        panic!("the corpus has the crow")
    };
    assert_eq!(crow.melee_cut, 2, "the dump states a cut of two");
    assert_eq!(crow.melee_bash, 0, "and no bash of its own");

    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..3 {
        for y in 0..3 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let monster = cdda_sim::creature::spawn_monster_of_type(
        &mut world,
        cdda_sim::MonsterKindId(1),
        crow,
        (1, 1, 0),
        100,
    );
    let attack = world
        .get::<cdda_sim::creature::MeleeAttack>(monster)
        .expect("a spawned monster can hit")
        .0;
    assert_eq!(attack.fixed.cut, 2, "the flat cut arrives with the monster");
    assert_eq!(attack.fixed.bash, 0, "and the flat bash it does not have stays zero");
    assert!(attack.dice > 0, "and its dice are still there as well: {}", attack.dice);
}

/// A blow is charged to the monster before it lands, so a monster that has walked cannot also swing for free.
///
/// Reference: `monster::melee_attack` calls `mod_moves( -type->attack_cost )` **before** it checks its reach (`monster.cpp:2108`) - the charge is for the attempt, not the hit. The
/// gate gives the monster exactly two turns' worth of moves and watches both blows take their cost, so a strike that forgot to charge would leave the count untouched.
#[test]
fn a_monster_pays_for_its_blow_out_of_its_own_moves() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(11));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);

    let mut monster_type = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
        let text = std::fs::read(path).expect("the monster dump exists");
        let value = cdda_json::parse(&text).expect("the dump is valid JSON");
        let cdda_json::Value::Object(root) = value else {
            panic!("the dump's root is not an object")
        };
        let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
            panic!("the dump has no prototypes")
        };
        cdda_content::monsters::load_objects(prototypes)
            .expect("the dump loads")
            .into_iter()
            .find(|one| one.id == "mon_zombie")
            .expect("the corpus has a zombie")
    };
    assert_eq!(monster_type.attack_cost, 100, "a zombie's blow costs a turn");
    monster_type.attack_cost = 60;

    let zombie = cdda_sim::creature::spawn_monster_of_type(
        &mut world,
        cdda_sim::MonsterKindId(1),
        &monster_type,
        (1, 1, 0),
        100,
    );
    let survivor = cdda_sim::spawn_player(&mut world, (0, 1), 80, 0);
    cdda_sim::creature::set_moves(&mut world, zombie, 200);

    cdda_sim::creature::strike_at(&mut world, zombie, survivor).expect("the zombie can swing");
    assert_eq!(
        cdda_sim::creature::moves_left(&world, zombie),
        140,
        "the first blow costs its sixty, hit or miss"
    );
    cdda_sim::creature::strike_at(&mut world, zombie, survivor).expect("and again");
    assert_eq!(
        cdda_sim::creature::moves_left(&world, zombie),
        80,
        "the second costs the same"
    );
}

/// A monster that states no dice deals no damage, whatever flat part its data states.
///
/// Reference: `monster::melee_attack` deals the hit, then returns early - `if( type->melee_dice == 0 ) { return true; }` (`monster.cpp:2128-2131`) - so the dice count is the
/// type's permission to hurt anyone at all. **159 of the corpus's 1,154 monsters state no dice, and five of them state a flat part too** (a smoker's 2 cut, a skittergun's 8 bash):
/// those five would hurt the survivor if the port read the flat part and forgot the gate.
#[test]
fn a_monster_with_no_dice_hurts_nobody_however_much_flat_damage_its_data_states() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(17));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);

    let mut monster_type = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
        let text = std::fs::read(path).expect("the monster dump exists");
        let value = cdda_json::parse(&text).expect("the dump is valid JSON");
        let cdda_json::Value::Object(root) = value else {
            panic!("the dump's root is not an object")
        };
        let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
            panic!("the dump has no prototypes")
        };
        cdda_content::monsters::load_objects(prototypes)
            .expect("the dump loads")
            .into_iter()
            .find(|one| one.id == "mon_zombie_smoker")
            .expect("the corpus has a smoker")
    };
    // The data's own shape: no dice, and a flat cut of two.
    monster_type.melee_dice = 0;
    monster_type.melee_bash = 0;
    monster_type.melee_cut = 2;
    monster_type.melee_stab = 0;
    monster_type.melee_skill = 10;

    let monster = cdda_sim::creature::spawn_monster_of_type(
        &mut world,
        cdda_sim::MonsterKindId(1),
        &monster_type,
        (1, 1, 0),
        100,
    );
    let survivor = cdda_sim::spawn_player(&mut world, (0, 1), 80, 0);

    // Two dozen blows, so a one-in-a-hundred accident cannot hide a rule that does hurt.
    for _ in 0..24 {
        cdda_sim::creature::strike_at(&mut world, monster, survivor);
    }
    assert_eq!(
        cdda_sim::creature::health_of(&world, survivor),
        Some(80),
        "no dice means no permission to hurt anyone"
    );
}

/// A monster that hit and runs is running after its blow lands.
///
/// Reference: `monster::melee_attack` gives the monster `effect_run` for four turns when it carries `HIT_AND_RUN` (`monster.cpp:2134-2136`), which is what makes the flag mean
/// anything. **121 of the corpus's 1,154 monsters carry it.** The gate uses the monster's own flags, so a spawn that dropped them fails here too.
#[test]
fn a_monster_that_hits_and_runs_is_running_after_its_blow() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(23));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let run = cdda_json::parse(br#"{"id":"run","max_duration":"10 turns"}"#).expect("valid json");
    let cdda_json::Value::Object(entry) = run else {
        panic!("an effect type is an object")
    };
    let kind = cdda_content::effects::read_one("run", &entry);
    cdda_sim::effect_registry::set_effect_types(&mut world, std::slice::from_ref(&kind));

    let mut monster_type = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
        let text = std::fs::read(path).expect("the monster dump exists");
        let value = cdda_json::parse(&text).expect("the dump is valid JSON");
        let cdda_json::Value::Object(root) = value else {
            panic!("the dump's root is not an object")
        };
        let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
            panic!("the dump has no prototypes")
        };
        cdda_content::monsters::load_objects(prototypes)
            .expect("the dump loads")
            .into_iter()
            .find(|one| one.id == "mon_zombie")
            .expect("the corpus has a zombie")
    };
    let runner = cdda_sim::creature::spawn_monster_of_type(&mut world, cdda_sim::MonsterKindId(1), &monster_type, (1, 1, 0), 100);
    let survivor = cdda_sim::spawn_player(&mut world, (0, 1), 80, 0);

    // A plain zombie does not run.
    for _ in 0..12 {
        cdda_sim::creature::strike_at(&mut world, runner, survivor);
    }
    assert!(
        !cdda_sim::applied_effects::has(&world, runner, "run"),
        "a zombie stands and fights"
    );

    // The same monster with the flag, spawned from the same type.
    monster_type.flags.insert("HIT_AND_RUN".to_owned());
    let runner = cdda_sim::creature::spawn_monster_of_type(&mut world, cdda_sim::MonsterKindId(1), &monster_type, (1, 2, 0), 100);
    let mut ran = false;
    for _ in 0..12 {
        cdda_sim::creature::strike_at(&mut world, runner, survivor);
        if cdda_sim::applied_effects::has(&world, runner, "run") {
            ran = true;
        }
    }
    assert!(ran, "and one with the flag runs after a blow that lands");
}
