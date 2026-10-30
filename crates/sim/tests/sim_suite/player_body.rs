//! The player as something that can be hit.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    MonsterKindId, Player, TurnRules, monster_turns_striking, player_dead, spawn_monster,
    spawn_player, strike_player,
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
    }
}

fn player_hp(world: &World, entity: Entity) -> i32 {
    world.get::<Player>(entity).map_or(-1, |player| player.hp)
}

#[test]
fn the_player_arrives_at_full_health_with_its_armour() {
    let mut world = World::new();
    let player = spawn_player(&mut world, (5, 5), 80, 2);
    let state = world.get::<Player>(player).expect("a player");
    assert_eq!(state.hp, 80);
    assert_eq!(state.hp_max, 80);
    assert_eq!(state.armour, 2);
    assert!(!player_dead(&mut world));
}

#[test]
fn armour_takes_what_it_can_from_a_blow_on_the_player() {
    let mut world = World::new();
    let player = spawn_player(&mut world, (5, 5), 20, 3);
    assert!(
        !strike_player(&mut world, player, 10),
        "seven through: thirteen left"
    );
    assert_eq!(player_hp(&world, player), 13);
    assert!(
        !strike_player(&mut world, player, 2),
        "less than the armour: nothing"
    );
    assert_eq!(player_hp(&world, player), 13);
}

#[test]
fn the_player_dies_at_zero_and_the_death_is_visible() {
    let mut world = World::new();
    let player = spawn_player(&mut world, (5, 5), 5, 0);
    assert!(strike_player(&mut world, player, 5), "exactly enough");
    assert!(player_dead(&mut world));
}

#[test]
fn a_monster_next_to_the_player_wounds_the_player() {
    let mut world = World::new();
    let player = spawn_player(&mut world, (1, 0), 100, 0);
    spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    let mut rolls = Rolls::new(21);
    let landed = monster_turns_striking(&mut world, (1, 0), &rules(), &mut rolls, &level());
    assert_eq!(landed, 1, "one blow reached the player");
    assert!(
        player_hp(&world, player) < 100,
        "and it hurt: {}",
        player_hp(&world, player)
    );
}

#[test]
fn the_player_stops_being_a_target_at_zero_health() {
    let mut world = World::new();
    let player = spawn_player(&mut world, (1, 0), 10, 0);
    spawn_monster(&mut world, (0, 0), MonsterKindId(1), 10);
    let mut rolls = Rolls::new(22);
    // A turn spends the creature's moves, and granting them back is the world's job (`regen_moves`), so the
    // test does what a world would: hand the monster its speed each turn.
    for _ in 0..20 {
        {
            let mut monsters = world.query::<&mut cdda_sim::creature::Moves>();
            for mut monster in monsters.iter_mut(&mut world) {
                monster.left = 100;
            }
        }
        monster_turns_striking(&mut world, (1, 0), &rules(), &mut rolls, &level());
    }
    assert!(player_hp(&world, player) <= 0, "the player is dead");
    let after = monster_turns_striking(&mut world, (1, 0), &rules(), &mut rolls, &level());
    assert_eq!(after, 0, "and a corpse is not struck again");
}
