//! A creature that reaches its goal hits what is standing there.
//!
//! Reference: `monster::move_to` refuses a square another creature holds, and `monster::attack` is what a
//! monster does instead - the swing is the move. These tests drive the **turn**, not `strike` directly, so
//! what is checked is that the behaviour is wired into the declared stage rather than existing as a function
//! nobody calls.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

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
        .insert(Mending { per_hour: 3_600.0, type_speed: 100, type_hp: 80, in_dark: false });
    let whole = cdda_sim::creature::spawn_creature(&mut world, "the whole one", (2, 2, 0));
    world
        .entity_mut(whole)
        .insert(Health { hp: 80, max_hp: 80 })
        .insert(Mending { per_hour: 3_600.0, type_speed: 100, type_hp: 80, in_dark: false });

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
        .insert(Mending { per_hour: 3_600.0, type_speed: 70, type_hp: 80, in_dark: false });

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

/// Wind comes back every turn, faster when it has not been lost to panic.
///
/// Reference: `Character::update_stamina` (`character.cpp:7022-7082`) recovers `ceil( rate * turns )` a turn, where the rate is `PLAYER_BASE_STAMINA_REGEN_RATE` (20, from
/// `data/core/external_options.json`) scaled by a multiplier that is **one normally and a tenth when winded** (`has_effect( effect_winded ) ? 0.1f : 1.0f`, floored at 0.1), and then
/// clamps the result to between zero and the maximum. The port has no cardio fitness, so the effective rate is the base rate; that is named in the roadmap, not invented here.
#[test]
fn a_turn_gives_wind_back_and_panic_slows_it() {
    use cdda_sim::regeneration::{recover_wind, wind_back_per_turn};

    assert!(
        (wind_back_per_turn(20.0, false) - 20.0).abs() < 0.001,
        "twenty a turn at rest"
    );
    assert!(
        (wind_back_per_turn(20.0, true) - 2.0).abs() < 0.001,
        "and two when winded, which is a tenth"
    );
    // A rate under one is still at least one a turn: `std::max( 1.0f, ... )`.
    assert!(
        (wind_back_per_turn(0.0, false) - 1.0).abs() < 0.001,
        "there is always some recovery"
    );

    let mut world = World::new();
    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Stamina { cur: 100.0, max: 3_500.0 });
    recover_wind(&mut world, survivor, 20.0, false, 1);
    let after = world
        .get::<cdda_sim::creature::Stamina>(survivor)
        .expect("the survivor has wind")
        .cur;
    assert!((after - 120.0).abs() < 0.001, "one turn gives twenty back: {after}");

    // And it never goes above the ceiling.
    recover_wind(&mut world, survivor, 20.0, false, 1_000);
    let capped = world
        .get::<cdda_sim::creature::Stamina>(survivor)
        .expect("still there")
        .cur;
    assert!((capped - 3_500.0).abs() < 0.001, "and stops at the ceiling: {capped}");
}

/// The turn gives a character its wind back, and a creature without wind is left alone.
///
/// Reference: `Character::update_stamina` runs every turn for a character (`character.cpp:7022`). The gate drives the **stage**: a survivor drained to nothing comes out of a turn
/// with twenty, and a monster - which has no stamina pool at all - is not given one by the sweep.
#[test]
fn the_turn_gives_a_character_its_wind_back() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(29));
    let mut level = Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Stamina { cur: 0.0, max: 3_500.0 });
    let monster = cdda_sim::creature::spawn_creature(&mut world, "a rat", (2, 2, 0));

    cdda_sim::turn::run_turn(&mut world);

    let wind = world
        .get::<cdda_sim::creature::Stamina>(survivor)
        .expect("the survivor still has wind")
        .cur;
    assert!((wind - 20.0).abs() < 0.001, "a turn gives twenty back: {wind}");
    assert!(
        world.get::<cdda_sim::creature::Stamina>(monster).is_none(),
        "and a creature with no wind is not handed one"
    );
}

/// What a monster thinks of the survivor, from its morale, its anger and how hurt it is.
///
/// Reference: the tail of `monster::attitude_to` (`monster.cpp:1809-1833`), which is the whole decision once the faction, species and trait modifiers are set aside - and this port
/// has no factions, species bonuses or traits, so setting them aside is exact rather than approximate:
///
/// - a monster with **negative morale** follows while it can still fight (`morale + anger > 0` and more than a third of its hit points) and **flees** otherwise;
/// - a monster with **no anger** flees once it is down to sixty per cent of its hit points and otherwise ignores the survivor;
/// - a monster with **some anger, under ten**, follows;
/// - and anything else attacks.
///
/// The gate walks the tree in order, including the two boundaries: `hp > max / 3` exactly, and `hp <= 0.6 * max`.
#[test]
fn a_monster_s_attitude_follows_its_morale_its_anger_and_its_wounds() {
    use cdda_sim::creature::{Attitude, attitude};

    // Negative morale: any wound past two thirds and it runs; a scratch and anger to spare and it follows.
    assert_eq!(attitude(90, 100, -10, 25), Attitude::Follow, "hurt but still game");
    assert_eq!(attitude(33, 100, -10, 25), Attitude::Flee, "a third of its hit points is not game");
    assert_eq!(attitude(34, 100, -10, 25), Attitude::Follow, "one point more is");
    assert_eq!(attitude(90, 100, -10, 5), Attitude::Flee, "anger that does not outweigh the fear runs");

    // No anger at all: it ignores a healthy survivor and runs from a hurt one.
    assert_eq!(attitude(100, 100, 10, 0), Attitude::Ignore, "nothing to be angry about");
    assert_eq!(attitude(61, 100, 10, 0), Attitude::Ignore, "sixty-one per cent is above the line");
    assert_eq!(attitude(60, 100, 10, 0), Attitude::Flee, "sixty is the line, and it runs");

    // Some anger: it follows.
    assert_eq!(attitude(100, 100, 10, 5), Attitude::Follow, "a little anger is a tail, not a fight");

    // And enough anger: it attacks.
    assert_eq!(attitude(100, 100, 10, 10), Attitude::Attack, "ten points of anger is a fight");
    assert_eq!(attitude(1, 100, 10, 50), Attitude::Attack, "one hit point and furious is still furious");
}

/// A spawned monster starts with its type's morale and anger, which is what its attitude is read from.
///
/// Reference: `monster::monster` sets `anger = type->agro` and `morale = type->morale` (`monster.cpp:282-283`). The gate reads the **corpus**, so the numbers are the data's: a zombie's
/// aggression and morale, whatever the dump says they are.
#[test]
fn a_spawned_monster_starts_with_its_type_s_morale_and_anger() {
    use bevy_ecs::prelude::*;

    let zombie_type = {
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
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..3 {
        for y in 0..3 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let zombie = cdda_sim::creature::spawn_monster_of_type(&mut world, cdda_sim::MonsterKindId(1), &zombie_type, (1, 1, 0), 100);

    assert_eq!(
        world.get::<cdda_sim::creature::Morale>(zombie).map(|carried| carried.0),
        Some(zombie_type.morale),
        "the monster's morale is its type's"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Anger>(zombie).map(|carried| carried.0),
        Some(zombie_type.aggression),
        "and its anger its type's aggression"
    );
    assert!(
        zombie_type.aggression >= 10,
        "a zombie is angry enough to attack, which is what makes the attitude rule mean something here: {}",
        zombie_type.aggression
    );
}

/// A frightened monster runs from the survivor instead of walking to its goal.
///
/// Reference: `monmove.cpp:489` turns the distance term of `monster::move`'s square scoring around when `is_fleeing`, so a monster whose attitude is `Flee` prefers the square farthest
/// from the survivor. The gate drives the **turn**: the same monster walks *towards* its goal with ten points of anger and *away* when its morale is negative and it is badly hurt.
#[test]
fn a_frightened_monster_runs_from_the_survivor_instead_of_walking_to_its_goal() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(37));
    let mut level = Level::new();
    for x in 0..9 {
        for y in 0..5 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    // A terrain table with a move cost for the floor: without one, every step costs nothing because `square_cost` answers "no prototype" and the router answers "no route".
    // That is what the first version of this gate proved - two monsters standing perfectly still next to a survivor they could see.
    let mut terrain = cdda_sim::terrain_table::TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain,
            furniture: cdda_sim::terrain_table::TerrainTable::new(),
        },
    );
    let survivor = cdda_sim::spawn_player(&mut world, (4, 2), 80, 0);

    let distance_to_survivor = |world: &World, entity: Entity| {
        let place = world.get::<cdda_sim::Position>(entity).expect("it has a place");
        (place.x - 4).abs() + (place.y - 2).abs()
    };

    // Angry and healthy: it walks to its goal, which is straight past the survivor.
    let bold = cdda_sim::creature::spawn_creature(&mut world, "the bold", (1, 2, 0));
    world
        .entity_mut(bold)
        .insert(cdda_sim::creature::Health { hp: 80, max_hp: 80 })
        .insert(cdda_sim::creature::Morale(10))
        .insert(cdda_sim::creature::Anger(100));
    cdda_sim::creature::set_goal(&mut world, bold, (7, 2, 0));
    cdda_sim::creature::set_speed(&mut world, bold, 100, 0);
    let before = distance_to_survivor(&world, bold);

    // Frightened and badly hurt: negative morale, and more than two thirds of its hit points given up.
    // Next to the survivor but with room behind it: the mirrored square has to be **on the map**, or the router answers "no route" and the monster stands still for a reason
    // that has nothing to do with being frightened.
    let scared = cdda_sim::creature::spawn_creature(&mut world, "the scared", (6, 1, 0));
    world
        .entity_mut(scared)
        .insert(cdda_sim::creature::Health { hp: 10, max_hp: 80 })
        .insert(cdda_sim::creature::Morale(-10))
        .insert(cdda_sim::creature::Anger(5));
    cdda_sim::creature::set_goal(&mut world, scared, (0, 1, 0));
    cdda_sim::creature::set_speed(&mut world, scared, 100, 0);
    let scared_before = (world.get::<cdda_sim::Position>(scared).expect("a place").x, world.get::<cdda_sim::Position>(scared).expect("a place").y);

    for _ in 0..4 {
        cdda_sim::turn::run_turn(&mut world);
    }

    let after = distance_to_survivor(&world, bold);
    assert!(
        after < before || world.get::<cdda_sim::creature::Goal>(bold).is_none(),
        "the bold monster closes on the survivor: {before} then {after}"
    );
    let scared_after = world.get::<cdda_sim::Position>(scared).expect("a place");
    assert!(
        (scared_after.x, scared_after.y) != scared_before,
        "the frightened one moved"
    );
    assert!(
        (scared_after.x - 4).abs() + (scared_after.y - 2).abs() > (scared_before.0 - 4).abs() + (scared_before.1 - 2).abs(),
        "and moved away: {scared_before:?} then {:?}",
        (scared_after.x, scared_after.y)
    );
    let _ = survivor;
}

/// Whether a monster runs: the hundred-point vetoes, then the attitude, then the four-square rule for a follower.
///
/// Reference: `monster::is_fleeing` (`monster.cpp:1608-1618`). The four-square clause is the one worth pinning: a **following** monster is fleeing whenever it is close, which is how the
/// reference says *keep your distance* - and a port that read only `attitude == Flee` would have a follower walk right up to the survivor it wants to stay away from.
#[test]
fn a_monster_runs_when_it_is_frightened_or_when_a_follower_gets_too_close() {
    use cdda_sim::creature::is_fleeing;

    // The vetoes come first: plenty of anger or morale means it never runs, whatever else is true.
    assert!(!is_fleeing(1, 100, -10, 100, 1), "a hundred anger is fearless");
    assert!(!is_fleeing(1, 100, 100, 0, 1), "and a hundred morale too");

    // Attitude says flee, so it flees at any distance.
    assert!(is_fleeing(10, 80, -10, 5, 9), "negative morale and little anger");
    assert!(is_fleeing(10, 100, 10, 0, 9), "no anger and badly hurt");

    // Attitude says follow, so distance decides: four squares and closer is running, five is not.
    assert!(is_fleeing(100, 100, 10, 5, 1), "a follower one square away is running");
    assert!(is_fleeing(100, 100, 10, 5, 4), "four squares is still too close");
    assert!(!is_fleeing(100, 100, 10, 5, 5), "five is far enough to follow");

    // And an attacker never runs.
    assert!(!is_fleeing(100, 100, 10, 50, 1), "a furious monster does not back off");
}

/// A monster gets its nerve back a point at a time, and five at a time while it is still negative.
///
/// Reference: `monster::process_turn` (`monster.cpp:3493-3512`), in the reference's order: the fleeing monster snaps back to its type's values, then one point of morale and one of anger,
/// and then - only if still negative and still below the type's value - five more, clamped to the type's value.
#[test]
fn a_monster_gets_its_nerve_back_a_point_at_a_time() {
    use cdda_sim::creature::{Nerve, regen_nerve};

    let plain = Nerve { morale: 10, anger: 20, regenerates: true };
    // Not regenerating, or wounded: nothing changes.
    let still = Nerve { regenerates: false, ..plain };
    assert_eq!(regen_nerve(100, 100, -30, 0, still, false), (-30, 0), "a type that does not regenerate never calms");
    assert_eq!(regen_nerve(99, 100, -30, 0, plain, false), (-30, 0), "and a wounded monster does not calm down");

    // Positive morale below the type's value: exactly one point.
    assert_eq!(regen_nerve(100, 100, 5, 15, plain, false), (6, 16), "one point each");

    // Negative morale: one point, then five more, then clamped at the type's value.
    assert_eq!(regen_nerve(100, 100, -30, -30, plain, false), (-24, -24), "six points of recovery from -30");
    // From three below zero: one point to -2, then five more, and the five lands at +3 rather than stopping at zero - the reference clamps to the **type's** value, not to zero.
    assert_eq!(regen_nerve(100, 100, -3, -3, plain, false), (3, 3), "the jump lands where the arithmetic puts it");
    // And it does clamp when the jump would pass the type's own value.
    assert_eq!(
        regen_nerve(100, 100, -3, -3, Nerve { morale: 1, anger: 1, regenerates: true }, false),
        (1, 1),
        "the five-point jump is clamped to the type's value"
    );
    assert_eq!(regen_nerve(100, 100, 5, 5, Nerve { morale: 1, anger: 1, regenerates: true }, false), (5, 5), "already above the type's value: nothing to regain");

    // Fleeing at full health: straight back to the type's own nerve, and no further arithmetic to do.
    assert_eq!(regen_nerve(100, 100, -50, -50, plain, true), (10, 20), "a fleeing monster at full health snaps back");
}

/// The turn gives a monster its nerve back, and a wounded one keeps its fear.
///
/// Reference: `monster::process_turn` runs the regrowth block for every monster each turn (`monster.cpp:3493-3512`), gated on `type->regen_morale && hp >= type->hp`. The gate drives the
/// **stage**: a calm monster below its type's morale gains a point, and the same monster with a scratch does not.
#[test]
fn the_turn_gives_a_monster_its_nerve_back() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(41));
    let mut level = Level::new();
    for x in 0..5 {
        for y in 0..5 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);

    let calm = cdda_sim::creature::spawn_creature(&mut world, "the calm", (3, 3, 0));
    world
        .entity_mut(calm)
        .insert(cdda_sim::creature::Health { hp: 80, max_hp: 80 })
        .insert(cdda_sim::creature::Morale(10))
        .insert(cdda_sim::creature::Anger(10))
        .insert(cdda_sim::creature::Nerve { morale: 20, anger: 20, regenerates: true });

    // The wounded one's nerve is **already at its type's value**, so the drift rule of round 1131 cannot move it: this gate is about the regrowth gate, and a fixture that let drift fire
    // would answer "did it calm down?" with two rules at once.
    let hurt = cdda_sim::creature::spawn_creature(&mut world, "the hurt", (1, 3, 0));
    world
        .entity_mut(hurt)
        .insert(cdda_sim::creature::Health { hp: 79, max_hp: 80 })
        .insert(cdda_sim::creature::Morale(10))
        .insert(cdda_sim::creature::Anger(10))
        .insert(cdda_sim::creature::Nerve { morale: 10, anger: 10, regenerates: true });

    cdda_sim::turn::run_turn(&mut world);

    // Regrowth gives one, and the drift of round 1131 may give another: eleven or twelve, never ten.
    let calm_morale = world
        .get::<cdda_sim::creature::Morale>(calm)
        .map(|carried| carried.0);
    assert!(
        matches!(calm_morale, Some(11 | 12)),
        "a point of morale, and one more if the drift fires: {calm_morale:?}"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Morale>(hurt).map(|carried| carried.0),
        Some(10),
        "a wounded monster does not calm down - one hit point short is short"
    );
}

/// A cornered fighter loses its nerve when another monster crowds it, and finds it again when it has been spooked before.
///
/// Reference: the `CORNERED_FIGHTER` block in `monster::process_turn` (`monster.cpp:3518-3549`): a monster with the flag, morale at zero or less, and another monster within two squares gets
/// `spooked` for three turns and `spooked_recent` for nine - and if it already carries `spooked_recent`, its morale and anger snap back to its type's values instead, which is what lets a
/// cornered animal turn and fight.
#[test]
fn a_cornered_fighter_loses_its_nerve_and_then_finds_it_again() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(43));
    let mut level = Level::new();
    for x in 0..7 {
        for y in 0..7 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let spooked = cdda_json::parse(br#"{"id":"spooked","max_duration":"1 hours"}"#).expect("valid json");
    let recent = cdda_json::parse(br#"{"id":"spooked_recent","max_duration":"1 hours"}"#).expect("valid json");
    let mut kinds = Vec::new();
    for (id, value) in [("spooked", spooked), ("spooked_recent", recent)] {
        let cdda_json::Value::Object(entry) = value else {
            panic!("an effect type is an object")
        };
        kinds.push(cdda_content::effects::read_one(id, &entry));
    }
    cdda_sim::effect_registry::set_effect_types(&mut world, &kinds);

    let crowded = cdda_sim::creature::spawn_creature(&mut world, "the cornered one", (2, 2, 0));
    let neighbour = cdda_sim::creature::spawn_creature(&mut world, "the neighbour", (3, 3, 0));
    let alone = cdda_sim::creature::spawn_creature(&mut world, "the one on its own", (6, 6, 0));
    for entity in [crowded, neighbour, alone] {
        world
            .entity_mut(entity)
            .insert(cdda_sim::creature::Morale(0))
            .insert(cdda_sim::creature::Anger(5))
            .insert(cdda_sim::creature::Nerve { morale: 20, anger: 20, regenerates: false })
            .insert(cdda_sim::creature_flags::CreatureFlags(
                ["CORNERED_FIGHTER".to_owned()].into_iter().collect(),
            ));
    }

    cdda_sim::turn::run_turn(&mut world);
    assert!(
        cdda_sim::applied_effects::has(&world, crowded, "spooked"),
        "a cornered fighter with a neighbour within two squares is spooked"
    );
    assert!(
        cdda_sim::applied_effects::has(&world, crowded, "spooked_recent"),
        "and remembers it for longer"
    );
    assert!(
        !cdda_sim::applied_effects::has(&world, alone, "spooked"),
        "one on its own is not cornered"
    );
    assert!(
        cdda_sim::applied_effects::has(&world, neighbour, "spooked"),
        "the neighbour is within two squares of a monster too, so it is spooked as well - which is what makes this a rule about crowding rather than about one unlucky monster"
    );

    // Now that it has been spooked before, the next turn snaps its nerve back to its type's values instead of spooking it again.
    let _ = cdda_sim::applied_effects::remove(&mut world, crowded, "spooked");
    if let Some(mut morale) = world.get_mut::<cdda_sim::creature::Morale>(crowded) {
        morale.0 = -30;
    }
    cdda_sim::turn::run_turn(&mut world);
    assert_eq!(
        world.get::<cdda_sim::creature::Morale>(crowded).map(|carried| carried.0),
        Some(20),
        "a monster that was spooked recently snaps back to its type's morale and fights"
    );
}

/// A cornered fighter is daunted by the survivor standing close, even with no other monster in sight.
///
/// Reference: the second half of the `CORNERED_FIGHTER` block (`monster.cpp:3534-3548`) - a **character** within two squares whose attitude the monster would only follow or flee from
/// spooks it. A monster that would attack is not daunted by the same proximity, which is what the second half of this gate pins: the branch is about *wanting to be elsewhere*, not about
/// distance alone.
#[test]
fn a_cornered_fighter_is_daunted_by_the_survivor_standing_close() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(47));
    let mut level = Level::new();
    for x in 0..8 {
        for y in 0..8 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let spooked = cdda_json::parse(br#"{"id":"spooked","max_duration":"1 hours"}"#).expect("valid json");
    let recent = cdda_json::parse(br#"{"id":"spooked_recent","max_duration":"1 hours"}"#).expect("valid json");
    let mut kinds = Vec::new();
    for (id, value) in [("spooked", spooked), ("spooked_recent", recent)] {
        let cdda_json::Value::Object(entry) = value else {
            panic!("an effect type is an object")
        };
        kinds.push(cdda_content::effects::read_one(id, &entry));
    }
    cdda_sim::effect_registry::set_effect_types(&mut world, &kinds);
    cdda_sim::spawn_player(&mut world, (3, 3), 80, 0);

    // Two squares from the survivor and angry enough only to follow: daunted.
    let timid = cdda_sim::creature::spawn_creature(&mut world, "the timid one", (5, 3, 0));
    // The same distance away and angry enough to attack: not daunted.
    let bold = cdda_sim::creature::spawn_creature(&mut world, "the bold one", (1, 3, 0));
    for (entity, anger) in [(timid, 5), (bold, 50)] {
        world
            .entity_mut(entity)
            .insert(cdda_sim::creature::Morale(0))
            .insert(cdda_sim::creature::Anger(anger))
            .insert(cdda_sim::creature::Nerve { morale: 20, anger: 20, regenerates: false })
            .insert(cdda_sim::creature_flags::CreatureFlags(
                ["CORNERED_FIGHTER".to_owned()].into_iter().collect(),
            ));
    }

    cdda_sim::turn::run_turn(&mut world);

    assert!(
        cdda_sim::applied_effects::has(&world, timid, "spooked"),
        "a cornered fighter that would only follow is spooked by the survivor standing two squares away"
    );
    assert!(
        !cdda_sim::applied_effects::has(&world, bold, "spooked"),
        "and one that would attack is not - the branch is about wanting to be elsewhere, not about distance"
    );
}

/// A monster's morale drifts back towards its type's value, on two rolls and by one point.
///
/// Reference: `monster::process_turn` (`monster.cpp:1877-1885`): one in four turns, and then either a gap of more than fifteen - which always drifts - or another one in two.
///
/// The gate counts over **one advancing stream** rather than over fresh seeds, because every fresh stream's first answer is the same number: a test that reseeds per case is a test that
/// asks one question many times and calls it a sample.
#[test]
fn a_monster_s_morale_drifts_back_towards_its_type() {
    use cdda_sim::creature::drift_morale;

    // A gap of more than fifteen drifts on the first roll alone: about one turn in four. Thirty below fifty is a gap of twenty, which is the point of this fixture - the first version of
    // this gate used a gap of ten and measured the *small*-gap rate while claiming to be the large one.
    let mut rolls = Rolls::new(53);
    let mut drifted = 0;
    for _ in 0..400 {
        if drift_morale(-30, -50, &mut rolls) != -30 {
            drifted += 1;
        }
    }
    assert!(
        (70..=130).contains(&drifted),
        "a big gap drifts on roughly a quarter of turns: {drifted} of 400"
    );

    // A small gap has to pass both rolls, so it drifts on about one turn in eight.
    let mut rolls = Rolls::new(53);
    let mut drifted = 0;
    for _ in 0..400 {
        if drift_morale(-45, -50, &mut rolls) != -45 {
            drifted += 1;
        }
    }
    assert!(
        (25..=80).contains(&drifted),
        "a small gap drifts on roughly an eighth of turns: {drifted} of 400"
    );

    // A monster already at its type's value never moves, and the sign follows the gap.
    let mut rolls = Rolls::new(53);
    for _ in 0..50 {
        assert_eq!(drift_morale(10, 10, &mut rolls), 10, "no gap, no drift");
    }
    // And the sign always follows the gap, whichever way it points.
    let mut rolls = Rolls::new(59);
    let mut frightened_drifts = 0;
    for _ in 0..200 {
        match drift_morale(-70, -50, &mut rolls) {
            -70 => {}
            -69 => frightened_drifts += 1,
            other => panic!("a frightened monster only ever gains a point: {other}"),
        }
    }
    assert!(frightened_drifts > 0, "and it does gain them");
    let mut rolls = Rolls::new(59);
    let mut bold_drifts = 0;
    for _ in 0..200 {
        match drift_morale(-30, -50, &mut rolls) {
            -30 => {}
            -31 => bold_drifts += 1,
            other => panic!("a braver-than-type monster only ever loses a point: {other}"),
        }
    }
    assert!(bold_drifts > 0, "and it does lose them");
}

/// An effect the creature is wearing changes how many moves a turn gives it, by the effect data's own number.
///
/// Reference: `Character::get_speed_base` sums every effect's `SPEED` modifier into the speed bonus (`character.cpp:12488-12494`). The gate uses the **data's** `winded`, whose
/// `speed_mod` is `[-10]`: a survivor at speed one hundred banks ninety moves while winded and a hundred when it is not.
#[test]
fn an_effect_the_creature_wears_changes_how_many_moves_a_turn_gives_it() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(61));
    // The number lives under `base_mods`, which is where the loader looks for it: a top-level `speed_mod` is not read at all, and the first version of this gate proved that by
    // failing on its own assertion about the data rather than on the rule.
    let winded =
        cdda_json::parse(br#"{"id":"winded","base_mods":{"speed_mod":[-10]},"max_duration":"1 hours"}"#)
            .expect("valid json");
    let cdda_json::Value::Object(entry) = winded else {
        panic!("an effect type is an object")
    };
    let kind = cdda_content::effects::read_one("winded", &entry);
    assert_eq!(
        kind.mod_value("SPEED", cdda_content::effects::ModAction::Min, 0, 1),
        -10.0,
        "the data says a winded creature moves ten speed slower"
    );
    cdda_sim::effect_registry::set_effect_types(&mut world, std::slice::from_ref(&kind));

    let restless = cdda_sim::creature::spawn_creature(&mut world, "the restless", (1, 1, 0));
    let calm = cdda_sim::creature::spawn_creature(&mut world, "the calm", (2, 2, 0));
    for entity in [restless, calm] {
        cdda_sim::creature::set_speed(&mut world, entity, 100, 0);
    }
    cdda_sim::applied_effects::add(
        &mut world,
        restless,
        cdda_sim::effects::ActiveEffect::new(&kind, 10, 1),
    );

    cdda_sim::creature::bank_a_turn(&mut world);

    assert_eq!(
        cdda_sim::creature::moves_left(&world, restless),
        90,
        "a winded creature banks ninety moves"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, calm),
        100,
        "and one that is not banks a hundred"
    );
}

/// An effect the creature is wearing changes its dexterity, and the stat never goes below zero.
///
/// Reference: `Character::get_dex` is `std::max( 0, get_dex_base() + dex_bonus )` (`character.cpp:4390-4392`), and the bonus is where the worn effects' own `DEX` modifiers land. The gate
/// drives the **same door the accuracy rule uses** - `effective_dexterity` - so what it proves is that a drug which costs two dexterity costs the survivor two points of aim.
#[test]
fn an_effect_the_creature_wears_changes_its_dexterity() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(67));
    // An array in `base_mods` is **per body-part level**, not per intensity: index zero is the creature as a whole (`get_effect_modifier_key`, `effect.cpp:310-316`), and intensity scaling
    // comes from a separate `scaling_mods` table. The first version of this gate assumed otherwise and measured a flat effect when it meant a heavy one.
    let clumsy = cdda_json::parse(br#"{"id":"clumsy","base_mods":{"dex_mod":[-2]},"max_duration":"1 hours"}"#)
        .expect("valid json");
    let heavy = cdda_json::parse(br#"{"id":"made-of-lead","base_mods":{"dex_mod":[-10]},"max_duration":"1 hours"}"#)
        .expect("valid json");
    let cdda_json::Value::Object(entry) = clumsy else {
        panic!("an effect type is an object")
    };
    let kind = cdda_content::effects::read_one("clumsy", &entry);
    let cdda_json::Value::Object(heavy_entry) = heavy else {
        panic!("an effect type is an object")
    };
    let heavy_kind = cdda_content::effects::read_one("made-of-lead", &heavy_entry);
    assert_eq!(
        kind.mod_value("DEX", cdda_content::effects::ModAction::Min, 0, 1),
        -2.0,
        "the data says the effect costs two dexterity"
    );
    cdda_sim::effect_registry::set_effect_types(&mut world, &[kind.clone(), heavy_kind.clone()]);

    let steady = cdda_sim::creature::spawn_creature(&mut world, "the steady one", (1, 1, 0));
    let fumbling = cdda_sim::creature::spawn_creature(&mut world, "the fumbling one", (2, 2, 0));
    let wrecked = cdda_sim::creature::spawn_creature(&mut world, "the wrecked one", (3, 3, 0));
    for entity in [steady, fumbling, wrecked] {
        world.entity_mut(entity).insert(cdda_sim::creature::Dexterity { base: 8 });
    }
    for entity in [fumbling, wrecked] {
        cdda_sim::applied_effects::add(
            &mut world,
            entity,
            cdda_sim::effects::ActiveEffect::new(&kind, 10, 1),
        );
    }
    // The wrecked one wears something that costs ten, which takes eight dexterity past nothing.
    let _ = cdda_sim::applied_effects::remove(&mut world, wrecked, "clumsy");
    cdda_sim::applied_effects::add(
        &mut world,
        wrecked,
        cdda_sim::effects::ActiveEffect::new(&heavy_kind, 10, 1),
    );

    assert_eq!(
        cdda_sim::creature::effective_dexterity(&world, steady),
        8,
        "nothing worn, nothing lost"
    );
    assert_eq!(
        cdda_sim::creature::effective_dexterity(&world, fumbling),
        6,
        "two points of dexterity gone"
    );
    assert_eq!(
        cdda_sim::creature::effective_dexterity(&world, wrecked),
        0,
        "and the stat floors at zero rather than going negative"
    );
}

/// An effect the creature is wearing changes its strength, and the strength it is read through is the one carrying capacity uses.
///
/// Reference: `Character::get_str` is `std::max( 0, get_str_base() + str_bonus )` (`character.cpp:4385-4388`), and `str_bonus` is where the worn effects' `STR` modifiers land. The gate
/// asks through `effective_strength`, which is the door `carrying_capacity` reads, so what it proves is that a drug worth two strength is eight kilograms of carrying capacity.
#[test]
fn an_effect_the_creature_wears_changes_its_strength() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    singletons::put_rolls(&mut world, Rolls::new(71));
    let strong = cdda_json::parse(br#"{"id":"hopped-up","base_mods":{"str_mod":[2]},"max_duration":"1 hours"}"#)
        .expect("valid json");
    let cdda_json::Value::Object(entry) = strong else {
        panic!("an effect type is an object")
    };
    let kind = cdda_content::effects::read_one("hopped-up", &entry);
    assert_eq!(
        kind.mod_value("STR", cdda_content::effects::ModAction::Min, 0, 1),
        2.0,
        "the data says the effect is worth two strength"
    );
    cdda_sim::effect_registry::set_effect_types(&mut world, std::slice::from_ref(&kind));

    let plain = cdda_sim::creature::spawn_creature(&mut world, "the plain one", (1, 1, 0));
    let hopped = cdda_sim::creature::spawn_creature(&mut world, "the hopped-up one", (2, 2, 0));
    for entity in [plain, hopped] {
        world.entity_mut(entity).insert(cdda_sim::creature::Strength { base: 8 });
    }
    cdda_sim::applied_effects::add(
        &mut world,
        hopped,
        cdda_sim::effects::ActiveEffect::new(&kind, 10, 1),
    );

    assert_eq!(
        cdda_sim::creature::effective_strength(&world, plain),
        8,
        "nothing worn, nothing gained"
    );
    assert_eq!(
        cdda_sim::creature::effective_strength(&world, hopped),
        10,
        "two points of strength"
    );
    let plain_kilos = cdda_sim::creature::carrying_capacity(&world, plain);
    let hopped_kilos = cdda_sim::creature::carrying_capacity(&world, hopped);
    assert!(
        hopped_kilos > plain_kilos,
        "and the strength that is read is the one carrying capacity uses: {plain_kilos} g then {hopped_kilos} g"
    );
}

/// A weapon's wear chance, and the inverted guard that makes a low chance the worst case.
///
/// Reference: `Character::handle_melee_wear` (`melee.cpp:271-293`). Three claims, each one a different way to get it wrong: **strength divides** the chance, **`DURABLE_MELEE` multiplies** it by
/// four (a bigger `one_in` denominator is a tougher weapon), and a **chance of zero or below damages the weapon every time** - because the reference only returns early when there is a chance
/// *and* the die misses it.
#[test]
fn a_weapon_wears_by_its_own_arithmetic_and_a_zero_chance_is_the_worst_case() {
    use cdda_core_types::rng::Rolls;
    use cdda_sim::melee::{melee_wear_chance, wear_damages};

    // A dexterous, skilled, strong fighter with a chipping-resistant weapon: 10/2 + 3 + 64/max(12, 4) = 5 + 3 + 5.33 = 13.33, times 20 = 266.
    let strong = melee_wear_chance(10.0, 3, 12.0, 20.0, 1.0, 1.0, false);
    assert_eq!(strong, 266, "the arithmetic, truncated to an int");
    // A stronger arm **lowers** the chance, because the strength term is 64 over it.
    let stronger = melee_wear_chance(10.0, 3, 24.0, 20.0, 1.0, 1.0, false);
    assert!(stronger < strong, "a stronger arm divides the chance: {stronger} against {strong}");
    // A durable weapon multiplies it by four, which is the opposite direction from the way it reads.
    assert_eq!(melee_wear_chance(10.0, 3, 12.0, 20.0, 1.0, 1.0, true), strong * 4, "DURABLE_MELEE is four times tougher");
    // A wear multiplier divides, and a zero multiplier means the weapon cannot wear at all.
    assert!(melee_wear_chance(10.0, 3, 12.0, 20.0, 2.0, 1.0, false) < strong, "a higher wear multiplier is a softer weapon");
    assert_eq!(melee_wear_chance(10.0, 3, 12.0, 20.0, 0.0, 1.0, false), 0, "no wear multiplier, no chance");

    // The guard: a large chance almost never damages, a chance of one always does, and a chance of zero always does too.
    let mut dice = Rolls::new(1234);
    let broken = (0..2_000).filter(|_| wear_damages(500, &mut dice)).count();
    assert!(broken < 40, "one in five hundred: {broken} of 2000");
    let mut dice = Rolls::new(1234);
    assert!((0..200).all(|_| wear_damages(1, &mut dice)), "a chance of one damages every time");
    let mut dice = Rolls::new(1234);
    assert!((0..200).all(|_| wear_damages(0, &mut dice)), "and so does a chance of zero, which is the reference's own quirk");
}

/// A swing dents the thing in the hand - a soft one every time, a rock hardly ever.
///
/// Reference: `Character::melee_attack` calls `handle_melee_wear( cur_weapon )` **after** the blow and outside the hit branch (`melee.cpp:930`), so a miss wears a weapon too, and `item::inc_damage`
/// raises the damage by one thousandth scale (`item.cpp:9467-9470`). **The soft case is deterministic and that is why the gate can use it**: `chip_resistance` zero makes the chance zero, and the
/// reference's inverted guard damages the weapon **every** time - so a weapon the arithmetic called indestructible is the fastest-wearing one in the game.
#[test]
fn a_swing_dents_the_thing_in_the_hand() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(4));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world
        .entity_mut(zombie)
        .insert(cdda_sim::creature::Health { hp: 900, max_hp: 900 });
    // A weapon of a kind that resists chipping not at all, taken in hand as a thing.
    // A turn's worth of moves, because taking a heavy thing in hand costs some and a creature with none cannot.
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    for _ in 0..2 {
        cdda_sim::creature::bank_a_turn(&mut world);
    }
    let weapon = world
        .spawn((
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::ItemDamage::default(),
            cdda_sim::creature::Carried(survivor),
        ))
        .id();
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, weapon, 250),
        cdda_sim::creature::Wielded::Done
    );
    cdda_sim::item_registry::set_item_chip_resistance(&mut world, &[(cdda_sim::ItemTypeId(7), 0)]);
    let damage_of = |world: &bevy_ecs::prelude::World| {
        world.get::<cdda_sim::ItemDamage>(weapon).map_or(-1, |damage| damage.damage)
    };
    assert_eq!(damage_of(&world), 0, "a new weapon is undamaged");

    let dealt = cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert!(dealt.is_some(), "the swing happened at all");
    assert_eq!(damage_of(&world), 1000, "and cost the weapon one thousandth scale");

    // The same swing twice more: three swings, three thousandths, because a zero chance is the worst case and not the best.
    for _ in 0..2 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    }
    assert_eq!(damage_of(&world), 3000, "three swings, three dents");

    // And a weapon that resists chipping well keeps its edge across the same three swings most of the time.
    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(4));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world
        .entity_mut(zombie)
        .insert(cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 });
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    for _ in 0..2 {
        cdda_sim::creature::bank_a_turn(&mut world);
    }
    let weapon = world
        .spawn((
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::ItemDamage::default(),
            cdda_sim::creature::Carried(survivor),
        ))
        .id();
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, weapon, 250),
        cdda_sim::creature::Wielded::Done
    );
    cdda_sim::item_registry::set_item_chip_resistance(&mut world, &[(cdda_sim::ItemTypeId(7), 20)]);
    for _ in 0..20 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    }
    let dents = world.get::<cdda_sim::ItemDamage>(weapon).map_or(0, |damage| damage.damage) / 1000;
    assert!(dents < 5, "a chipping-resistant weapon takes few dents in twenty swings: {dents}");
}

/// A weapon taken past its maximum is destroyed by the blow, and the hand is empty.
///
/// Reference: `item::mod_damage` (`item.cpp:9449-9464`) is `destroy = ( damage_ + qty ) > max_damage()`, then `set_damage`, which clamps - so **the hit that takes an item exactly to its
/// maximum is not the hit that destroys it**, and the next one is. `Character::handle_melee_wear` then removes the item from the hand (`melee.cpp:293-330`).
#[test]
fn a_weapon_worn_past_its_maximum_is_destroyed_by_the_blow() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(11));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world.entity_mut(zombie).insert(cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 });
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    for _ in 0..2 {
        cdda_sim::creature::bank_a_turn(&mut world);
    }
    let weapon = world
        .spawn((
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::ItemDamage::default(),
            cdda_sim::creature::Carried(survivor),
        ))
        .id();
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, weapon, 250),
        cdda_sim::creature::Wielded::Done
    );
    // A weapon that resists chipping not at all, so every swing adds a thousandth scale and the maximum is reached in four.
    cdda_sim::item_registry::set_item_chip_resistance(&mut world, &[(cdda_sim::ItemTypeId(7), 0)]);

    for swing in 1..=4 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
        let alive = world.get_entity(weapon).is_ok();
        assert!(alive, "still there after swing {swing}");
    }
    // Four thousand is exactly the maximum, which is **not** destruction; the fifth is.
    assert_eq!(
        world.get::<cdda_sim::ItemDamage>(weapon).map(|damage| damage.damage),
        Some(4_000),
        "worn to its maximum and still in one piece"
    );
    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert!(world.get_entity(weapon).is_err(), "the fifth swing destroys it");
    assert!(
        cdda_sim::creature::wielding(&mut world, survivor).is_none(),
        "and the hand is empty afterwards"
    );
}

/// A weapon that has taken a blow gains the place a fault would live, and an untouched one has none.
///
/// Reference: `item::mod_damage`'s `set_random_fault_of_type( "mechanical_damage" )` (`item.cpp:9458-9461`) writes into `item::faults`, which starts empty. **The port stops one step short of the
/// reference here on purpose**: the pick needs the kind's weighted fault list installed, and nothing installs it yet, so the swing gives the item its empty fault set and no invented fault.
#[test]
fn a_blow_leaves_room_for_a_fault_even_before_one_is_picked() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(3));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world.entity_mut(zombie).insert(cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 });
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    for _ in 0..2 {
        cdda_sim::creature::bank_a_turn(&mut world);
    }
    let weapon = world
        .spawn((
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::ItemDamage::default(),
            cdda_sim::creature::Carried(survivor),
        ))
        .id();
    assert!(world.get::<cdda_sim::ItemFaults>(weapon).is_none(), "a fresh weapon carries no fault set");
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, weapon, 250),
        cdda_sim::creature::Wielded::Done
    );
    cdda_sim::item_registry::set_item_chip_resistance(&mut world, &[(cdda_sim::ItemTypeId(7), 0)]);
    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    let faults = world.get::<cdda_sim::ItemFaults>(weapon).expect("the weapon can now carry faults");
    assert!(faults.0.is_empty(), "but nothing invented one: the pick is not wired to a kind's list yet");
}

/// A weapon damaged by a blow catches a mechanical fault, and a kind with no faults catches none.
///
/// Reference: `item::mod_damage` ends with `set_random_fault_of_type( "mechanical_damage" )` (`item.cpp:9458-9461`), which keeps only the type's faults **of that type** that the item can have and
/// picks one by weight. **Both halves are in one gate**, because a port that always added a fault and a port that never did would each pass half of it.
#[test]
fn a_damaged_weapon_catches_a_mechanical_fault() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let swing_once = |faults: &[(&str, &str)], weights: Vec<(String, i32)>| {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(21));
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
        world.entity_mut(zombie).insert(cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 });
        world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
        for _ in 0..2 {
            cdda_sim::creature::bank_a_turn(&mut world);
        }
        let weapon = world
            .spawn((
                cdda_sim::GroundItem {
                    kind: cdda_sim::ItemTypeId(7),
                    charges: 1,
                    age: 0,
                    burnt: 0,
                    damage: 0,
                    burn: None,
                },
                cdda_sim::ItemDamage::default(),
                cdda_sim::creature::Carried(survivor),
            ))
            .id();
        assert_eq!(
            cdda_sim::creature::wield(&mut world, survivor, weapon, 250),
            cdda_sim::creature::Wielded::Done
        );
        // Soft enough that every swing damages it, and the kind's own fault list and the catalogue's types.
        cdda_sim::item_registry::set_item_chip_resistance(&mut world, &[(cdda_sim::ItemTypeId(7), 0)]);
        cdda_sim::item_registry::set_item_fault_weights(&mut world, &[(cdda_sim::ItemTypeId(7), weights)]);
        let typed: Vec<(String, String)> = faults.iter().map(|(fault, kind)| ((*fault).to_owned(), (*kind).to_owned())).collect();
        cdda_sim::item_registry::set_fault_types(&mut world, &typed);
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
        world.get::<cdda_sim::ItemFaults>(weapon).map(|faults| faults.0.clone()).unwrap_or_default()
    };

    // One mechanical fault at full weight: it always comes back.
    let caught = swing_once(&[("dented", "mechanical_damage")], vec![("dented".to_owned(), 100)]);
    assert!(caught.contains("dented"), "the weapon caught the dent: {caught:?}");

    // A fault of another type is not eligible, so nothing is caught.
    let wrong_type = swing_once(&[("dented", "wet")], vec![("dented".to_owned(), 100)]);
    assert!(wrong_type.is_empty(), "a wet fault is not mechanical damage: {wrong_type:?}");

    // A kind with no faults at all catches none.
    let none = swing_once(&[("dented", "mechanical_damage")], Vec::new());
    assert!(none.is_empty(), "no list, no fault: {none:?}");
}

/// A worn weapon says so, in the log the store keeps - once when it is damaged and once when it breaks.
///
/// Reference: `Character::handle_melee_wear` prints `"Your %s is damaged by the force of the blow!"` for a weapon that survives (`melee.cpp:295-297`) and
/// `"Your %s is destroyed by the blow!"` when it does not (`:328-330`). The port keeps both in `messages::MessageLog`, which is the component a rule writes and the interface drains - **not a
/// `println!`**, because a rule that prints cannot be asserted on.
#[test]
fn a_worn_weapon_says_so_in_the_log() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(8));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world.entity_mut(zombie).insert(cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 });
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    for _ in 0..2 {
        cdda_sim::creature::bank_a_turn(&mut world);
    }
    let weapon = world
        .spawn((
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::ItemDamage::default(),
            cdda_sim::creature::Carried(survivor),
        ))
        .id();
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, weapon, 250),
        cdda_sim::creature::Wielded::Done
    );
    cdda_sim::item_registry::set_item_chip_resistance(&mut world, &[(cdda_sim::ItemTypeId(7), 0)]);

    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    let said = cdda_sim::messages::lines(&world);
    assert!(
        said.iter().any(|line| line.contains("damaged by the force of the blow")),
        "the first swing only damages it: {said:?}"
    );

    // Three more swings take it to its maximum and the fifth destroys it, which is the other message.
    for _ in 0..4 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    }
    let said = cdda_sim::messages::lines(&world);
    assert!(
        said.iter().any(|line| line.contains("destroyed by the blow")),
        "and the last one breaks it: {said:?}"
    );
}

/// A fragile weapon that breaks apart yields most of its parts and destroys the weakest half of the time.
///
/// Reference: `Character::handle_melee_wear`'s `FRAGILE_MELEE` branch (`melee.cpp:301-330`): every component of the broken weapon is rolled, and the roll is `one_in( 2 )` for the **weakest**
/// component and `one_in( 8 )` for the rest - **the weak part is four times as likely to be lost as any other**, which is the whole reason the data marks a weakest part at all.
#[test]
fn a_broken_fragile_weapon_loses_its_weakest_part_four_times_as_often() {
    use cdda_core_types::rng::Rolls;
    use cdda_sim::melee::break_apart;

    let parts = vec![
        ("handle".to_owned(), false),
        ("string".to_owned(), true),
        ("limb".to_owned(), false),
    ];
    let mut dice = Rolls::new(4242);
    let (mut weak_lost, mut others_lost, mut others_total) = (0, 0, 0);
    for _ in 0..4_000 {
        let (kept, destroyed) = break_apart(&parts, &mut dice);
        assert_eq!(kept.len() + destroyed.len(), 3, "every part is either kept or destroyed");
        if destroyed.contains(&"string".to_owned()) {
            weak_lost += 1;
        }
        for part in ["handle", "limb"] {
            others_total += 1;
            if destroyed.contains(&part.to_owned()) {
                others_lost += 1;
            }
        }
    }
    // One in two against one in eight: about 2000 of 4000 against about 1000 of 8000.
    assert!((1_800..2_200).contains(&weak_lost), "the weakest part goes about half the time: {weak_lost} of 4000");
    let ratio = f64::from(others_lost) / f64::from(others_total);
    assert!((0.10..0.15).contains(&ratio), "the others go about one in eight: {ratio:.3} of {others_total}");
}

/// The weak part is the least chip-resistant one that counts, and the big part is the largest one - and cloth never counts.
///
/// Reference: `Character::handle_melee_wear`'s `FRAGILE_MELEE` prologue (`melee.cpp:255-270`). Two choices, each with a rule that is easy to lose:
///
/// * **the blacklist** - `sheet_cotton`, `leather` and `fur` are skipped entirely, because a grip or a strap "should have no bearing on durability";
/// * **strict comparison** - `weak_chip > comp.chip_resistance()` and `comp.volume() > big_vol` mean **the first of equal candidates wins**, and a part of zero volume never becomes the big one.
#[test]
fn the_weak_part_and_the_big_part_are_chosen_the_reference_way() {
    use cdda_sim::melee::{PartFacts, weak_and_big_parts};

    let parts = [
        PartFacts { kind: "sheet_cotton", chip_resistance: 1, volume_ml: 50 },
        PartFacts { kind: "steel_chunk", chip_resistance: 20, volume_ml: 250 },
        PartFacts { kind: "pipe", chip_resistance: 12, volume_ml: 250 },
        PartFacts { kind: "scrap", chip_resistance: 12, volume_ml: 0 },
    ];
    let (weak, big) = weak_and_big_parts(&parts);
    assert_eq!(weak, Some("pipe"), "the least resistant that is not blacklisted - and the first of two equal ones");
    assert_eq!(big, Some("steel_chunk"), "the largest, first of equal volumes, and never a zero-volume part");

    // Cloth is the least resistant thing here and it still does not count: the blacklist is a rule, not a tie-breaker.
    let cloth_only = [
        PartFacts { kind: "sheet_cotton", chip_resistance: 1, volume_ml: 50 },
        PartFacts { kind: "fur", chip_resistance: 2, volume_ml: 200 },
    ];
    assert_eq!(weak_and_big_parts(&cloth_only), (None, None), "nothing that counts is left to choose");

    // And a part with no volume is never the big one, even alone: `big_vol` starts at zero and the test is strict.
    let flat = [PartFacts { kind: "scrap", chip_resistance: 5, volume_ml: 0 }];
    assert_eq!(weak_and_big_parts(&flat), (Some("scrap"), None), "the weak part is still chosen");
}

/// A fragile weapon wears at its weakest part, six times over - and at its own material when no part counts.
///
/// Reference: `handle_melee_wear`'s fragile prologue (`melee.cpp:272-273`): `material_factor = ( weak_chip < INT_MAX ? weak_chip : shield->chip_resistance() ) / fragile_factor;` with
/// `fragile_factor = 6.0f`. **The division is the whole point**: a weapon whose weakest part is resistance 12 wears as if it were resistance 2, which is why a well-made blade with one cheap bolt
/// falls apart - the reference calls these *"fragile items that fall apart easily when used as a weapon due to poor construction quality"* (`:254-255`).
#[test]
fn a_fragile_weapon_wears_at_its_weakest_part_six_times_over() {
    use cdda_sim::melee::{PartFacts, fragile_material_factor};

    let parts = [
        PartFacts { kind: "pipe", chip_resistance: 12, volume_ml: 250 },
        PartFacts { kind: "steel_chunk", chip_resistance: 20, volume_ml: 250 },
    ];
    assert!((fragile_material_factor(&parts, 30.0) - 2.0).abs() < 1e-9, "12 / 6, and the weapon's own 30 never enters");
    // **The fallback is for a weapon with nothing that counts**, and only then: the reference's `weak_chip` stays `INT_MAX` exactly when every part was blacklisted or there were none.
    let only_cloth = [PartFacts { kind: "leather", chip_resistance: 1, volume_ml: 90 }];
    assert!((fragile_material_factor(&only_cloth, 30.0) - 30.0).abs() < 1e-9, "cloth gives no factor, so the weapon answers for itself");
    assert!((fragile_material_factor(&[], 30.0) - 30.0).abs() < 1e-9, "and so does a weapon with no parts at all");
}

/// The swing uses the weapon's weakest part when the weapon is fragile, and its own material when it is not.
///
/// Reference: `Character::handle_melee_wear` (`melee.cpp:246-274`): `material_factor` is the `FRAGILE_MELEE` computation for a fragile weapon and the weapon's own `chip_resistance()` otherwise.
/// **A part list that is never read changes nothing**: before this gate, a fragile weapon wore at its own resistance and the whole breakdown machinery was decoration.
#[test]
fn a_fragile_weapon_wears_at_its_weakest_part_when_it_is_swung() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    let whatever = cdda_sim::ItemTypeId(1);
    let weak = cdda_sim::ItemTypeId(2);
    let strong = cdda_sim::ItemTypeId(3);
    // **One installer for all three kinds**: `install_kind_facts` writes the chip table too, so setting it first would be undone.
    let facts = |flags: Vec<String>, chip: i32| cdda_sim::item_registry::KindFacts {
        flags,
        qualities: Vec::new(),
        chip_resistance: chip,
        faults: Vec::new(),
    };
    cdda_sim::item_registry::install_kind_facts(
        &mut world,
        &[
            (whatever, facts(vec!["FRAGILE_MELEE".to_owned()], 30)),
            (weak, facts(Vec::new(), 6)),
            (strong, facts(Vec::new(), 40)),
        ],
    );
    assert!(cdda_sim::item_registry::kind_has_flag(&world, whatever, "FRAGILE_MELEE"), "the weapon's kind is the fragile one");
    let weapon = world.spawn(cdda_sim::GroundItem { kind: whatever, charges: 1, age: 0, burnt: 0, damage: 0, burn: None }).id();
    let weak_part = world.spawn(cdda_sim::GroundItem { kind: weak, charges: 1, age: 0, burnt: 0, damage: 0, burn: None }).id();
    let strong_part = world.spawn(cdda_sim::GroundItem { kind: strong, charges: 1, age: 0, burnt: 0, damage: 0, burn: None }).id();
    cdda_sim::item_parts::add_part(&mut world, weapon, weak_part);
    cdda_sim::item_parts::add_part(&mut world, weapon, strong_part);

    assert!(
        (cdda_sim::creature::weapon_material_factor(&mut world, weapon) - 1.0).abs() < 1e-9,
        "6 / 6: the weakest part is what it wears at, not the weapon's own 30"
    );

    // The same weapon without parts answers for itself, which is the reference's `weak_chip == INT_MAX` fallback.
    let bare = world.spawn(cdda_sim::GroundItem { kind: whatever, charges: 1, age: 0, burnt: 0, damage: 0, burn: None }).id();
    assert!(
        (cdda_sim::creature::weapon_material_factor(&mut world, bare) - 30.0).abs() < 1e-9,
        "no parts, no factor"
    );
}

/// A fragile weapon does not just break: it comes apart, and it says so - and the parts that survive stay on the ground as themselves.
///
/// Reference: `handle_melee_wear`'s destruction branch (`melee.cpp:313-340`): a weapon with `FRAGILE_MELEE` **and** components prints *"Your %s breaks apart!"* instead of *"is destroyed by the
/// blow!"*, then rolls each component and **drops what survives**. The port must not print the ordinary destruction line for such a weapon - **that would tell the player their parts vanished when
/// they did not**.
#[test]
fn a_fragile_weapon_breaks_apart_instead_of_being_destroyed() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let weapon = cdda_sim::ItemTypeId(1);
    let cloth = cdda_sim::ItemTypeId(2);
    let iron = cdda_sim::ItemTypeId(3);
    let facts = |flags: Vec<String>, chip: i32| cdda_sim::item_registry::KindFacts {
        flags,
        qualities: Vec::new(),
        chip_resistance: chip,
        faults: Vec::new(),
    };
    // Chip resistance zero makes the wear chance zero, and a chance of zero **always** damages - the reference's inverted guard (`melee.cpp:291`).
    cdda_sim::item_registry::install_kind_facts(
        &mut world,
        &[
            (weapon, facts(vec!["FRAGILE_MELEE".to_owned()], 0)),
            // **Both parts at zero chip resistance**, so the fragile factor is zero and the wear chance is zero - and a chance of zero always damages.
            (cloth, facts(Vec::new(), 0)),
            (iron, facts(Vec::new(), 0)),
        ],
    );
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(99));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world.entity_mut(zombie).insert(cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 });
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    for _ in 0..2 {
        cdda_sim::creature::bank_a_turn(&mut world);
    }
    let held = world
        .spawn((
            cdda_sim::GroundItem { kind: weapon, charges: 1, age: 0, burnt: 0, damage: 0, burn: None },
            cdda_sim::ItemDamage::default(),
            cdda_sim::creature::Carried(survivor),
        ))
        .id();
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, held, 250),
        cdda_sim::creature::Wielded::Done
    );
    let cloth_part = world
        .spawn(cdda_sim::GroundItem { kind: cloth, charges: 1, age: 0, burnt: 0, damage: 0, burn: None })
        .id();
    let iron_part = world
        .spawn(cdda_sim::GroundItem { kind: iron, charges: 1, age: 0, burnt: 0, damage: 0, burn: None })
        .id();
    cdda_sim::item_parts::add_part(&mut world, held, cloth_part);
    cdda_sim::item_parts::add_part(&mut world, held, iron_part);

    // Five blows: the fifth takes the weapon past its maximum. Swings do not cost moves on this path, which is why the log-based gate needs no banking between them.
    for _ in 0..5 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    }
    let said = cdda_sim::messages::lines(&world);
    assert!(
        said.iter().any(|line| line.contains("breaks apart")),
        "the fragile weapon comes apart: {said:?}"
    );
    assert!(
        !said.iter().any(|line| line.contains("is destroyed by the blow")),
        "and it is not reported as an ordinary destruction: {said:?}"
    );
    assert!(!world.get_entity(held).is_ok_and(|_| true), "the weapon itself is gone");

    // **Every part is accounted for**: either it was destroyed (its line is in the log and its entity is gone) or it survived as itself, with no parent any more.
    let destroyed_lines = said.iter().filter(|line| line.contains("is destroyed!")).count();
    let mut gone = 0;
    let mut free = 0;
    for part in [cloth_part, iron_part] {
        if world.get_entity(part).is_ok() {
            free += 1;
            assert!(world.get::<cdda_sim::item_parts::PartOf>(part).is_none(), "a survivor is nobody's part any more");
        } else {
            gone += 1;
        }
    }
    assert_eq!(gone, destroyed_lines, "one line per part that is gone: {said:?}");
    assert_eq!(gone + free, 2, "two parts, each destroyed or free");
}

/// A thing can be picked up into a hand only when the hand is free or already holds the same kind.
///
/// Reference: `Character::has_wield_conflicts` (`character.cpp:8023-8026`): `is_wielding( it ) || ( is_armed() && !it.can_combine( weapon ) )`. **The second clause is the one that reads
/// backwards**: an armed character has no conflict when the new thing **can combine** with what is held - the same kind stacks into the same hand - and a conflict whenever it cannot.
#[test]
fn a_hand_takes_a_thing_when_it_is_free_or_holds_the_same_kind() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let stone = cdda_sim::ItemTypeId(1);
    let rock = cdda_sim::ItemTypeId(2);
    let spawn = |world: &mut World, kind| {
        world
            .spawn((
                cdda_sim::GroundItem { kind, charges: 1, age: 0, burnt: 0, damage: 0, burn: None },
                cdda_sim::ItemDamage::default(),
            ))
            .id()
    };
    let first = spawn(&mut world, stone);
    let same_kind = spawn(&mut world, stone);
    let other_kind = spawn(&mut world, rock);

    assert!(!cdda_sim::creature::has_wield_conflicts(&mut world, survivor, first), "empty hands take anything");
    // **The gate must not depend on the wield succeeding by luck**: wielding costs moves, so the turn is banked and the result asserted, or the three clauses below would be skipped and pass vacuously.
    // A thing is in a hand only once it is carried: the wield path refuses anything else (`Wielded::NotInHand`).
    world.entity_mut(first).insert(cdda_sim::creature::Carried(survivor));
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    cdda_sim::creature::bank_a_turn(&mut world);
    cdda_sim::creature::bank_a_turn(&mut world);
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, first, 250),
        cdda_sim::creature::Wielded::Done,
        "the fixture's wield has to happen, or the three rules below are never asked"
    );
    assert!(cdda_sim::creature::has_wield_conflicts(&mut world, survivor, first), "the same thing twice is a conflict");
    assert!(!cdda_sim::creature::has_wield_conflicts(&mut world, survivor, same_kind), "the same kind combines into the held hand");
    assert!(cdda_sim::creature::has_wield_conflicts(&mut world, survivor, other_kind), "a different kind does not");
}

/// The biggest part that survives a weapon's break goes into the hand.
///
/// Reference: `handle_melee_wear` (`melee.cpp:327-332`): `if( comp.typeId() == big_comp && !has_wield_conflicts( comp ) ) { wield( comp ); } else { drop it }`. **The hand is free by then** - the
/// weapon that held it has just been destroyed - so the biggest survivor is taken rather than dropped, and a smaller one is not.
///
/// The fixture is one part, so it is both the weakest and the biggest; the seed is probed first, so the test knows the part survives before it asks what happened to it.
#[test]
fn the_biggest_part_of_a_broken_weapon_goes_into_the_hand() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut seed = 1;
    loop {
        let mut probe = Rolls::new(seed);
        if !probe.one_in(2) {
            break;
        }
        seed += 1;
    }

    let mut world = World::new();
    let weapon = cdda_sim::ItemTypeId(1);
    let shaft = cdda_sim::ItemTypeId(2);
    let facts = |flags: Vec<String>, chip: i32| cdda_sim::item_registry::KindFacts {
        flags,
        qualities: Vec::new(),
        chip_resistance: chip,
        faults: Vec::new(),
    };
    cdda_sim::item_registry::install_kind_facts(
        &mut world,
        &[(weapon, facts(vec!["FRAGILE_MELEE".to_owned()], 0)), (shaft, facts(Vec::new(), 5))],
    );
    cdda_sim::item_registry::set_item_volumes(&mut world, &[(shaft, 500)]);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let held = world
        .spawn((
            cdda_sim::GroundItem { kind: weapon, charges: 1, age: 0, burnt: 0, damage: 0, burn: None },
            cdda_sim::ItemDamage::default(),
        ))
        .id();
    let part = world
        .spawn(cdda_sim::GroundItem { kind: shaft, charges: 1, age: 0, burnt: 0, damage: 0, burn: None })
        .id();
    cdda_sim::item_parts::add_part(&mut world, held, part);
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(seed));
    // Wielding costs moves, so the survivor banks two turns before the weapon breaks - otherwise the part is refused for a reason that has nothing to do with the rule.
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    cdda_sim::creature::bank_a_turn(&mut world);
    cdda_sim::creature::bank_a_turn(&mut world);

    assert!(
        cdda_sim::creature::break_apart_the_weapon(&mut world, held, survivor, "a fragile thing"),
        "the weapon comes apart"
    );
    assert!(world.get_entity(part).is_ok(), "the part survived the roll");
    assert_eq!(
        cdda_sim::creature::wielding(&mut world, survivor),
        Some(part),
        "**and it is in the hand**, because it was the biggest thing that came out"
    );
    assert!(world.get::<cdda_sim::item_parts::PartOf>(part).is_none(), "it is nobody's part any more");
}

/// A weapon that breaks drops what was inside it where the survivor stands.
///
/// Reference: `handle_melee_wear` spills the contents **before** the item is removed (`melee.cpp:305-312`), so a broken weapon's magazine, water bottle or holstered knife ends up on the floor rather
/// than ceasing to exist with the weapon. **The order is the rule**: spilling after the weapon is despawned would have nothing left to read the contents from.
#[test]
fn a_weapon_that_breaks_drops_what_was_inside_it() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let weapon = cdda_sim::ItemTypeId(1);
    let flask = cdda_sim::ItemTypeId(2);
    let facts = |chip: i32| cdda_sim::item_registry::KindFacts {
        flags: Vec::new(),
        qualities: Vec::new(),
        chip_resistance: chip,
        faults: Vec::new(),
    };
    cdda_sim::item_registry::install_kind_facts(&mut world, &[(weapon, facts(0)), (flask, facts(10))]);
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(11));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (5, 7, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (6, 7, 0));
    world.entity_mut(zombie).insert(cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 });
    world.entity_mut(survivor).insert(cdda_sim::creature::Speed { base: 100, bonus: 0 });
    for _ in 0..2 {
        cdda_sim::creature::bank_a_turn(&mut world);
    }
    let held = world
        .spawn((
            cdda_sim::GroundItem { kind: weapon, charges: 1, age: 0, burnt: 0, damage: 0, burn: None },
            cdda_sim::ItemDamage::default(),
            cdda_sim::creature::Carried(survivor),
        ))
        .id();
    assert_eq!(
        cdda_sim::creature::wield(&mut world, survivor, held, 250),
        cdda_sim::creature::Wielded::Done,
        "the fixture's weapon goes into the hand"
    );
    let inside = world
        .spawn(cdda_sim::GroundItem { kind: flask, charges: 1, age: 0, burnt: 0, damage: 0, burn: None })
        .id();
    assert!(cdda_sim::containment::put_inside(&mut world, inside, held), "the flask goes inside the weapon");

    // Five blows: the last destroys the weapon.
    for _ in 0..5 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    }
    assert!(world.get_entity(held).is_err(), "the weapon is gone");
    assert!(world.get_entity(inside).is_ok(), "**the flask is not**: it was spilled, not destroyed");
    assert!(
        world.get::<cdda_sim::containment::ContainedIn>(inside).is_none(),
        "and it is nobody's content any more"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(inside).map(|at| (at.x, at.y, at.z)),
        Some((5, 7, 0)),
        "it is on the square the survivor stood on"
    );
}

/// What a thing in hand is worth as a block: ten, six or four for a technique, two for something worn, nothing otherwise.
///
/// Reference: `melee::blocking_ability` (`melee.cpp:1950-1965`) - an `if / else if` chain **in that order**, so a thing with several technique flags gets the **best** of them, and a thing with no
/// technique but the `BLOCK_WHILE_WORN` flag is worth two. **The two is the trap**: a worn item is worth something in the hand and nothing at all when `best_shield` looks at the weapon (`:1970`),
/// because the flag's bonus only counts when the thing is actually worn.
#[test]
fn what_a_thing_is_worth_as_a_block() {
    use cdda_sim::melee::blocking_ability;

    assert_eq!(blocking_ability(&["WBLOCK_3"], false), 10, "the best technique");
    assert_eq!(blocking_ability(&["WBLOCK_2"], false), 6, "then six");
    assert_eq!(blocking_ability(&["WBLOCK_1"], false), 4, "then four");
    assert_eq!(blocking_ability(&[], true), 2, "and two for something worn with no technique");
    assert_eq!(blocking_ability(&[], false), 0, "nothing else blocks");
    assert_eq!(blocking_ability(&["WBLOCK_1", "WBLOCK_3"], false), 10, "**the chain takes the best**, whatever the order of the list");
}

/// Which thing a character blocks with: the best worn shield when it ties or beats the weapon, and never a worn bonus counted in the hand.
///
/// Reference: `Character::best_shield` (`melee.cpp:1967-1976`):
///
/// ```text
/// int best_value = blocking_ability( weapon );
/// best_value = best_value == 2 ? 0 : best_value;              // the worn flag is worth nothing in the hand
/// item_location best = best_value > 0 ? get_wielded_item() : none;
/// if( best_worn && blocking_ability( *best_worn ) >= best_value ) best = worn;
/// ```
///
/// **Two comparisons decide it, and both are easy to get wrong:** the `== 2` discards a weapon's worn bonus, and the worn test is `>=` - so **a worn shield that merely ties the weapon wins**.
#[test]
fn the_best_shield_is_the_worn_one_when_it_ties_or_beats_the_weapon() {
    use cdda_sim::melee::{Shield, best_shield};

    assert_eq!(best_shield(10, None), Shield::Weapon, "a technique weapon with nothing worn");
    assert_eq!(best_shield(10, Some(9)), Shield::Weapon, "a worse worn shield stays worn and is not used");
    assert_eq!(best_shield(10, Some(10)), Shield::Worn, "**a tie goes to the worn one**");
    assert_eq!(best_shield(10, Some(12)), Shield::Worn, "and so does anything better");
    assert_eq!(best_shield(2, None), Shield::None, "**a worn bonus in the hand is worth nothing** - two is discarded");
    assert_eq!(best_shield(2, Some(2)), Shield::Worn, "and against that zero, any worn shield wins");
    assert_eq!(best_shield(0, None), Shield::None, "nothing to block with");
}

/// Whether a character gets to block at all, and whether the attempt lands.
///
/// Reference: `Character::block_hit` (`melee.cpp:1983-2005`), whose first four steps decide before any block happens:
///
/// ```text
/// if( in_sleep_state() || has_effect( winded ) || ... ) return false;     // no die is rolled
/// if( blocks_left < 1 ) return false;                                      // no die is rolled
/// if( !x_in_y( melee_skill * 20.0 * get_limb_score( reaction ), 100 ) ) return false;
/// blocks_left--;
/// ```
///
/// **The order is the rule, in two ways that a player notices:** a sleeper or a winded character **spends no die and no block**, and a character who fails the roll **spends the die but not the
/// block** - `blocks_left--` sits *after* the roll, so a failed attempt is free and a successful one costs a block.
#[test]
fn a_block_is_attempted_only_by_someone_able_to_react_and_costs_only_when_it_lands() {
    use cdda_core_types::rng::Rolls;
    use cdda_sim::melee::{BlockAttempt, block_attempt};

    // **No die is spent by someone who cannot react at all**: the same seed's first roll is the same afterwards.
    let mut untouched = Rolls::new(7);
    let expected = untouched.rng(0, 99);
    let mut dice = Rolls::new(7);
    assert_eq!(block_attempt(false, 3, 5.0, 1.0, &mut dice), BlockAttempt::NotAttempted);
    assert_eq!(dice.rng(0, 99), expected, "the die a sleeper never rolled is still there");

    // No blocks left: the same, and no die.
    let mut untouched = Rolls::new(7);
    let expected = untouched.rng(0, 99);
    let mut dice = Rolls::new(7);
    assert_eq!(block_attempt(true, 0, 5.0, 1.0, &mut dice), BlockAttempt::NotAttempted, "no blocks left means no attempt");
    assert_eq!(dice.rng(0, 99), expected, "and no die spent");

    // A skill of five with a full reaction score is five times twenty in a hundred: certain.
    let mut dice = Rolls::new(1);
    assert_eq!(block_attempt(true, 2, 5.0, 1.0, &mut dice), BlockAttempt::Blocked);
    // And no skill at all never lands.
    let mut dice = Rolls::new(1);
    assert_eq!(block_attempt(true, 2, 0.0, 1.0, &mut dice), BlockAttempt::RollFailed);
}

/// A block score becomes a fraction of damage stopped, on the reference's own curve.
///
/// Reference: `logarithmic_range( min, max, pos )` (`cata_utility.cpp:161-193`): a logistic curve cut off at `±4`, clamped so **anything at or below the minimum stops everything and anything at or above
/// the maximum stops nothing** - the direction surprises because the score is a *strength*. `block_hit` calls it as `logarithmic_range( 0, 40, block_score )` and the reference documents what that
/// means in its own comment: **a beginner around 10 blocks about 15% of the damage, a proficient character around 20 about 45%, an expert in the high twenties about 80%**.
#[test]
fn a_block_score_becomes_a_fraction_of_the_damage_stopped() {
    use cdda_sim::melee::logarithmic_range;

    // The answer is **the fraction of damage that keeps coming**, so blocked = 1 - answer.
    let blocked = |score| 1.0 - logarithmic_range(0, 40, score);
    let beginner = blocked(10);
    assert!((0.08..0.20).contains(&beginner), "a beginner blocks about a tenth to a fifth: {beginner:.3}");
    let proficient = blocked(20);
    assert!((0.40..0.55).contains(&proficient), "a proficient character about 45%: {proficient:.3}");
    let expert = blocked(28);
    assert!((0.75..0.90).contains(&expert), "an expert about 80%: {expert:.3}");

    // The clamps, and the direction they fix: **a score at the minimum stops nothing, a score at the maximum stops everything.**
    assert!((logarithmic_range(0, 40, 0) - 1.0).abs() < 1e-9, "the minimum lets everything through");
    assert!((logarithmic_range(0, 40, -5) - 1.0).abs() < 1e-9, "and below it too");
    assert!(logarithmic_range(0, 40, 40).abs() < 1e-9, "the maximum stops everything");
    assert!(logarithmic_range(0, 40, 99).abs() < 1e-9, "and above it too");
    assert!(logarithmic_range(10, 10, 10).abs() < 1e-9, "an empty interval answers zero, as the reference's error path does");
}

/// A block spends its flat bonus first and then scales what is left, and only on physical damage that a melee blow deals.
///
/// Reference: `Character::block_hit`'s damage loop (`melee.cpp:2117-2145`): for each damage unit, if it is `physical && melee_only`, the **flat block bonus is taken off first**, and only then is the
/// remainder multiplied by the logistic fraction. **Two orderings live in five lines**: flat before multiplier (so a big flat bonus absorbs a blow whole and the multiplier never sees it), and per unit
/// in turn (so the flat bonus is spent by the first unit that needs it, not shared out).
#[test]
fn a_block_spends_its_flat_bonus_before_it_scales_anything() {
    use cdda_sim::melee::{DamageUnit, block_damage};

    // Two physical units and one that is not: flat 10 comes off the first two in turn.
    let mut units = vec![
        DamageUnit { physical: true, melee_only: true, amount: 8.0 },
        DamageUnit { physical: true, melee_only: true, amount: 9.0 },
        DamageUnit { physical: false, melee_only: true, amount: 5.0 },
    ];
    let blocked = block_damage(&mut units, 10.0, 0.5);
    assert!((units[0].amount - 0.0).abs() < 1e-9, "the first unit is absorbed whole by the flat bonus: 8 of 10");
    assert!((units[1].amount - 3.5).abs() < 1e-9, "the last 2 of the bonus come off the second, and half of the remaining 7 is blocked: {:.3}", units[1].amount);
    assert!((units[2].amount - 5.0).abs() < 1e-9, "**a non-physical unit is untouched**, however big the block");
    assert!((blocked - 8.0 - 2.0 - 3.5).abs() < 1e-9, "and the blocked total counts both halves: flat 10 plus the 3.5 the multiplier took");

    // Setting the flat bonus to zero leaves the multiplier alone to do the work.
    let mut units = vec![DamageUnit { physical: true, melee_only: true, amount: 10.0 }];
    let blocked = block_damage(&mut units, 0.0, 0.25);
    assert!((units[0].amount - 2.5).abs() < 1e-9, "a quarter gets through");
    assert!((blocked - 7.5).abs() < 1e-9, "and three quarters is blocked");
}

/// How much of a blow a block stopped, in words - on a ladder whose first word is the opposite of its condition.
///
/// Reference: `block_hit`'s description ladder (`melee.cpp:2149-2178`). The variable is called `blocked_ratio` but holds **the fraction that got through**: `( total_damage - damage_blocked ) /
/// total_damage`. Every threshold below is therefore read backwards - **less than 0.2 through means "nearly all" blocked** - and the first rung, `< epsilon`, is **"all"**, which a reader who trusts
/// the variable's name will get wrong. A total of zero also lands on "all", because the ratio is zero.
#[test]
fn how_much_of_a_blow_a_block_stopped_in_words() {
    use cdda_sim::melee::blocked_description;

    assert_eq!(blocked_description(0.0), "all", "nothing got through");
    assert_eq!(blocked_description(0.1), "nearly all", "a tenth got through");
    assert_eq!(blocked_description(0.2), "most", "**0.2 is not under 0.2**, so it is a rung down");
    assert_eq!(blocked_description(0.39), "most", "just under two fifths");
    assert_eq!(blocked_description(0.4), "a lot", "two fifths exactly moves a rung");
    assert_eq!(blocked_description(0.59), "a lot");
    assert_eq!(blocked_description(0.6), "some");
    assert_eq!(blocked_description(0.79), "some");
    assert_eq!(blocked_description(0.8), "a little", "four fifths through is still a little blocked");
    assert_eq!(blocked_description(0.99), "a little");
}

/// A counter-attack needs stamina and a weapon that is not mostly glass.
///
/// Reference: `block_hit`'s tail (`melee.cpp:2190-2198`): `if( get_stamina() < get_stamina_max() / 3 )` says the character is too exhausted, `else if( weapon.made_of( material_glass ) >
/// twenty_percent )` says the weapon is too fragile, `else` it counter-attacks. **Both comparisons are strict**, so a character on exactly a third of their stamina still counters, and a weapon exactly at
/// the glass threshold still counts as sound.
#[test]
fn a_counter_attack_needs_stamina_and_a_weapon_that_is_not_mostly_glass() {
    use cdda_sim::melee::{CounterAttack, counter_attack};

    // **Integer division**: 100 / 3 is 33, so 32 is under a third and 33 is not.
    assert_eq!(counter_attack(32, 100, 0.0, 100.0), CounterAttack::TooExhausted, "32 is under the third");
    assert_eq!(counter_attack(33, 100, 0.0, 100.0), CounterAttack::Yes, "**33 is exactly the third, and the test is strict**");
    assert_eq!(counter_attack(33, 99, 0.0, 100.0), CounterAttack::Yes, "**integer division**: 99 / 3 is exactly 33, so a third is not under a third");
    assert_eq!(counter_attack(100, 100, 25.0, 100.0), CounterAttack::TooFragile, "25 of 100 is over the fifth");
    assert_eq!(counter_attack(100, 100, 20.0, 100.0), CounterAttack::Yes, "**exactly a fifth is not over it**");
    assert_eq!(counter_attack(100, 100, 0.0, 0.0), CounterAttack::Yes, "a weapon with no materials has no glass in it");
}

/// A body's overkill decides what is left of it: a corpse with damage scaled to the blow, or a splatter.
///
/// Reference: `mdeath::normal` (`mondeath.cpp:42-68`) and `mdeath::splatter` (`mondeath.cpp:124-135`):
///
/// ```text
/// const int max_hp = std::max( z.get_hp_max(), 1 );
/// const float overflow_damage = std::max( -z.get_hp(), 0 );
/// const float corpse_damage = 2.5 * overflow_damage / max_hp;
/// const bool pulverized = corpse_damage > 5 && overflow_damage > z.get_hp_max();
/// damage = std::floor( corpse_damage * itype::damage_scale );   // damage_scale is 1000
/// ```
///
/// **The `&&` has a redundant half, and the gate proves it**: corpse damage is `2.5 * overkill / health`, so "damage above five" already means "overkill above twice the health", which
/// implies the reference's second test. The second test can therefore never change the answer - it is kept here as the reference states it, and the gate sweeps the range where the two
/// could disagree to show there is none.
#[test]
fn overkill_decides_what_is_left_of_a_body() {
    use cdda_sim::creature::{DeathRemains, death_remains};

    assert_eq!(
        death_remains(-40, 40),
        DeathRemains::Corpse { damage: 2500 },
        "**twice and a half the health over is a corpse at 2,500 damage**: 2.5 * 40 / 40 * 1,000"
    );
    assert_eq!(death_remains(0, 40), DeathRemains::Corpse { damage: 0 }, "a body killed exactly is undamaged");
    assert_eq!(death_remains(-120, 40), DeathRemains::Pulverized, "three times its health over is a splatter");

    // A body killed for two and a half times its health over is 6.25 of corpse damage, and pulverised.
    assert_eq!(death_remains(-100, 40), DeathRemains::Pulverized, "**two and a half times over splatters**");
    // And there is no health and overkill where the two halves of the `&&` disagree: damage above five *is* overkill above twice the health.
    for max_hp in 1..200 {
        for over in 0..=2 * max_hp + 2 {
            let hp = -over;
            let pulverized = matches!(death_remains(hp, max_hp), DeathRemains::Pulverized);
            assert_eq!(
                pulverized,
                over > 2 * max_hp,
                "**at health {max_hp} and {over} over, the two tests agree** - so the second half of the `&&` never decides anything"
            );
        }
    }
}

/// A corpse carries the overkill that made it, and a pulverised body leaves no corpse at all.
///
/// Reference: `monster::die` -> `mdeath::normal` (`monster.cpp:3098`, `mondeath.cpp:58-68`) writes the corpse's damage as `floor( corpse_damage * damage_scale )`, and a `pulverized` body
/// goes to `mdeath::splatter` instead, which returns no corpse item. **The overkill is information the world keeps**: a body killed for twice its health over is a damaged corpse a
/// repairer can read, not a pristine one.
#[test]
fn a_corpse_carries_the_overkill_that_made_it() {
    let mut world = World::new();
    let shambler = cdda_sim::creature::spawn_creature(&mut world, "a shambler", (1, 0, 0));
    cdda_sim::creature::set_kind(&mut world, shambler, cdda_sim::MonsterKindId(42));
    let mut health = cdda_sim::creature::Health { hp: 0, max_hp: 40 };
    world.entity_mut(shambler).insert(health);
    cdda_sim::creature::set_health(&mut world, shambler, 40);
    assert_eq!(world.get::<cdda_sim::creature::Health>(shambler).map(|h| h.max_hp), Some(40), "the ceiling is forty");
    health.hp = -40;
    world.entity_mut(shambler).insert(health);

    let corpse_kind = cdda_sim::ItemTypeId(7);
    let cleared = cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, corpse_kind, |_| 62_500);
    assert_eq!(cleared, 1, "the body was cleared");

    let corpse = cdda_sim::items_at(&mut world, (1, 0))
        .into_iter()
        .next()
        .expect("**a twice-over corpse is still a corpse**");
    assert_eq!(
        world.get::<cdda_sim::GroundItem>(corpse).map(|item| item.damage),
        Some(2500),
        "and it carries 2,500 damage from the overkill"
    );

    // The same body killed three times its health over is a splatter, and leaves nothing whole.
    let other = cdda_sim::creature::spawn_creature(&mut world, "another shambler", (2, 0, 0));
    cdda_sim::creature::set_kind(&mut world, other, cdda_sim::MonsterKindId(42));
    world.entity_mut(other).insert(cdda_sim::creature::Health { hp: -120, max_hp: 40 });
    cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, corpse_kind, |_| 62_500);
    assert_eq!(
        cdda_sim::items_at(&mut world, (2, 0)).len(),
        0,
        "**three times over is a splatter, not a corpse**"
    );
}

/// A dead body takes no further blows, so the overkill a corpse carries is the overkill that killed it.
///
/// Reference: `Creature::deal_damage` calls `monster::die` the moment health reaches zero, so later blows in the same turn are struck at a monster that no longer exists. The port
/// reaps at the turn's end instead, which makes the guard in `hurt` the thing that keeps the reference's arithmetic: **without it a body soaks every remaining attack and its corpse
/// is pulverised by damage nobody dealt to a living thing.**
#[test]
fn a_dead_body_takes_no_further_blows() {
    let mut world = World::new();
    let shambler = cdda_sim::creature::spawn_creature(&mut world, "a shambler", (1, 0, 0));
    world.entity_mut(shambler).insert(cdda_sim::creature::Health { hp: -10, max_hp: 40 });

    cdda_sim::creature::hurt(&mut world, shambler, 30.0);
    assert_eq!(
        cdda_sim::creature::health_of(&world, shambler),
        Some(-10),
        "**the blow is not struck**: health stays where the killing blow left it"
    );

    let alive = cdda_sim::creature::spawn_creature(&mut world, "a living one", (2, 0, 0));
    world.entity_mut(alive).insert(cdda_sim::creature::Health { hp: 40, max_hp: 40 });
    cdda_sim::creature::hurt(&mut world, alive, 15.0);
    assert_eq!(cdda_sim::creature::health_of(&world, alive), Some(25), "while a living body still takes its damage");
}

/// A splatter's gibs follow the body's own damage and health, and a body of one health can be owed none.
///
/// Reference: `mdeath::splatter` (`mondeath.cpp:135-141`):
///
/// ```text
/// int number_of_gibs = std::min( std::floor( corpse_damage ) - 1, 1 + max_hp / 5.0f );
/// if( z.type->size >= creature_size::medium ) number_of_gibs += rng( 1, 6 );
/// ```
///
/// **The count can be negative**, and the reference's `for` loop then simply does nothing - so the port keeps the arithmetic rather than clamping, because clamping at the rule would hide an
/// ordinary small body's answer of zero behind the same value as a body that was never hit hard enough to splatter at all.
#[test]
fn a_splatter_gets_gibs_from_its_damage_and_its_health() {
    use cdda_sim::creature::gib_count;

    assert_eq!(gib_count(2.5, 40), 1, "floor(2.5) - 1 is one, and 1 + 40/5 is nine, so the damage decides");
    assert_eq!(gib_count(10.0, 100), 9, "and a big one is capped by the damage: nine against twenty-one");
    assert_eq!(gib_count(1.0, 40), 0, "a body killed barely is owed no gibs");
    assert_eq!(
        gib_count(1.9, 4),
        0,
        "**the health side can decide too**: floor(1.9) - 1 is zero and 1 + 4/5 is one, so the smaller wins"
    );
    assert_eq!(gib_count(0.4, 40), -1, "and under a whole point of corpse damage the answer is negative, as the reference's is");
}

/// What a monster bleeds, and what it splatters into, follow its flags before its flesh.
///
/// Reference: `mtype::bloodType` (`mtype.cpp:431-454`) and `mtype::gibType` (`mtype.cpp:456-470`):
///
/// ```text
/// blood: ACID_BLOOD -> fd_acid; BILE_BLOOD -> fd_bile; ARTHROPOD_BLOOD -> fd_blood_invertebrate;
///        veggy -> fd_blood_veggy; iflesh -> fd_blood_insect; WARM && flesh -> fd_blood; else get_bleed_type()
/// gibs:  MOLLUSK species -> fd_gibs_invertebrate; veggy -> fd_gibs_veggy; iflesh -> fd_gibs_insect;
///        flesh -> fd_gibs_flesh; else fd_null
/// ```
///
/// **The order is the rule**: an acid-blooded zombie made of flesh bleeds acid, not blood, and the port's ladder has to be walked in the reference's order rather than by "most specific material".
#[test]
fn a_monster_bleeds_by_its_flags_before_its_flesh() {
    use cdda_sim::blood::{BloodTraits, blood_field, gib_field};

    let acid_blooded = BloodTraits { acid: true, bile: true, arthropod: true, warm: true };
    assert_eq!(blood_field(&acid_blooded, &["flesh"]), Some("fd_acid"), "**acid wins over everything below it**");

    let bile_blooded = BloodTraits { acid: false, bile: true, arthropod: true, warm: true };
    assert_eq!(blood_field(&bile_blooded, &["flesh"]), Some("fd_bile"), "then bile");

    let arthropod = BloodTraits { acid: false, bile: false, arthropod: true, warm: true };
    assert_eq!(blood_field(&arthropod, &["flesh"]), Some("fd_blood_invertebrate"), "then an arthropod's");

    let plain = BloodTraits { acid: false, bile: false, arthropod: false, warm: true };
    assert_eq!(blood_field(&plain, &["flesh"]), Some("fd_blood"), "a warm fleshy thing bleeds blood");
    assert_eq!(blood_field(&plain, &["veggy"]), Some("fd_blood_veggy"), "a plant bleeds sap");
    assert_eq!(blood_field(&plain, &["iflesh"]), Some("fd_blood_insect"), "and an insect its own");
    let cold = BloodTraits { acid: false, bile: false, arthropod: false, warm: false };
    assert_eq!(blood_field(&cold, &["flesh"]), None, "**a cold flesh thing falls through**, and its bleed type is not loaded yet");

    assert_eq!(gib_field(&["flesh"], true), Some("fd_gibs_invertebrate"), "a mollusc gibs as an invertebrate");
    assert_eq!(gib_field(&["veggy"], false), Some("fd_gibs_veggy"), "plants into veggy gibs");
    assert_eq!(gib_field(&["iflesh"], false), Some("fd_gibs_insect"), "insects into insect gibs");
    assert_eq!(gib_field(&["flesh"], false), Some("fd_gibs_flesh"), "and flesh into flesh gibs");
    assert_eq!(gib_field(&["steel"], false), None, "while steel gibs into nothing at all");
}

/// A pulverised body writes its gibs and its blood into the world, and an ordinary corpse bleeds where it fell.
///
/// Reference: `mdeath::splatter` (`mondeath.cpp:143-150`) adds `fd_gib` per gib and `fd_blood` with it, and `mdeath::normal`'s `z.bleed( *here )` (`mondeath.cpp:63`) leaves blood without
/// gibs. **The field is the record**: the corpse item says what died, and the field says what it left on the ground, which is the only thing a later player sees.
#[test]
fn a_splattered_body_writes_its_gibs_into_the_world() {
    let mut world = World::new();
    let shambler = cdda_sim::spawn_monster(&mut world, (4, 4), cdda_sim::MonsterKindId(42), 40);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_fixture", hp: 40 }],
    );
    cdda_sim::monster_registry::set_remains(&mut world, cdda_sim::MonsterKindId(1), Some("fd_blood"), Some("fd_gibs_flesh"));
    // The two fields the corpse will write, with the intensity range the data gives them.
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_blood", 3, 0), ("fd_gibs_flesh", 3, 0)]);
    cdda_sim::creature::set_kind(&mut world, shambler, cdda_sim::MonsterKindId(1));
    world
        .entity_mut(shambler)
        .insert(cdda_sim::creature::Health { hp: -120, max_hp: 40 });

    let corpse_kind = cdda_sim::ItemTypeId(7);
    cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, corpse_kind, |_| 62_500);

    let gibs = cdda_sim::field_registry::id_of(&mut world, "fd_gibs_flesh").expect("the field id is known");
    let blood = cdda_sim::field_registry::id_of(&mut world, "fd_blood").expect("the field id is known");
    assert!(
        cdda_sim::field_strength_at(&world, (4, 4, 0), gibs) > 0,
        "**the splatter left gibs on the square**"
    );
    assert!(cdda_sim::field_strength_at(&world, (4, 4, 0), blood) > 0, "and blood with them");
}

/// A hallucination that dies simply disappears: no message, no corpse, and no blood.
///
/// Reference: `monster::die` (`monster.cpp:3046-3050`):
///
/// ```text
/// if( is_hallucination() ) {
///     mdeath::disappear( *this );
///     return;
/// }
/// ```
///
/// **The early return is the whole rule**: everything after it - the death message, the death effects, the corpse, the drops - belongs to a real body, and a hallucination left any of them
/// behind would be a figment a player could butcher. The port's reaping has to skip the corpse **and** the blood for the same reason.
#[test]
fn a_hallucination_leaves_nothing_behind() {
    let mut world = World::new();
    let phantom = cdda_sim::spawn_monster(&mut world, (5, 5), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_fixture", hp: 40 }],
    );
    cdda_sim::monster_registry::set_remains(&mut world, cdda_sim::MonsterKindId(1), Some("fd_blood"), Some("fd_gibs_flesh"));
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_blood", 3, 0), ("fd_gibs_flesh", 3, 0)]);
    cdda_sim::creature::set_kind(&mut world, phantom, cdda_sim::MonsterKindId(1));
    world.entity_mut(phantom).insert(cdda_sim::creature::Hallucination);
    world.entity_mut(phantom).insert(cdda_sim::creature::Health { hp: -40, max_hp: 40 });

    let cleared = cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, cdda_sim::ItemTypeId(7), |_| 62_500);

    assert_eq!(cleared, 1, "it is gone");
    assert!(world.get_entity(phantom).is_err(), "and no creature remains");
    assert_eq!(cdda_sim::items_at(&mut world, (5, 5)).len(), 0, "**no corpse to butcher**");
    let blood = cdda_sim::field_registry::id_of(&mut world, "fd_blood").expect("known");
    assert_eq!(cdda_sim::field_strength_at(&world, (5, 5, 0), blood), 0, "and no blood either");
}

/// A body the player can see says that it died, and a quiet one does not.
///
/// Reference: `mdeath::normal` (`mondeath.cpp:50-56`):
///
/// ```text
/// if( !z.quiet_death && !z.has_flag( mon_flag_QUIETDEATH ) ) {
///     ...
///     add_msg_if_player_sees( z, m_good, _( "The %s dies!" ), z.name() );
/// }
/// ```
///
/// **The line is good news** (`m_good`) and it is what tells a player the fight is over; a `QUIETDEATH` monster dies silently, which is the flag's whole purpose. The port writes it as a
/// message of the Good kind, because a death the log files as neutral would read like a warning.
#[test]
fn a_death_says_so_unless_the_body_is_quiet() {
    let line = cdda_sim::creature::death_message("shambler", false);
    assert_eq!(line.as_deref(), Some("The shambler dies!"), "the ordinary body says so");
    assert_eq!(
        cdda_sim::creature::death_message("shambler", true),
        None,
        "**and a quiet one says nothing at all**"
    );
}

/// The reaping step writes the death line into the game log, as good news, and a quiet body writes none.
///
/// Reference: `mdeath::normal` (`mondeath.cpp:50-56`) with `m_good`. The gate reads the **log**, not the helper: a rule that produced the right sentence and never pushed it would pass a
/// test of the sentence and fail the player.
#[test]
fn the_game_log_says_that_a_body_died() {
    let mut world = World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_fixture", hp: 40 }],
    );
    let loud = cdda_sim::spawn_monster(&mut world, (6, 6), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::creature::set_health_left(&mut world, loud, 0);
    let quiet = cdda_sim::spawn_monster(&mut world, (7, 6), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::creature::set_health_left(&mut world, quiet, 0);
    world
        .entity_mut(quiet)
        .insert(cdda_sim::creature_flags::CreatureFlags(std::collections::BTreeSet::from([
            "QUIETDEATH".to_owned(),
        ])));

    cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, cdda_sim::ItemTypeId(7), |_| 62_500);

    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines.iter().any(|line| line.contains("mon_fixture") && line.contains("dies!")),
        "**the loud body's death is in the log**: {lines:?}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.contains("dies!")).count(),
        1,
        "and the quiet one wrote nothing, so there is exactly one line"
    );
}

/// The type's own death sentence is printed before the normal branch's, and only when the data states one.
///
/// Reference: `monster::die` (`monster.cpp:3086`) prints `type->mdeath_effect.death_message.translated()` with the monster's name, and `mdeath::normal` then prints "The %s dies!"
/// (`mondeath.cpp:50-56`) - the reference's own comment says a body can produce several lines. **321 of the 1,154 types state a sentence**, so the port has to print it, and the `%s` is
/// where the name goes.
#[test]
fn a_type_can_have_its_own_death_sentence() {
    use cdda_sim::creature::typed_death_message;

    assert_eq!(
        typed_death_message("The %s becomes part of the endless chamber.", "shoggoth"),
        Some("The shoggoth becomes part of the endless chamber.".to_owned()),
        "**the name goes where the data put the placeholder**"
    );
    assert_eq!(typed_death_message("", "shambler"), None, "a type with no sentence adds nothing");
    assert_eq!(
        typed_death_message("It is gone.", "shambler"),
        Some("It is gone.".to_owned()),
        "and one with no placeholder is printed as it stands"
    );
}

/// The type's own `corpse_type` decides what is left, and it can overrule the overkill rule both ways.
///
/// Reference: `monster::die`'s switch (`monster.cpp:3093-3106`): `NORMAL` goes to `mdeath::normal` (which may still splatter), `SPLATTER` goes straight to `mdeath::splatter`, and
/// `NO_CORPSE` does nothing at all - no corpse, and no blood either, because the bleeding lives inside the normal branch.
///
/// **The data has the last word**: a `NO_CORPSE` body killed gently still leaves nothing, and a `SPLATTER` body killed exactly still splatters.
#[test]
fn the_type_decides_what_the_body_leaves() {
    let mut world = World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[
            cdda_sim::monster_registry::MonsterKind { id: "mon_none", hp: 40 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_splatter", hp: 40 },
        ],
    );
    cdda_sim::monster_registry::set_remains(&mut world, cdda_sim::MonsterKindId(1), Some("fd_blood"), Some("fd_gibs_flesh"));
    cdda_sim::monster_registry::set_remains(&mut world, cdda_sim::MonsterKindId(2), Some("fd_blood"), Some("fd_gibs_flesh"));
    cdda_sim::monster_registry::set_corpse_type(&mut world, cdda_sim::MonsterKindId(1), "none");
    cdda_sim::monster_registry::set_corpse_type(&mut world, cdda_sim::MonsterKindId(2), "splatter");
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_blood", 3, 0), ("fd_gibs_flesh", 3, 0)]);

    // A `NO_CORPSE` body killed to exactly zero: nothing, where an ordinary one would leave a corpse.
    let nothing = cdda_sim::spawn_monster(&mut world, (8, 8), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::creature::set_health_left(&mut world, nothing, 0);
    // A `SPLATTER` body killed to exactly zero: a splatter, where an ordinary one would leave an undamaged corpse.
    let splat = cdda_sim::spawn_monster(&mut world, (9, 8), cdda_sim::MonsterKindId(2), 40);
    cdda_sim::creature::set_health_left(&mut world, splat, 0);

    cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, cdda_sim::ItemTypeId(7), |_| 62_500);

    assert_eq!(cdda_sim::items_at(&mut world, (8, 8)).len(), 0, "**none leaves nothing**");
    let blood = cdda_sim::field_registry::id_of(&mut world, "fd_blood").expect("known");
    assert_eq!(
        cdda_sim::field_strength_at(&world, (8, 8, 0), blood),
        0,
        "and no blood either, because the bleeding lives in the normal branch"
    );
    assert_eq!(cdda_sim::items_at(&mut world, (9, 8)).len(), 0, "splatter leaves no corpse on a gentle kill");
    let gibs = cdda_sim::field_registry::id_of(&mut world, "fd_gibs_flesh").expect("known");
    assert!(
        cdda_sim::field_strength_at(&world, (9, 8, 0), gibs) > 0,
        "**but it does splatter**"
    );
}

/// A `broken` body leaves the broken thing, not a corpse, and the id is made from the monster's own.
///
/// Reference: `mdeath::broken` (`mondeath.cpp:204-228`): the item is the type's `broken_itype` when the data states one, and otherwise the monster's id with `mon_` stripped and
/// `broken_` put in front - so `mon_manhack` leaves a `broken_manhack`. **The damage is the same overkill rule as a corpse** (`set_damage( floor( corpse_damage * damage_scale ) )`).
#[test]
fn a_broken_body_leaves_the_broken_thing() {
    use cdda_sim::creature::broken_item_id;

    assert_eq!(broken_item_id("mon_manhack", ""), "broken_manhack", "**the id is made from the monster's own**");
    assert_eq!(broken_item_id("mon_zombie", ""), "broken_zombie");
    assert_eq!(
        broken_item_id("mon_manhack", "broken_drone"),
        "broken_drone",
        "and the data's own id wins when it states one"
    );
    assert_eq!(broken_item_id("manhack", ""), "broken_manhack", "an id without the prefix is left alone");
}

/// A `broken` body really leaves the broken item, found by the id the rule makes.
///
/// Reference: `mdeath::broken` (`mondeath.cpp:224-226`): the item is made and given `set_damage`, then `add_item_or_charges` puts it on the square. **It is not a corpse**: it carries no
/// `CorpseOf`, because there is nothing to butcher - the machine is scrap.
#[test]
fn a_broken_body_drops_the_broken_item() {
    let mut world = World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_manhack", hp: 40 }],
    );
    cdda_sim::monster_registry::set_corpse_type(&mut world, cdda_sim::MonsterKindId(1), "broken");
    // The item the broken id rule makes has to exist in the registry, exactly as the loader would leave it.
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "broken_manhack",
            volume_ml: 250,
            weight_g: 1_000,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let scrap = cdda_sim::spawn_monster(&mut world, (10, 10), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::creature::set_health_left(&mut world, scrap, 0);

    cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, cdda_sim::ItemTypeId(99), |_| 62_500);

    let left = cdda_sim::items_at(&mut world, (10, 10));
    assert_eq!(left.len(), 1, "**one broken thing on the square**");
    let expected = cdda_sim::item_registry::id_of(&mut world, "broken_manhack").expect("the item is registered");
    assert_eq!(
        world.get::<cdda_sim::GroundItem>(left[0]).map(|item| item.kind),
        Some(expected),
        "and it is the broken item the id rule named"
    );
    assert!(
        world.get::<cdda_sim::CorpseOf>(left[0]).is_none(),
        "**it is scrap, not a corpse** - there is nothing to butcher"
    );
}

/// A body drops what its `death_drops` group says, on the square it fell on.
///
/// Reference: `monster::die` (`monster.cpp:3128-3131`) -> `drop_items_on_death` (`monster.cpp:3248-3272`), which draws the group with `item_group::items_from` and puts every
/// produced item on the square. **A hallucination drops nothing** (`:3250-3252`), and a type with no group drops nothing (`:3253-3255`) - two early returns that look the same and are
/// not: one is about the body being real, the other about the data saying nothing.
#[test]
fn a_body_drops_what_its_group_says() {
    let mut world = World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_looter", hp: 40 }],
    );
    cdda_sim::monster_registry::set_death_drops(&mut world, cdda_sim::MonsterKindId(1), "loot_group");
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rag",
            volume_ml: 250,
            weight_g: 100,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    // The group's data, as a reader would find it: a collection with one entry, always one rag.
    let mut definitions = std::collections::BTreeMap::new();
    definitions.insert(
        "loot_group".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "loot_group".to_owned(),
            distribution: false,
            entries: vec![cdda_content::item_group_draw::Entry::Item {
                id: "rag".to_owned(),
                probability: 100,
                count: (1, 1),
                event: None,
            }],
        },
    );
    cdda_sim::item_groups::set_groups(&mut world, definitions);
    let mut dice = cdda_core_types::rng::Rolls::new(1);
    cdda_sim::singletons::put_rolls(&mut world, dice);

    let looter = cdda_sim::spawn_monster(&mut world, (11, 11), cdda_sim::MonsterKindId(1), 40);
    cdda_sim::creature::set_health_left(&mut world, looter, 0);
    cdda_sim::creature::reap_dead_creatures_as_corpses(&mut world, cdda_sim::ItemTypeId(99), |_| 62_500);

    let left = cdda_sim::items_at(&mut world, (11, 11));
    let rag = cdda_sim::item_registry::id_of(&mut world, "rag").expect("the item is registered");
    assert!(
        left.iter().any(|item| world.get::<cdda_sim::GroundItem>(*item).is_some_and(|held| held.kind == rag)),
        "**the group's rag is on the square**: {left:?}"
    );
}

/// A monster says how well it is mending, in three grades, and a dark one mends by a curve that a light snuffs out.
///
/// Reference: `monster::process_effects` (`monster.cpp:3460-3497`):
///
/// ```text
/// int regeneration_amount = type->regenerates;              // plus effect and enchantment modifiers, floored at zero
/// const int healed_amount = heal( regeneration_amount );
/// if( healed_amount > 0 && one_in( 2 ) )
///     healed >= 50 -> "The %s is visibly regenerating!"
///     healed >= 10 -> "The %s seems a little healthier."
///     else         -> "The %s is healing slowly."
/// if( type->regenerates_in_dark && !is_in_sunlight() )
///     dHP = 50.0 * exp( - light * light / 10000 )
/// ```
///
/// **The thresholds are on what was actually healed, not on what was asked for**: a monster at full health heals nothing and says nothing, which is the branch a port that keyed off
/// `regenerates` would get wrong. **The dark curve is the reference's own magic number**: `10000` was chosen so a floodlight stops regeneration twenty tiles away.
#[test]
fn a_mending_monster_says_how_well_it_is_mending() {
    use cdda_sim::creature::{dark_regeneration, regeneration_message};

    assert_eq!(regeneration_message(0), None, "**nothing healed, nothing said**");
    assert_eq!(regeneration_message(1), Some("The %s is healing slowly."), "under ten is slow");
    assert_eq!(regeneration_message(10), Some("The %s seems a little healthier."), "ten is a little healthier");
    assert_eq!(regeneration_message(49), Some("The %s seems a little healthier."), "and forty-nine is still that");
    assert_eq!(regeneration_message(50), Some("The %s is visibly regenerating!"), "fifty is visible");

    // 50 * e^0 at full dark, and the curve halves every time light^2/10000 grows by ln 2 - so light 100 is 50 * e^-1.
    assert_eq!(dark_regeneration(0.0), 50, "in pitch dark, fifty");
    assert_eq!(dark_regeneration(100.0), 18, "at a hundred, 50/e truncated to eighteen");
    // At two hundred the curve is 50 * e^-4, which is 0.91 - **truncated to zero**, so the light has stopped it.
    assert_eq!(dark_regeneration(200.0), 0, "and at two hundred the light has stopped it");
}

/// The mending system says how well a body is mending, on the reference's one-in-two.
///
/// Reference: `monster::process_effects` (`monster.cpp:3473-3486`), which runs `heal( regeneration_amount )` and then, **on `one_in( 2 )`**, says one of the three lines. The gate
/// runs the real turn system over a range of seeds and asserts the line that appears is the grade of the amount actually mended - a port that keyed the text off the type's rate would
/// print "visibly regenerating" for a body mending one point.
#[test]
fn the_mending_system_says_so_on_a_coin_flip() {
    let mut said = 0;
    for seed in 0..40 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let monster = cdda_sim::spawn_monster(&mut world, (12, 12), cdda_sim::MonsterKindId(1), 40);
        cdda_sim::creature::set_health_left(&mut world, monster, 20);
        // One hit point a turn: the smallest grade there is.
        world.entity_mut(monster).insert(cdda_sim::regeneration::Mending {
            per_hour: 3_600.0,
            type_speed: 100,
            type_hp: 40,
        in_dark: false,
        });
        cdda_sim::regeneration::mend_everyone(&mut world);
        let lines = cdda_sim::messages::lines(&world);
        if let Some(line) = lines.iter().find(|line| line.contains("healing") || line.contains("healthier") || line.contains("regenerating")) {
            said += 1;
            assert!(
                line.contains("is healing slowly."),
                "**one point is the slowest grade**, whatever the rate was: {line}"
            );
        }
    }
    assert!(said > 5, "the one-in-two coin flip says it often enough to be seen: {said} of 40 seeds");
}

/// A square is in sunlight only when it has no roof over it **and** the sun is high enough to count.
///
/// Reference: `game::is_in_sunlight` (`game.cpp:5683-5687`):
///
/// ```text
/// return !is_sheltered( p ) && incident_sun_irradiance( weather, calendar::turn ) > irradiance::minimal;
/// ```
///
/// `irradiance::minimal` is **87 W/m²**, the sun at five degrees on a clear day (`weather.h:56-60`). **Both halves are needed and neither is the obvious one**: a roof shuts out a noon sun,
/// and a sun below five degrees is not sunlight however open the sky.
#[test]
fn sunlight_needs_an_open_sky_and_a_sun_above_five_degrees() {
    use cdda_sim::creature::is_in_sunlight;

    assert!(is_in_sunlight(false, 900.0), "a noon sun over an open square is sunlight");
    assert!(!is_in_sunlight(true, 900.0), "**a roof shuts it out**");
    assert!(!is_in_sunlight(false, 87.0), "and the floor is exclusive: 87 is not above 87");
    assert!(is_in_sunlight(false, 87.1), "while a hair above it is");
    assert!(!is_in_sunlight(false, 0.0), "darkness is not sunlight");
}

/// A monster that mends by the dark gains the curve on top of its rate, and loses it under a lamp.
///
/// Reference: `monster::process_effects` (`monster.cpp:3487-3494`): a `regenerates_in_dark` type heals `50 * e^(-light^2/10000)` when the square is not in sunlight, **in addition** to its
/// stated rate. The gate drives the real system over the real lighting answer: a body with a rate of zero mends fifty a turn on a dark square and **nothing at all** under a lamp bright
/// enough to put the answer past two hundred, which is the only way to tell the flag from a rate that happened to be large.
#[test]
fn a_monster_mends_by_the_dark() {
    let mut healed = Vec::new();
    for lantern in [false, true] {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
        let monster = cdda_sim::spawn_monster(&mut world, (13, 13), cdda_sim::MonsterKindId(1), 200);
        cdda_sim::creature::set_health_left(&mut world, monster, 50);
        world.entity_mut(monster).insert(cdda_sim::regeneration::Mending {
            per_hour: 0.0,
            type_speed: 100,
            type_hp: 200,
            in_dark: true,
        });
        if lantern {
            // A lamp on the body's own square: the lighting answer is built from the store like any other turn's.
            world.spawn((
                cdda_sim::Position { x: 13, y: 13, z: 0 },
                cdda_sim::LightSource { luminance: 400.0 },
            ));
            cdda_sim::cast_lights_with(&mut world);
            assert!(
                cdda_sim::singletons::light(&world).max_at(13, 13) > 200.0,
                "the lamp really is bright enough to stop the curve"
            );
        }
        healed.push(cdda_sim::regeneration::mend_everyone(&mut world));
    }
    assert_eq!(healed[0], 50, "**in the dark, the full fifty**");
    assert_eq!(healed[1], 0, "and under the lamp, nothing at all");
}

/// The weather scales the sun's irradiance, and the answer can never go below nothing.
///
/// Reference: `incident_sun_irradiance` (`weather.cpp:147-150`):
///
/// ```text
/// return std::max<float>( 0.0f, sun_irradiance( t ) * wtype->sun_multiplier );
/// ```
///
/// **One number, and the clamp is the whole guard**: a weather with a multiplier above one makes the sun stronger than the clear sky's own, which is how snow glare burns at a lower sun than a
/// clear day's glare, and a negative multiplier - which the data does not use but the reference still guards - stops at zero rather than turning the sun into darkness.
#[test]
fn the_weather_scales_the_sun() {
    use cdda_sim::creature::incident_sun_irradiance;

    assert_eq!(incident_sun_irradiance(800.0, 1.0), 800.0, "clear weather passes the sun through");
    assert_eq!(incident_sun_irradiance(800.0, 1.25), 1_000.0, "and a glare-making sky makes it stronger");
    assert_eq!(incident_sun_irradiance(800.0, 0.0), 0.0, "an overcast sky puts it out");
    assert_eq!(incident_sun_irradiance(800.0, -3.0), 0.0, "**and the clamp stops a negative sky at nothing**");
}

/// The sun glares into a creature's eyes, and a winter sun does it at a much lower height.
///
/// Reference: `glare` (`weather.cpp:100-138`):
///
/// ```text
/// if( sheltered || asleep || SUN_GLASSES || GLARE_RESIST || blind ) return;
/// if( season == WINTER && incident_sun_irradiance > irradiance::moderate )  effect = snow_glare;   // 573 W/m2
/// else if( incident_sun_irradiance > irradiance::high )                     effect = glare;        // 707 W/m2
/// dur = has_effect( effect ) ? 10_turns : 20_turns;
/// if( has_trait( CEPH_VISION ) ) dur *= 2;
/// ```
///
/// **The winter branch is the rule worth pinning**: snow glare starts at **573**, not at the 707 a clear summer day needs, so the *same* sun glares in winter and does not in summer. And it
/// is an `else if` - a winter sun strong enough for either takes the **snow** effect, never both.
#[test]
fn the_sun_glares_into_eyes() {
    use cdda_sim::glare::{Conditions, glare_of};

    let clear = Conditions::default();
    assert_eq!(glare_of(true, 800.0, clear), Some(("snow_glare", 20)), "**a winter sun glares at 800 in snow**");
    assert_eq!(glare_of(false, 800.0, clear), Some(("glare", 20)), "and a summer sun glares at 800 without it");
    assert_eq!(
        glare_of(true, 600.0, clear),
        Some(("snow_glare", 20)),
        "**600 is snow glare in winter** - above moderate"
    );
    assert_eq!(glare_of(false, 600.0, clear), None, "and nothing at all in summer: it is under high");

    let already = Conditions {
        already_glared: true,
        ..Conditions::default()
    };
    assert_eq!(glare_of(false, 800.0, already), Some(("glare", 10)), "a second glare is shorter");
    let ceph = Conditions {
        ceph_vision: true,
        ..Conditions::default()
    };
    assert_eq!(glare_of(false, 800.0, ceph), Some(("glare", 40)), "and the cephalopod trait doubles it");

    for blocked in [
        Conditions { sheltered: true, ..Conditions::default() },
        Conditions { asleep: true, ..Conditions::default() },
        Conditions { sun_glasses: true, ..Conditions::default() },
        Conditions { glare_resist: true, ..Conditions::default() },
        Conditions { blind: true, ..Conditions::default() },
    ] {
        assert_eq!(glare_of(false, 900.0, blocked), None, "**any one prerequisite stops it**: {blocked:?}");
    }
}

/// The weather's light is the sun **scaled and then shifted**, clamped at nothing - a different shape from its irradiance.
///
/// Reference: `incident_sunlight` (`weather.cpp:142-145`):
///
/// ```text
/// return std::max<float>( 0.0f, sun_light_at( t ) * wtype->light_multiplier + wtype->light_modifier );
/// ```
///
/// **The order is the rule**: the multiplier applies to the sun alone and the modifier is added afterwards, so a clear day of 100 under a fog with a modifier of -30 and a multiplier of 0.5 is
/// 20, not 35. A port that folded the modifier into the scale would agree on a multiplier of one and disagree everywhere else - which is exactly the case the data uses.
#[test]
fn the_weather_shifts_the_sunlight_it_lets_through() {
    use cdda_sim::creature::incident_sunlight;

    assert_eq!(incident_sunlight(100.0, 1.0, 0), 100.0, "clear weather passes the light through");
    assert_eq!(incident_sunlight(100.0, 0.5, -30), 20.0, "**half of a hundred, then thirty less**");
    assert_eq!(incident_sunlight(100.0, 1.0, -120), 0.0, "and the clamp stops a dark sky at nothing");
    assert_eq!(incident_sunlight(100.0, 1.0, 15), 115.0, "a modifier above zero brightens, which the data does for glare");
}

/// A stretch of weather is summed in coarse steps, and the steps get coarser the longer the stretch.
///
/// Reference: `sum_conditions` (`weather.cpp:190-215`) and `proc_weather_sum` (`weather.cpp:152-180`):
///
/// ```text
/// if( diff < 10_turns )        tick = 1_turn;
/// else if( diff > 7_days )     tick = 1_hours;
/// else                         tick = 1_minutes;
/// ...
/// amount = very_light ? 1 : light ? 4 : heavy ? 8;   // only when the weather rains
/// data.rain_amount += amount * to_turns( tick );
/// data.sunlight += incident_sunlight( w ) * to_turns( tick );
/// data.radiant_exposure += incident_sun_irradiance( w ) * to_seconds( tick );
/// ```
///
/// **The comparisons are asymmetric and that is the trap**: *under* ten turns is the fine step and *over* seven days is the coarse one, so exactly ten turns and exactly seven days both take the
/// **middle** step. A port that wrote `<=` in either place would agree on every value but the two boundaries and never notice.
#[test]
fn a_weather_sum_steps_in_coarser_ticks() {
    use cdda_sim::weather_sum::{MovesPerTick, tick_size};

    assert_eq!(tick_size(9), MovesPerTick::Turn, "nine turns is summed a turn at a time");
    assert_eq!(tick_size(10), MovesPerTick::Minute, "**and exactly ten turns is already a minute**");
    assert_eq!(tick_size(7 * 24 * 60 * 60), MovesPerTick::Minute, "seven days exactly is still a minute");
    assert_eq!(tick_size(7 * 24 * 60 * 60 + 1), MovesPerTick::Hour, "**one turn more and it is an hour**");
}

/// One step of weather adds rain per turn, light per turn and exposure per second.
///
/// Reference: `proc_weather_sum` (`weather.cpp:174-180`). **The two sun numbers are scaled by different clocks**: light by `to_turns( tick )` and radiant exposure by `to_seconds( tick )`.
/// They happen to be the same number, because a turn is a second - which is exactly the coincidence a port must not rely on, so the gate pins a minute as sixty turns for one and sixty
/// seconds for the other, and checks the rain against the precipitation ladder.
#[test]
fn one_step_of_weather_adds_rain_light_and_exposure() {
    use cdda_sim::weather_sum::{MovesPerTick, rain_per_turn, step_of};

    assert_eq!(rain_per_turn(cdda_content::weather::Precipitation::VeryLight, true), 1, "a turn of drizzle is one");
    assert_eq!(rain_per_turn(cdda_content::weather::Precipitation::Light, true), 4, "of rain, four");
    assert_eq!(rain_per_turn(cdda_content::weather::Precipitation::Heavy, true), 8, "of a downpour, eight");
    assert_eq!(rain_per_turn(cdda_content::weather::Precipitation::Heavy, false), 0, "**and a snowing sky adds no rain at all**");

    let mut weather = cdda_content::weather::WeatherType::default();
    weather.precip = cdda_content::weather::Precipitation::Light;
    weather.rains = true;
    let minute = step_of(&weather, MovesPerTick::Minute, 100.0, 800.0);
    assert_eq!(minute.rain, 240, "a minute of light rain is sixty turns of four");
    assert_eq!(minute.sunlight, 6_000.0, "and a hundred of light for sixty turns");
    assert_eq!(minute.radiant_exposure, 48_000.0, "with eight hundred of exposure for sixty seconds");
}

/// Wind has a name and a direction, and both ladders are read from the top down.
///
/// Reference: `get_wind_desc` (`weather.cpp:817-850`) and `convert_wind_to_coord` (`weather.cpp:852-874`):
///
/// ```text
/// if( windpower < 1 )        "Calm"
/// else if( <= 3 )            "Light Air"     ... twelve more grades ... else "Hurricane"
///
/// for( const auto &val : outputs ) if( angle >= val.first ) return val.second;   // 330, 301, 240, 211, 150, 121, 60, 31, 0
/// return ( 0, 0 );
/// ```
///
/// **The first grade is `< 1` and every other is `<=`**, so exactly one is "Calm" and a wind of one is already "Light Air". **The direction table is read from the top down and its
/// thresholds descend**, so the first row at or below the angle wins - 330 is north, 31 is north-east, and the last row catches everything down to zero.
#[test]
fn wind_has_a_name_and_a_direction() {
    use cdda_sim::wind::{description, direction};

    assert_eq!(description(0.0), "Calm", "**under one is calm**");
    assert_eq!(description(0.99), "Calm");
    assert_eq!(description(1.0), "Light Air", "**and one is already light air**");
    assert_eq!(description(3.0), "Light Air");
    assert_eq!(description(3.1), "Light Breeze");
    assert_eq!(description(72.0), "Violent Storm", "seventy-two is still a violent storm");
    assert_eq!(description(72.1), "Hurricane", "and above it, a hurricane");

    assert_eq!(direction(330), (0, -1), "330 is north");
    assert_eq!(direction(360), (0, -1), "and so is the whole turn past it");
    assert_eq!(direction(329), (-1, -1), "329 has already turned north-west");
    assert_eq!(direction(31), (1, -1), "**thirty-one is north-east**");
    assert_eq!(direction(0), (0, -1), "zero is the last row, which is north again");
}

/// The wind is halved by a wood, raised by height and cut by a wall - in that order.
///
/// Reference: `get_local_windpower` (`weather.cpp:785-810`). **The order is the rule**: a gale of twenty in a wood on a roof of height two beside a wall is `( 20/2 + 2*min(5, 20/2) )/10 = 2`,
/// not the `20/10 + ...` a port that blocked first would get, and a shelter answers **zero** whatever the sky is doing.
#[test]
fn the_ground_takes_the_wind() {
    use cdda_sim::wind::{Ground, local_windpower};

    assert_eq!(local_windpower(20, Ground::default()), 20, "open ground leaves a gale a gale");
    assert_eq!(
        local_windpower(20, Ground { forest: true, ..Ground::default() }),
        10,
        "a wood halves it"
    );
    assert_eq!(
        local_windpower(20, Ground { height: 3, ..Ground::default() }),
        35,
        "height three adds `3 * min( 5, 20 )`"
    );
    assert_eq!(
        local_windpower(20, Ground { blocked: true, ..Ground::default() }),
        2,
        "a wall cuts it to a tenth"
    );
    assert_eq!(
        local_windpower(20, Ground { forest: true, height: 2, blocked: true, ..Ground::default() }),
        2,
        "**and all three in the reference's order**: ( 20/2 + 2*5 ) / 10"
    );
    assert_eq!(
        local_windpower(90, Ground { sheltered: true, ..Ground::default() }),
        0,
        "while indoors there is no wind however hard it blows"
    );
}

/// A swing from the floor or from a crouch costs extra, and a thing on pseudopods pays neither.
///
/// Reference: `get_total_melee_stamina_cost` (`melee.cpp:970-997`):
///
/// ```text
/// const int stance_malus = ( is_on_ground() && !PSEUDOPOD_GRASP ) ? 50
///     : ( !PSEUDOPOD_GRASP && !has_effect( natural_stance ) && !unarmed_attack() && is_crouching() ) ? 20
///     : 0;
/// ... return std::min<int>( -50, proficiency_multiplier * ( mod_sta + melee - stance_malus ) );
/// ```
///
/// **Two maluses, and the reference asks them as a chain**: `50` if the swinger is on the ground, `20` only if it is also **not** on the ground, not in a natural stance and **armed**,
/// and nothing otherwise. **`PSEUDOPOD_GRASP` cancels both**, because a thing on pseudopods is not inconvenienced by being low - the reference says as much in its own comment.
#[test]
fn a_low_swing_costs_more_stamina() {
    use cdda_sim::melee::{Stance, stamina_cost, stance_malus};

    let standing = Stance::default();
    assert_eq!(stance_malus(standing), 0, "a standing, armed swing pays nothing extra");
    assert_eq!(
        stance_malus(Stance { on_ground: true, ..Stance::default() }),
        50,
        "**from the floor it is fifty**"
    );
    assert_eq!(
        stance_malus(Stance { crouching: true, armed: true, ..Stance::default() }),
        20,
        "a crouched swing with a weapon is twenty"
    );
    assert_eq!(
        stance_malus(Stance { crouching: true, armed: false, ..Stance::default() }),
        0,
        "and an unarmed crouch pays nothing: `!unarmed_attack()` is one of the conditions"
    );
    assert_eq!(
        stance_malus(Stance { crouching: true, armed: true, natural_stance: true, ..Stance::default() }),
        0,
        "nor does a natural stance"
    );
    assert_eq!(
        stance_malus(Stance { on_ground: true, pseudopod_grasp: true, ..Stance::default() }),
        0,
        "**and pseudopods cancel the floor's fifty**"
    );

    // **The sign trap**: the answer is *negative* and the malus is *subtracted*, so a bigger malus makes the number **more negative** - which is *more* spent, not less. The
    // reference then takes `std::min( -50, ... )`, i.e. the more negative of the two, so the floor is a floor on the *rebate* rather than on the cost.
    assert_eq!(stamina_cost(0, 0, 0, 1.0), -50, "an empty-handed standing swing is the floor");
    assert_eq!(stamina_cost(0, 0, 20, 1.0), -70, "**a crouched one spends twenty more**");
    assert_eq!(stamina_cost(0, 0, 50, 1.0), -100, "and the floor's malus fifty more again");
}

/// Planting needs a square at fifty Fahrenheit, and fifty exactly counts.
///
/// Reference: `warm_enough_to_plant` (`weather.cpp:875-883`):
///
/// ```text
/// return get_weather().get_temperature( pos ) >= units::from_fahrenheit( 50 );
/// ```
///
/// **The comparison is `>=`**, so a square at exactly fifty is warm enough - which is the first thing a port rewrites as `>` and the last thing a test notices. The threshold is stated in
/// **Fahrenheit** and the port holds kelvin, so the gate pins the conversion too: `( 50 - 32 ) / 1.8 + 273.15 = 283.15 K`.
#[test]
fn planting_needs_fifty_fahrenheit() {
    use cdda_sim::temperature::warm_enough_to_plant;

    assert!(warm_enough_to_plant(283.15), "**exactly fifty Fahrenheit is warm enough**");
    assert!(!warm_enough_to_plant(283.14), "and a hundredth of a degree colder is not");
    assert!(warm_enough_to_plant(300.0), "a summer day certainly is");
    assert!(!warm_enough_to_plant(273.15), "and freezing is not");
}

/// A proficiency's stamina bonus can only ever make a swing cheaper, and a body of them can zero the cost.
///
/// Reference: `get_total_melee_stamina_cost` (`melee.cpp:979-995`):
///
/// ```text
/// float proficiency_multiplier = 1.f;
/// for( const weapon_category_id &cat : wielded_weapon_categories( *this ) ) {
///     float loss = 0.f;
///     for( const proficiency_id &prof : cat->category_proficiencies() ) {
///         if( !has_proficiency( prof ) ) continue;
///         std::optional<float> bonus = prof->bonus_for( "melee_attack", proficiency_bonus_type::stamina );
///         if( !bonus.has_value() ) continue;
///         loss += bonus.value();
///     }
///     proficiency_multiplier = std::clamp( 1.f - loss, 0.f, proficiency_multiplier );
/// }
/// return std::min<int>( -50, proficiency_multiplier * ( mod_sta + melee - stance_malus ) );
/// ```
///
/// **The upper bound of the clamp is the running multiplier, not one**: so each weapon category can only *lower* the multiplier, and a category that loses nothing leaves it where it was
/// rather than resetting it. **The losses inside a category add**, which is how two proficiencies of a quarter each become half. And the clamp's floor at zero means a body of bonuses
/// stops at free rather than turning into a rebate.
#[test]
fn a_proficiency_can_only_make_a_swing_cheaper() {
    use cdda_sim::melee::proficiency_stamina_multiplier;

    assert_eq!(proficiency_stamina_multiplier(&[]), 1.0, "no proficiencies, no discount");
    assert_eq!(proficiency_stamina_multiplier(&[0.025]), 0.975, "a familiar's two and a half percent");
    assert_eq!(proficiency_stamina_multiplier(&[0.05]), 0.95, "**two of them add inside the category**, which is the inner loop's sum");
    assert_eq!(
        proficiency_stamina_multiplier(&[0.025, 0.025]),
        0.975,
        "**but two categories do not stack**: the second wants 0.975, and the clamp's ceiling is already 0.975"
    );
    // Single precision: 1.0 - 0.9 is 0.100000024, which is the same answer the reference's `float` gives.
    assert!(
        (proficiency_stamina_multiplier(&[0.9, 0.9]) - 0.1).abs() < 1e-6,
        "and a bigger loss still only counts once"
    );
    assert_eq!(proficiency_stamina_multiplier(&[1.5]), 0.0, "**the floor stops it at free**, never at a rebate");
    assert_eq!(
        proficiency_stamina_multiplier(&[0.5, 0.0]),
        0.5,
        "a category that loses nothing leaves the multiplier where the first one left it"
    );
}

/// A proficiency's stat bonus is the sum over everything a creature knows, and archery is what lowers a bow's strength.
///
/// Reference: `proficiency_set::get_proficiency_bonus` (`proficiency.cpp:529-543`): **the sum runs over every known proficiency**, not over a weapon's category - a different shape from the
/// stamina multiplier, which walks categories. And `item::get_min_str` (`item.cpp:15673-15694`) is its one reader with a rule: a bow's requirement falls by the archery strength bonus and
/// **stops at zero**, because the reference's last line is `return min_str > 0 ? min_str : 0`.
#[test]
fn a_stat_bonus_adds_up_and_archery_lowers_a_bow() {
    use cdda_sim::proficiencies::bonus;

    let mut world = World::new();
    let dump = cdda_content::proficiencies::Proficiencies {
        by_id: [
            (
                "prof_athlete_basic".to_owned(),
                cdda_content::proficiencies::Proficiency {
                    id: "prof_athlete_basic".to_owned(),
                    seconds: 14_400,
                    can_learn: true,
                    required: Vec::new(),
                    bonuses: vec![("athlete".to_owned(), vec![("strength".to_owned(), 2.0)])],
                },
            ),
            (
                "prof_archery_master".to_owned(),
                cdda_content::proficiencies::Proficiency {
                    id: "prof_archery_master".to_owned(),
                    seconds: 57_600,
                    can_learn: true,
                    required: Vec::new(),
                    bonuses: vec![("archery".to_owned(), vec![("strength".to_owned(), 3.0)])],
                },
            ),
        ]
        .into_iter()
        .collect(),
    };
    cdda_sim::proficiencies::install(&mut world, &dump);

    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    assert_eq!(bonus(&mut world, survivor, "athlete", "strength"), 0.0, "nothing known, nothing added");
    cdda_sim::proficiencies::learn(&mut world, survivor, "prof_athlete_basic");
    assert_eq!(bonus(&mut world, survivor, "athlete", "strength"), 2.0, "a basic athlete is two strength");
    assert_eq!(
        bonus(&mut world, survivor, "archery", "strength"),
        0.0,
        "**and the sum is per category**: an athlete is no archer"
    );

    // The bow's requirement falls by the archery bonus and stops at zero.
    assert_eq!(cdda_content::items::min_strength_required(12, 0.0, true), 12, "a bow with no proficiency asks for twelve");
    cdda_sim::proficiencies::learn(&mut world, survivor, "prof_archery_master");
    let archery = bonus(&mut world, survivor, "archery", "strength");
    assert_eq!(cdda_content::items::min_strength_required(12, archery, true), 9, "**a master archer asks for nine**");
    assert_eq!(
        cdda_content::items::min_strength_required(12, archery, false),
        12,
        "and a gun that is not a bow keeps its requirement: the reduction is for bows only"
    );
    assert_eq!(
        cdda_content::items::min_strength_required(2, 9.0, true),
        0,
        "**and a requirement cannot go below nothing**"
    );
}

/// Drawing a bow costs stamina by how heavy it is against how strong the arm is, and the skills take a little back.
///
/// Reference: `mod_stamina_archery` (`ranged.cpp:1682-1701`):
///
/// ```text
/// const float str_ratio = static_cast<float>( relevant.get_min_str() ) / you.str_cur;
/// you.set_activity_level( 10 * str_ratio );
/// const float skill_modifier = ( 2.0f * archery_skill + athletics_skill ) / 3.0f;
/// const int stamina_cost = pow( 10.0f * str_ratio + 10 - skill_modifier, 2 );
/// you.mod_stamina( -stamina_cost );
/// ```
///
/// **The square is the rule**: a bow the arm can barely draw costs `( 10 + 10 - skills )^2`, so **four hundred** at no skill, and the same bow drawn by a strong arm costs far less because
/// `str_ratio` is a **ratio of the requirement to the strength** rather than a penalty. **The skill term is two thirds archery and one third athletics**, and it is subtracted **inside** the
/// square, which is why a point of archery is worth more than a point of the swimming skill the reference uses for athletics.
#[test]
fn drawing_a_bow_costs_stamina_by_its_weight() {
    use cdda_sim::shot::archery_stamina_cost;

    // A bow wanting ten, drawn by a strength of ten: ratio one, so ( 10 + 10 )^2 = 400.
    assert_eq!(archery_stamina_cost(10, 10.0, 0.0, 0.0), 400, "**a bow at the limit of the arm costs four hundred**");
    // Twice the strength halves the ratio: ( 5 + 10 )^2 = 225.
    assert_eq!(archery_stamina_cost(10, 20.0, 0.0, 0.0), 225, "and twice the arm costs two hundred and twenty-five");
    // Four archery points are worth two thirds each: 400 becomes ( 20 - 2.666 )^2 = 300.44, truncated to 300.
    assert_eq!(archery_stamina_cost(10, 10.0, 4.0, 0.0), 300, "four archery points take a hundred off");
    // **Three athletics points are worth one of modifier**, because the skill term divides by three: ( 20 - 1 )^2 = 361.
    assert_eq!(
        archery_stamina_cost(10, 10.0, 0.0, 3.0),
        361,
        "**and three of athletics are worth one point of modifier**"
    );
}

/// A shot's quality is read off how far it missed by, and each rung scales the damage differently.
///
/// Reference: `Creature::select_body_part_projectile_attack` (`creature.cpp:1172-1196`), whose ladder is
///
/// ```text
/// goodhit < accuracy_headshot (0.1) -> "Critical!!"   dmg *= 0.6 + 0.45*crit_mod - 0.1*hit_roll; dmg *= std_hit_mult + ( crit_mult - std_hit_mult )*crit_mod
/// goodhit < accuracy_critical (0.2) -> "Critical!"    dmg *= 0.75 + 0.25*crit_mod - 0.25*hit_roll; dmg *= std_hit_mult + ( crit_mult - std_hit_mult )*crit_mod
/// goodhit < accuracy_goodhit  (0.5) -> "Good hit!"    dmg *= 1.0 - 0.5*hit_roll; dmg *= std_hit_mult
/// goodhit < accuracy_standard (0.8) -> no message      dmg *= 1.5 - hit_roll
/// goodhit < accuracy_grazing  (1.0) -> "Grazing hit."  dmg *= 1.05 - hit_roll
/// ```
/// with `std_hit_mult = sqrt( 2 * crit_multiplier )`.
///
/// **Only the two best rungs use `crit_mod` and the critical multiplier** - the good hit already halves the roll's weight and multiplies by the standard figure - so a port that applied
/// `crit_multiplier` at every rung would make a graze as strong as a hit. **The thresholds are exclusive**: a shot that misses by exactly **0.5** is not a good hit, it is a
/// standard one.
#[test]
fn a_shot_is_graded_by_how_far_it_missed() {
    use cdda_sim::shot::{Grade, grade_of};

    assert_eq!(grade_of(0.05), Grade::Headshot, "under a tenth is a critical");
    assert_eq!(grade_of(0.1), Grade::Critical, "**exactly a tenth is merely critical**");
    assert_eq!(grade_of(0.2), Grade::Good, "and a fifth is a good hit");
    assert_eq!(grade_of(0.5), Grade::Standard, "**half is the standard hit, not the good one**");
    assert_eq!(grade_of(0.8), Grade::Grazing, "four fifths is a graze");
    assert_eq!(grade_of(0.99), Grade::Grazing, "and so is anything under a full miss");
    assert_eq!(grade_of(1.0), Grade::Miss, "a shot that misses by a full square is not a hit at all");
}

/// A grade's damage multiplier is the reference's own branch arithmetic, and only the top two rungs reach the critical figure.
///
/// Reference: `Creature::select_body_part_projectile_attack` (`creature.cpp:1172-1196`), with `std_hit_mult = sqrt( 2 * crit_multiplier )`:
///
/// ```text
/// headshot: ( 0.6 + 0.45*crit_mod - 0.1*hit_roll ) * ( std_hit_mult + ( crit_multiplier - std_hit_mult ) * crit_mod )
/// critical: ( 0.75 + 0.25*crit_mod - 0.25*hit_roll ) * ( std_hit_mult + ( crit_multiplier - std_hit_mult ) * crit_mod )
/// good:     ( 1.0 - 0.5*hit_roll ) * std_hit_mult
/// standard: 1.5 - hit_roll
/// grazing:  1.05 - hit_roll
/// ```
///
/// **The standard rung's multiplier is above one** (`1.5 - hit_roll`, so 1.5 down to 0.7) while the good rung's is around `sqrt(3)` - so a *worse* rung can hit harder than a better one for
/// an unlucky roll, which is the reference's arithmetic rather than a defect. **A graze is `1.05 - hit_roll`**, tiny because `hit_roll` is near one when the shot is far off.
#[test]
fn a_grade_multiplies_the_damage_its_own_way() {
    use cdda_sim::shot::{Grade, damage_multiplier};

    // crit_multiplier 1.5 makes std_hit_mult sqrt( 3 ) = 1.7320508.
    let crit = 1.5;
    assert!(
        (damage_multiplier(Grade::Good, 1.0, 0.0, crit) - 3.0f64.sqrt()).abs() < 1e-9,
        "**a perfect good hit is the standard figure alone**"
    );
    assert!(
        (damage_multiplier(Grade::Good, 1.0, 1.0, crit) - 3.0f64.sqrt() / 2.0).abs() < 1e-9,
        "and an unlucky one is half of it"
    );
    assert!((damage_multiplier(Grade::Standard, 1.0, 0.0, crit) - 1.5).abs() < 1e-9, "a standard hit at the top is 1.5");
    assert!((damage_multiplier(Grade::Standard, 1.0, 0.5, crit) - 1.0).abs() < 1e-9, "and in the middle is 1.0");
    assert!((damage_multiplier(Grade::Grazing, 1.0, 0.05, crit) - 1.0).abs() < 1e-9, "a graze is `1.05 - hit_roll`");
    assert!(
        (damage_multiplier(Grade::Headshot, 1.0, 0.0, crit) - 1.05 * crit).abs() < 1e-9,
        "**a headshot with a full crit_mod reaches the critical figure itself**: 1.05 * 1.5"
    );
    // **A critical at full `crit_mod` reaches the critical figure itself**, not three quarters of it: `0.75 + 0.25 * crit_mod` is one when `crit_mod` is one.
    assert!(
        (damage_multiplier(Grade::Critical, 1.0, 0.0, crit) - crit).abs() < 1e-9,
        "and a critical at a full crit_mod is the critical figure too"
    );
    assert!(
        (damage_multiplier(Grade::Critical, 0.0, 0.0, crit) - 0.75 * 3.0f64.sqrt()).abs() < 1e-9,
        "while a critical with no crit_mod is three quarters of the standard figure"
    );
    assert_eq!(damage_multiplier(Grade::Miss, 1.0, 0.0, crit), 0.0, "**a miss multiplies nothing**, because it never landed");
}
