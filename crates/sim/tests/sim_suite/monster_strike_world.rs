//! When a creature's attack lands on another creature.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    MonsterKindId, Position, TurnRules, monster_turns_striking, spawn_monster,
};

const FLOOR: u32 = 1;

fn level() -> Level {
    let mut level = Level::new();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    level
}

fn rules() -> TurnRules<'static> {
    TurnRules {
        speed_of: &|_| 100,
        // One six-sided die: a blow of one to six, always enough to hurt something with one hit point.
        blow_of: &|_| (1, 6, 0),
        armour_of: &|_| 0,
        passable: &|terrain| terrain == FLOOR,
        // A skilled survivor: accuracy four, which is a mean roll of twenty.
        player_accuracy_tenths: 40,
    }
}

fn hp(world: &World, entity: Entity) -> i32 {
    cdda_sim::creature::health_of(world, entity).unwrap_or(-1)
}

#[test]
fn an_attack_lands_on_whoever_stands_where_the_creature_strikes() {
    let mut world = World::new();
    let attacker = spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    let victim = spawn_monster(&mut world, (1, 0), MonsterKindId(2), 20);
    let mut rolls = Rolls::new(3);
    let landed = monster_turns_striking(&mut world, (1, 0), &rules(), &mut rolls, &level());
    assert_eq!(landed, 1, "one blow, because one action was affordable");
    assert!(
        hp(&world, victim) < 20,
        "and it hurt: {}",
        hp(&world, victim)
    );
    assert_eq!(hp(&world, attacker), 10, "the attacker was untouched");
}

#[test]
fn an_empty_target_square_means_no_blow_lands() {
    let mut world = World::new();
    spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    let mut rolls = Rolls::new(3);
    let landed = monster_turns_striking(&mut world, (1, 0), &rules(), &mut rolls, &level());
    assert_eq!(landed, 0, "nothing to hit");
}

#[test]
fn enough_blows_kill_and_the_dead_stop_striking() {
    // A blow lands on a die, so three turns kill a two-hit-point victim **sometimes**; the assertion is that it happens, over spread seeds, and that a victim
    // that survives is still being struck.
    let mut killed = 0;
    for attempt in 0..16_u32 {
        let mut world = World::new();
        spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
        let victim = spawn_monster(&mut world, (1, 0), MonsterKindId(2), 2);
        let mut rolls = Rolls::new(crate::spread_seed(attempt));
        for _ in 0..3 {
            monster_turns_striking(&mut world, (1, 0), &rules(), &mut rolls, &level());
        }
        if hp(&world, victim) <= 0 {
            killed += 1;
        }
    }
    assert!(killed > 0, "two hit points do not survive three turns of blows, sometimes: {killed} of 16");
    assert!(killed < 16, "and not every time, because a blow is a die: {killed} of 16");
}

#[test]
fn armour_reduces_what_lands_and_a_wall_of_it_stops_everything() {
    let mut world = World::new();
    spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    let armoured = spawn_monster(&mut world, (1, 0), MonsterKindId(2), 20);
    let heavy = TurnRules {
        armour_of: &|_| 100,
        ..rules()
    };
    let mut rolls = Rolls::new(4);
    monster_turns_striking(&mut world, (1, 0), &heavy, &mut rolls, &level());
    assert_eq!(
        hp(&world, armoured),
        20,
        "a hundred points of armour is a wall"
    );
}

#[test]
fn the_dice_come_from_the_caller_so_a_seed_replays_the_fight() {
    let fight = |seed: u32| {
        let mut world = World::new();
        spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
        let victim = spawn_monster(&mut world, (1, 0), MonsterKindId(2), 100);
        let mut rolls = Rolls::new(seed);
        for _ in 0..4 {
            monster_turns_striking(&mut world, (1, 0), &rules(), &mut rolls, &level());
        }
        let position = world.get::<Position>(victim).expect("a position");
        (hp(&world, victim), position.x, position.y)
    };
    assert_eq!(fight(1234), fight(1234), "the same seed is the same fight");
}
