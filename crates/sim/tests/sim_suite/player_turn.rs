//! The player's own turn, and the first mutual exchange.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    MonsterKindId, Player, Position, TurnRules, monster_turns_striking, player_turn,
    spawn_monster, spawn_player,
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
        blow_of: &|_| (1, 4, 0),
        armour_of: &|_| 0,
        passable: &|terrain| terrain == FLOOR,
        // A skilled survivor: accuracy four, which is a mean roll of twenty.
        player_accuracy_tenths: 40,
    }
}

fn player(world: &mut World) -> Entity {
    let mut query = world.query_filtered::<Entity, With<Player>>();
    query.iter(world).next().expect("a player")
}

fn monster_hp(world: &World, entity: Entity) -> i32 {
    cdda_sim::creature::health_of(world, entity).unwrap_or(-1)
}

#[test]
fn the_player_walks_toward_where_it_is_aiming() {
    let mut world = World::new();
    let me = spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::creature::set_moves(&mut world, me, 100);
    let mut rolls = Rolls::new(1);
    let landed = player_turn(
        &mut world,
        (9, 0),
        (1, 4, 0),
        100,
        &rules(),
        &mut rolls,
        &level(),
    );
    assert_eq!(landed, 0, "nothing to hit yet");
    let position = world.get::<Position>(me).expect("a position");
    assert_eq!(
        (position.x, position.y),
        (1, 0),
        "one step for one turn's moves"
    );
}

#[test]
fn the_player_strikes_what_is_next_to_it() {
    let mut world = World::new();
    let me = spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::creature::set_moves(&mut world, me, 100);
    let zombie = spawn_monster(&mut world, (1, 0), MonsterKindId(1), 30);
    let mut rolls = Rolls::new(2);
    let landed = player_turn(
        &mut world,
        (1, 0),
        (1, 4, 0),
        100,
        &rules(),
        &mut rolls,
        &level(),
    );
    assert_eq!(landed, 1, "one blow");
    assert!(monster_hp(&world, zombie) < 30, "and the zombie felt it");
}

#[test]
fn the_player_cannot_walk_into_the_zombie() {
    let mut world = World::new();
    let me = spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::creature::set_moves(&mut world, me, 100);
    spawn_monster(&mut world, (1, 0), MonsterKindId(1), 30);
    let mut rolls = Rolls::new(3);
    // Aiming at an empty square behind the zombie: the step is blocked, and striking is not what was asked.
    player_turn(
        &mut world,
        (3, 0),
        (1, 4, 0),
        100,
        &rules(),
        &mut rolls,
        &level(),
    );
    let position = world.get::<Position>(me).expect("a position");
    assert_eq!(
        (position.x, position.y),
        (0, 0),
        "it did not walk through the zombie"
    );
}

#[test]
fn a_fight_runs_both_ways_and_the_same_seed_replays_it() {
    let fight = |seed: u32| {
        let mut world = World::new();
        let me = spawn_player(&mut world, (0, 0), 80, 1);
        let zombie = spawn_monster(&mut world, (1, 0), MonsterKindId(1), 30);
        let mut rolls = Rolls::new(seed);
        for _ in 0..3 {
            // The world hands out moves, then both sides take their turn.
            cdda_sim::creature::set_moves(&mut world, me, 100);
            {
                let mut monsters = world.query::<&mut cdda_sim::creature::Moves>();
                for mut monster in monsters.iter_mut(&mut world) {
                    monster.left = 100;
                }
            }
            player_turn(
                &mut world,
                (1, 0),
                (1, 6, 1),
                100,
                &rules(),
                &mut rolls,
                &level(),
            );
            monster_turns_striking(&mut world, (0, 0), &rules(), &mut rolls, &level());
        }
        let me_entity = player(&mut world);
        let my_hp = cdda_sim::creature::health_of(&world, me_entity).expect("a player");
        (my_hp, monster_hp(&world, zombie))
    };
    let first = fight(77);
    assert_eq!(first, fight(77), "the same seed is the same fight");
    assert!(first.0 < 80, "the player was hurt: {}", first.0);
    assert!(first.1 < 30, "and so was the zombie: {}", first.1);
}

/// The player's blow is dodgeable, because it goes through the same rules a monster's does.
///
/// Reference: `Character::melee_attack` makes the same roll `monster::melee_attack` does - the attacker's
/// accuracy against the target's dodge - so a nimble target avoids a survivor's swing exactly as it avoids a
/// zombie's. Before this, the player's path took armour off a blow and never asked whether it landed, which
/// meant no target could dodge the avatar at all.
#[test]
fn a_dodging_monster_avoids_the_player_s_blow() {
    let mut world = World::new();
    let me = spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::creature::set_moves(&mut world, me, 100);
    let plain = spawn_monster(&mut world, (1, 0), MonsterKindId(2), 100);
    let mut rolls = Rolls::new(9);
    player_turn(&mut world, (1, 0), (1, 4, 0), 100, &rules(), &mut rolls, &level());
    let plain_left = monster_hp(&world, plain);

    let mut world = World::new();
    let me = spawn_player(&mut world, (0, 0), 80, 0);
    cdda_sim::creature::set_moves(&mut world, me, 100);
    let nimble = spawn_monster(&mut world, (1, 0), MonsterKindId(2), 100);
    world
        .entity_mut(nimble)
        .insert(cdda_sim::creature::Dodge { points: 20 });
    let mut rolls = Rolls::new(9);
    player_turn(&mut world, (1, 0), (1, 4, 0), 100, &rules(), &mut rolls, &level());
    let nimble_left = monster_hp(&world, nimble);

    assert!(
        plain_left < 100,
        "a target with no dodge takes the blow: {plain_left} left"
    );
    assert_eq!(
        nimble_left, 100,
        "and twenty points of dodge - a hundred points of roll - turns it aside entirely"
    );
}
