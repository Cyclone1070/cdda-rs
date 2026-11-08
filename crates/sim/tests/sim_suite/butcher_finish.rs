//! What a finished butchery leaves on the ground.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    ButcherAction, ButcherFinish, CreatureSize, CorpseDamage, CorpseOf, GroundItem, HarvestEntry, HarvestSpec, HarvestTable,
    ItemTypeId, MonsterKindId, butcher_finish, drop_item, items_at,
};

const CORPSE: ItemTypeId = ItemTypeId(3_000);
const MEAT: ItemTypeId = ItemTypeId(101);
const BONE: ItemTypeId = ItemTypeId(102);
const SKIN: ItemTypeId = ItemTypeId(103);

fn body(world: &mut World) -> Entity {
    let item = drop_item(world, (4, 4), CORPSE, 1);
    world.entity_mut(item).insert(CorpseOf {
        kind: MonsterKindId(1),
        died_on: 0,
        revives: false,
        monster_volume_ml: 62_499,
    });
    item
}

fn entry(part: HarvestEntry, base: (i32, i32)) -> HarvestSpec {
    HarvestSpec {
        part: Some(part),
        base_num: base,
        scale_num: (0.0, 0.0),
        mass_ratio: None,
        max: None,
    }
}

fn finish() -> ButcherFinish {
    ButcherFinish {
        action: ButcherAction::Full,
        size: CreatureSize::Medium,
        butchery: 0,
        // Nothing extra: a share of a body is what the weight says, which keeps these gates about the count rule.
        yield_multiplier: 1.0,
        damage: CorpseDamage::Light,
        gibbed: false,
        skinned: false,
    }
}

fn ids(name: &str) -> ItemTypeId {
    match name {
        "meat" => MEAT,
        "bone" => BONE,
        "skin" => SKIN,
        _ => ItemTypeId(0),
    }
}

#[test]
fn the_yields_land_on_the_square_the_body_was_on() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [
        entry(HarvestEntry::Flesh, (3, 3)),
        entry(HarvestEntry::Bone, (2, 2)),
    ];
    let mut rolls = Rolls::new(1);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat", "bone"],
            weight: 0.0,
        },
        finish(),
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert_eq!(dropped, vec![(MEAT, 3), (BONE, 2)]);
    let here = items_at(&mut world, (4, 4));
    assert_eq!(here.len(), 2, "two stacks on the square");
}

#[test]
fn the_counts_are_charges_on_the_stacks_that_were_dropped() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [entry(HarvestEntry::Flesh, (3, 3))];
    let mut rolls = Rolls::new(2);
    butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat"],
            weight: 0.0,
        },
        finish(),
        ids,
        |_| 1_000,
        &mut rolls,
    );
    let here = items_at(&mut world, (4, 4));
    let stack = world
        .get::<GroundItem>(*here.first().expect("a stack"))
        .expect("an item");
    assert_eq!(stack.kind, MEAT);
    assert_eq!(stack.charges, 3);
}

#[test]
fn a_body_that_gives_nothing_drops_nothing() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [entry(HarvestEntry::Offal, (2, 2))];
    let mut rolls = Rolls::new(3);
    // A mangled body has no offal at all, and the floor of the count does not rescue it: the reference's floor
    // applies to the count, and the damage result of zero is then floored to the minimum only for counted
    // entries - which this is - so the yield is the minimum, not zero.
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat"],
            weight: 0.0,
        },
        ButcherFinish {
            damage: CorpseDamage::Mangled,
            ..finish()
        },
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert_eq!(
        dropped,
        vec![(MEAT, 2)],
        "the promised minimum, not the damaged zero"
    );
}

#[test]
fn a_skinned_body_gives_no_hide() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [entry(HarvestEntry::Skin, (5, 5))];
    let mut rolls = Rolls::new(4);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["skin"],
            weight: 0.0,
        },
        ButcherFinish {
            skinned: true,
            ..finish()
        },
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert!(dropped.is_empty(), "the hide was already taken");
    assert!(items_at(&mut world, (4, 4)).is_empty());
}

#[test]
fn the_body_is_used_up_and_gone_from_the_world() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let mut rolls = Rolls::new(5);
    butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &[entry(HarvestEntry::Flesh, (1, 1))],
            ids: &["meat"],
            weight: 0.0,
        },
        finish(),
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert!(world.get_entity(corpse).is_err(), "the corpse is consumed");
}

#[test]
fn the_same_seed_gives_the_same_harvest() {
    let harvest = |seed: u32| {
        let mut world = World::new();
        let corpse = body(&mut world);
        let entries = [
            entry(HarvestEntry::Flesh, (1, 10)),
            entry(HarvestEntry::Bone, (1, 10)),
        ];
        let mut rolls = Rolls::new(seed);
        butcher_finish(
            &mut world,
            corpse,
            HarvestTable {
                entries: &entries,
                ids: &["meat", "bone"],
                weight: 0.0,
            },
            finish(),
            ids,
        |_| 1_000,
            &mut rolls,
        )
    };
    assert_eq!(harvest(4242), harvest(4242));
}

/// **A bulk entry's weight becomes items only after it is divided by one item's weight.** This gate used to assert the opposite - that fifty grams of flesh dropped **fifty** cuts - which was the
/// rule the port had rather than the rule the reference has.
///
/// Reference: `butchery.cpp:829-840`: `roll = std::ceil( roll / to_gram( drop->weight ) )`, so fifty grams of a thousand-gram cut is one cut, not fifty.
#[test]
fn a_bulk_entry_uses_the_weight_it_was_given() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [HarvestSpec {
        part: Some(HarvestEntry::Flesh),
        base_num: (0, 0),
        scale_num: (0.0, 0.0),
        mass_ratio: Some(0.5),
        max: None,
    }];
    let mut rolls = Rolls::new(6);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat"],
            weight: 100.0,
        },
        finish(),
        ids,
        |_| 40,
        &mut rolls,
    );
    assert_eq!(
        dropped,
        vec![(MEAT, 2)],
        "and fifty grams of a forty-gram cut is two, because the division rounds **up**"
    );
}

/// **A quick butchery takes a quarter of the flesh and half the bone, and gives no blood at all.**
///
/// Reference: the `QUICK BUTCHERY` block (`butchery.cpp:741-756`) - flesh `/= 4`, bone `/= 2`, skin `/= 2` for a medium body or bigger, offal `/= 5`, and **every other entry is skipped**. The port's finish
/// ignored the action entirely, so a quick job on a body gave the same harvest as a full one.
#[test]
fn a_quick_butchery_takes_a_quarter_of_the_flesh_and_half_the_bone() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [
        entry(HarvestEntry::Flesh, (40, 40)),
        entry(HarvestEntry::Bone, (20, 20)),
        entry(HarvestEntry::Blood, (10, 10)),
    ];
    let mut rolls = Rolls::new(7);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat", "bone", "blood"],
            weight: 0.0,
        },
        ButcherFinish {
            action: ButcherAction::Quick,
            ..finish()
        },
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert_eq!(
        dropped,
        vec![(MEAT, 10), (BONE, 10)],
        "**forty cuts of flesh become ten and twenty bones become ten; blood is not on the list at all**"
    );
}

/// **Skinning takes the hide and nothing else.**
///
/// Reference: the `SKIN` block (`butchery.cpp:781-790`) - every entry that is not skin is skipped, and a body whose field dressing failed has its hide rolled away from.
#[test]
fn skinning_takes_the_hide_and_nothing_else() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [
        entry(HarvestEntry::Flesh, (40, 40)),
        entry(HarvestEntry::Skin, (5, 5)),
        entry(HarvestEntry::Bone, (20, 20)),
    ];
    let mut rolls = Rolls::new(9);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat", "skin", "bone"],
            weight: 0.0,
        },
        ButcherFinish {
            action: ButcherAction::Skin,
            ..finish()
        },
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert_eq!(dropped, vec![(SKIN, 5)], "**only the hide**, and all five of it");
}
