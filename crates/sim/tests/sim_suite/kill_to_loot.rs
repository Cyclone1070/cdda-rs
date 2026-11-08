//! The whole chain in one test: kill it, reap it, butcher it, pick the loot up.
//!
//! Every rule in this chain was tested on its own as it was ported. This is the test that would have caught the
//! joins being wrong even when every part was right - a corpse that names the wrong kind, a harvest drawn from the
//! wrong weight, a butchery that finishes without dropping anything.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    ButcherAction, ButcherFinish, ButcherState, CorpseDamage, CreatureSize, GroundItem,
    HarvestEntry, HarvestSpec, HarvestTable, ItemTypeId, MonsterKindId, Position,
    butcher_finish, butchery_moves_left, butchery_turn, corpses_at, items_at, reap_dead_monsters,
    spawn_monster, start_butchery, strike,
};

const CORPSE: ItemTypeId = ItemTypeId(3_000);
const MEAT: ItemTypeId = ItemTypeId(101);
const BONE: ItemTypeId = ItemTypeId(102);

fn butcher_state(action: ButcherAction) -> ButcherState {
    ButcherState {
        size: CreatureSize::Medium,
        action,
        tool_factor: 0,
        field_dressed: false,
        field_dress_failed: false,
        quartered: false,
        instant_bleed: false,
        butchering_basic: 0.0,
        butchering_advanced: 0.0,
        skinning_basic: 0.0,
        helpers: 0,
    }
}

fn drops(name: &str) -> ItemTypeId {
    match name {
        "meat" => MEAT,
        "bone" => BONE,
        _ => ItemTypeId(0),
    }
}

fn corpse_square(world: &mut World) -> (i32, i32) {
    let mut query = world.query::<&Position>();
    let position = query
        .iter(world)
        .next()
        .expect("something is standing somewhere");
    (position.x, position.y)
}

#[test]
fn a_kill_becomes_loot_with_every_rule_in_the_chain_agreeing() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (5, 5), MonsterKindId(7), 12);
    // It dies where it stood.
    assert!(
        strike(&mut world, zombie, 20, 0),
        "twelve hit points against twenty"
    );
    assert_eq!(reap_dead_monsters(&mut world, CORPSE, |_| 62_499), 1, "one body reaped");
    assert_eq!(
        corpses_at(&mut world, (5, 5)),
        1,
        "a corpse item on its square"
    );
    assert!(items_at(&mut world, (5, 5)).len() == 1, "which is an item");
    assert!(
        world.get_entity(zombie).is_err(),
        "and the creature is gone"
    );

    // The corpse is butchered, over as many turns as the time rule says.
    let corpse = items_at(&mut world, (5, 5))
        .first()
        .copied()
        .expect("the corpse");
    let total = start_butchery(&mut world, corpse, butcher_state(ButcherAction::Quick))
        .expect("a corpse can be butchered");
    assert_eq!(butchery_moves_left(&world, corpse), Some(total));
    assert_eq!(
        butchery_turn(&mut world, corpse, total),
        cdda_sim::ButcheryStep::Finished
    );

    // And the yields land where the body was.
    let entries = [
        HarvestSpec {
            part: Some(HarvestEntry::Flesh),
            base_num: (3, 3),
            scale_num: (0.0, 0.0),
            mass_ratio: None,
            max: None,
        },
        HarvestSpec {
            part: Some(HarvestEntry::Bone),
            base_num: (2, 2),
            scale_num: (0.0, 0.0),
            mass_ratio: None,
            max: None,
        },
    ];
    let mut rolls = Rolls::new(2468);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat", "bone"],
            weight: 0.0,
        },
        ButcherFinish {
            action: cdda_sim::ButcherAction::Full,
            size: cdda_sim::CreatureSize::Medium,
            butchery: 0,
            yield_multiplier: 1.0,
            damage: CorpseDamage::Light,
            gibbed: false,
    field_dressed: false,
    field_dress_failed: false,
            skinned: false,
        },
        drops,
        |_| 1_000,
        &mut rolls,
    );
    assert_eq!(
        dropped,
        vec![(MEAT, 3), (BONE, 2)],
        "three meat and two bone"
    );
    let loot = items_at(&mut world, (5, 5));
    assert_eq!(loot.len(), 2, "two stacks where the creature fell");
    let mut counts: Vec<(ItemTypeId, i32)> = loot
        .iter()
        .filter_map(|entity| world.get::<GroundItem>(*entity))
        .map(|stack| (stack.kind, stack.charges))
        .collect();
    counts.sort_by_key(|(kind, _)| kind.0);
    assert_eq!(counts, vec![(MEAT, 3), (BONE, 2)]);
}

#[test]
fn the_same_seed_kills_the_same_creature_and_butchers_the_same_loot() {
    let run = |seed: u32| {
        let mut world = World::new();
        let zombie = spawn_monster(&mut world, (1, 1), MonsterKindId(7), 12);
        strike(&mut world, zombie, 20, 0);
        reap_dead_monsters(&mut world, CORPSE, |_| 62_499);
        let square = corpse_square(&mut world);
        let corpse = items_at(&mut world, square)
            .first()
            .copied()
            .expect("corpse");
        let total = start_butchery(&mut world, corpse, butcher_state(ButcherAction::Quick))
            .expect("butcherable");
        butchery_turn(&mut world, corpse, total);
        let entries = [HarvestSpec {
            part: Some(HarvestEntry::Flesh),
            base_num: (1, 6),
            scale_num: (0.0, 0.0),
            mass_ratio: None,
            max: None,
        }];
        let mut rolls = Rolls::new(seed);
        butcher_finish(
            &mut world,
            corpse,
            HarvestTable {
                entries: &entries,
                ids: &["meat"],
                weight: 0.0,
            },
            ButcherFinish {
                action: cdda_sim::ButcherAction::Full,
                size: cdda_sim::CreatureSize::Medium,
                yield_multiplier: 1.0,
                butchery: 0,
                damage: CorpseDamage::Light,
                gibbed: false,
    field_dressed: false,
    field_dress_failed: false,
                skinned: false,
            },
            drops,
            |_| 1_000,
            &mut rolls,
        )
    };
    assert_eq!(run(4321), run(4321), "one seed, one outcome");
}

#[test]
fn a_reaped_body_that_is_never_butchered_stays_a_corpse() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (2, 2), MonsterKindId(7), 40);
    strike(&mut world, zombie, 40, 0);
    reap_dead_monsters(&mut world, CORPSE, |_| 62_499);
    assert_eq!(corpses_at(&mut world, (2, 2)), 1, "still lying there");
    assert!(items_at(&mut world, (2, 2)).len() == 1, "and nothing else");
    let mut monsters = world.query::<&cdda_sim::creature::Creature>();
    assert_eq!(
        monsters.iter(&world).count(),
        0,
        "with nothing left to fight"
    );
}
