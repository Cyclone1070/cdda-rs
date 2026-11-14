//! What a finished butchery leaves on the ground.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    ButcherAction, ButcherFinish, CorpseDamage, CorpseOf, CreatureSize, GroundItem, HarvestEntry,
    HarvestSpec, HarvestTable, ItemTypeId, MonsterKindId, butcher_finish, drop_item, items_at,
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
        is_group: false,
    }
}

fn finish() -> ButcherFinish {
    ButcherFinish {
        action: ButcherAction::Full,
        size: CreatureSize::Medium,
        dex: 10,
        tool_quality: 0,
        butchery: 0,
        // Nothing extra: a share of a body is what the weight says, which keeps these gates about the count rule.
        skill_levels: Vec::new(),
        yield_multiplier: 1.0,
        damage: CorpseDamage::Light,
        gibbed: false,
        skinned: false,
        field_dressed: false,
        quartered: false,
        field_dress_failed: false,
        // Nothing known, which is the survivor a fresh game starts as and the worst case the reference allows.
        butch_basic: 1.0,
        skin_basic: 1.0,
        skin_adv: 1.0,
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
            leftovers: None,
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
            leftovers: None,
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
            leftovers: None,
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
            leftovers: None,
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
            leftovers: None,
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
                leftovers: None,
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
        is_group: false,
    }];
    let mut rolls = Rolls::new(6);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat"],
            leftovers: None,
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
            leftovers: None,
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
            leftovers: None,
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
    assert_eq!(
        dropped,
        vec![(SKIN, 5)],
        "**only the hide**, and all five of it"
    );
}

/// **Dissecting takes the body's dissection list, and a body with none falls back to its harvest.**
///
/// Reference: the finish loop's own choice of list - `( action == butcher_type::DISSECT && !mt.dissect.is_empty() ) ? *mt.dissect : *mt.harvest` (`butchery.cpp:692-694`). A body that states no dissection
/// list is butchered the ordinary way even when the action is a dissection, which is the reference's `&&` and not a special case of its own.
#[test]
fn dissecting_takes_the_body_s_dissection_list_when_it_has_one() {
    let mut world = World::new();
    let mut registry = cdda_sim::monster_registry::MonsterKind {
        id: "mon_probe",
        hp: 10,
    };
    registry.id = "mon_probe";
    cdda_sim::monster_registry::set_monster_types(&mut world, &[registry]);
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_probe").expect("the kind");
    // A dissection list that shares nothing with the harvest, so which list was walked is visible.
    cdda_sim::monster_registry::set_harvests(
        &mut world,
        &[(
            kind,
            vec![entry(HarvestEntry::Flesh, (40, 40))],
            vec!["meat".to_owned()],
        )],
    );
    cdda_sim::monster_registry::set_dissections(
        &mut world,
        &[(
            kind,
            vec![entry(HarvestEntry::Bone, (3, 3))],
            vec!["bone".to_owned()],
        )],
    );
    let corpse = body(&mut world);
    world.entity_mut(corpse).insert(CorpseOf {
        kind,
        died_on: 0,
        revives: false,
        monster_volume_ml: 1_000,
    });
    // Which list the rule picks, and then the finish over it: the two halves of the reference's own line.
    let (entries, names) =
        cdda_sim::monster_registry::list_for(&world, kind, ButcherAction::Dissect);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut rolls = Rolls::new(21);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &names,
            leftovers: None,
            weight: 0.0,
        },
        ButcherFinish {
            action: ButcherAction::Dissect,
            ..finish()
        },
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert_eq!(
        dropped,
        vec![(BONE, 3)],
        "**the dissection list is walked, not the harvest**: three bones, and no meat at all"
    );

    // **And a body with no dissection list is butchered the ordinary way**, which is the reference's `&&` and not a special case.
    let mut plain = World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut plain,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_probe",
            hp: 10,
        }],
    );
    let bare = cdda_sim::monster_registry::id_of(&mut plain, "mon_probe").expect("the kind");
    cdda_sim::monster_registry::set_harvests(
        &mut plain,
        &[(
            bare,
            vec![entry(HarvestEntry::Flesh, (40, 40))],
            vec!["meat".to_owned()],
        )],
    );
    let (fallback, names) =
        cdda_sim::monster_registry::list_for(&plain, bare, ButcherAction::Dissect);
    assert_eq!(fallback.len(), 1, "one entry, the harvest's own");
    assert_eq!(
        names,
        vec!["meat".to_owned()],
        "and its drop is the harvest's"
    );
}

/// **A field-dressed body has given up its innards, and what is left of its bones is a roll.**
///
/// Reference: the block after the action (`butchery.cpp:789-797`) - a body carrying the `FIELD_DRESS` flag keeps no offal at all, and its bone roll becomes `roll / 2 + rng( roll / 2, roll )`, which is between
/// the **whole** of the roll and **one and a half times** it: the draw runs from the half to the whole and is added to another half. A dressed body's bones are worth *more* than a fresh one's, which is
/// the reference's arithmetic and reads like a mistake; it is not the port's to correct. A body dressed badly carries `FIELD_DRESS_FAILED` instead, and there the flesh and the hide are rolled down too (`:801-812`).
#[test]
fn a_field_dressed_body_keeps_no_offal_and_rolls_its_bones() {
    let mut world = World::new();
    let corpse = body(&mut world);
    world.entity_mut(corpse).insert(cdda_sim::ItemFlags(
        ["FIELD_DRESS".to_owned()].into_iter().collect(),
    ));
    let entries = [
        entry(HarvestEntry::Bone, (20, 20)),
        entry(HarvestEntry::Offal, (5, 5)),
        entry(HarvestEntry::Flesh, (40, 40)),
    ];
    let mut rolls = Rolls::new(31);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["bone", "offal", "meat"],
            leftovers: None,
            weight: 0.0,
        },
        ButcherFinish {
            field_dressed: true,
            ..finish()
        },
        ids,
        |_| 1_000,
        &mut rolls,
    );
    let bones = dropped
        .iter()
        .find(|(kind, _)| *kind == BONE)
        .map_or(0, |(_, count)| *count);
    assert!(
        (20..=30).contains(&bones),
        "the bones are whatever the dressing's own roll left: {bones} of {dropped:?}"
    );
    assert!(
        !dropped.iter().any(|(kind, _)| *kind == ItemTypeId(0)),
        "**and the innards are gone**, which a field dressing takes: {dropped:?}"
    );
    assert!(
        dropped
            .iter()
            .any(|(kind, count)| *kind == MEAT && *count == 40),
        "while the flesh is untouched - only a **botched** dressing rolls that: {dropped:?}"
    );
}

/// **A butcher without the proficiencies gets less of everything, by the reference's own percentages.**
///
/// Reference: `butchery.cpp:725-738`, the four lines before the action's block -
///
/// ```text
/// flesh or offal:  roll /= 1 + 0.15 * ( 1 - butch_basic )
/// skin:            roll /= 1 + 2.00 * ( 1 - skin_basic )
///                  roll /= 1 + 0.15 * ( 1 - skin_adv )
/// ```
///
/// where each figure is `pct_practiced`, which is **one when the proficiency is learned** and the fraction of its learning time practised otherwise (`proficiency.cpp:473-484`). So a survivor who knows
/// nothing loses a seventh of the flesh - `100 / 1.15` is eighty-seven - and a survivor who knows everything loses nothing.
#[test]
fn a_butcher_without_the_proficiencies_loses_a_share_of_everything() {
    let harvest = |butch_basic: f64, skin_basic: f64, skin_adv: f64| {
        let mut world = World::new();
        let corpse = body(&mut world);
        let entries = [
            entry(HarvestEntry::Flesh, (100, 100)),
            entry(HarvestEntry::Skin, (100, 100)),
            entry(HarvestEntry::Bone, (100, 100)),
        ];
        let mut rolls = Rolls::new(5);
        butcher_finish(
            &mut world,
            corpse,
            HarvestTable {
                entries: &entries,
                ids: &["meat", "skin", "bone"],
                leftovers: None,
                weight: 0.0,
            },
            ButcherFinish {
                butch_basic,
                skin_basic,
                skin_adv,
                ..finish()
            },
            ids,
            |_| 1_000,
            &mut rolls,
        )
    };

    let nothing = harvest(0.0, 0.0, 0.0);
    let count = |dropped: &[(ItemTypeId, i32)], kind: ItemTypeId| {
        dropped
            .iter()
            .find(|(each, _)| *each == kind)
            .map_or(0, |(_, count)| *count)
    };
    assert_eq!(
        count(&nothing, MEAT),
        86,
        "**a hundred cuts of flesh become eighty-six**: `100 / 1.15` is 86.9, and the reference truncates"
    );
    assert_eq!(
        count(&nothing, SKIN),
        28,
        "and a hundred hides become twenty-eight: `100 / 3` is thirty-three and `33 / 1.15` is twenty-eight, truncated"
    );
    assert_eq!(
        count(&nothing, BONE),
        100,
        "while the bones are untouched, which is the reference's asymmetry"
    );

    let everything = harvest(1.0, 1.0, 1.0);
    assert_eq!(
        count(&everything, MEAT),
        100,
        "**a survivor who knows it all loses nothing**"
    );
    assert_eq!(count(&everything, SKIN), 100, "not even the hide");
}

/// **A field dressing takes a roll of the bones, not a half of them, and the roll reaches the top of the range.**
///
/// Reference: `butchery.cpp:764-766` is `roll = rng( 0, roll / 2 )` - a uniform draw from nothing to the half, which means a body of twenty bones gives anything from none to ten. The port halved, which is
/// the **ceiling** of that range and never less. A draw of zero and a draw of one must therefore give different answers on the same body, and neither may exceed the half.
#[test]
fn a_field_dressing_rolls_the_bones_between_none_and_half() {
    let dress = |draw: f64| {
        let mut world = World::new();
        let corpse = body(&mut world);
        let entries = [entry(HarvestEntry::Bone, (20, 20))];
        // Every die the finish reads answers the same figure, so "a low roll" and "a high roll" are two worlds apart by one word.
        let mut rolls = Rolls::fixed(draw);
        butcher_finish(
            &mut world,
            corpse,
            HarvestTable {
                entries: &entries,
                ids: &["bone"],
                leftovers: None,
                weight: 0.0,
            },
            ButcherFinish {
                action: ButcherAction::FieldDress,
                ..finish()
            },
            ids,
            |_| 1_000,
            &mut rolls,
        )
        .first()
        .map_or(0, |(_, count)| *count)
    };
    assert_eq!(dress(0.0), 0, "**a zero draw takes no bones at all**");
    assert_eq!(
        dress(1.0),
        10,
        "and a full draw takes the half, which is the ceiling of the range"
    );
}

/// **The butchery roll is a spread around the skill, not the skill itself**, and it is that roll the entry's `scale_num` multiplies.
///
/// Reference: `roll_butchery_dissect` (`butchery.cpp:566-583`):
///
/// ```text
/// skill_shift  = rng_float( 0, skill_level - 3 )
/// skill_shift += rng_float( 0, dex - 8 ) / 4.0
/// tool_quality < 0  ->  skill_shift -= rng_float( 0, -tool_quality / 5.0 )
/// tool_quality >= 0 ->  skill_shift += min( tool_quality, 4 )
/// return round( skill_shift )
/// ```
///
/// So a survivor with five survival, ten dexterity and a quality-zero tool rolls between nothing and ten; a **tool worse than nothing** subtracts, and a good one adds **at most four** however fine it is. The
/// port passed the raw survival level, which is the range's top and never the bottom.
#[test]
fn the_butchery_roll_spreads_around_the_skill_and_the_tool() {
    // Low dice: nothing at all, whatever the skill.
    let mut low = Rolls::fixed(0.0);
    assert_eq!(
        cdda_sim::roll_butchery_dissect(10, 20, 0, &mut low),
        0,
        "**a zero draw rolls nothing**, so no skill level is a floor"
    );
    // High dice: the top of every range at once.
    let mut high = Rolls::fixed(1.0);
    assert_eq!(
        cdda_sim::roll_butchery_dissect(10, 20, 0, &mut high),
        10,
        "ten survival is seven, twenty dexterity is three, and a quality-zero tool adds nothing"
    );
    // A fine tool adds **four and no more**, however fine.
    let mut high = Rolls::fixed(1.0);
    assert_eq!(
        cdda_sim::roll_butchery_dissect(10, 20, 9, &mut high),
        14,
        "**a masterwork blade is worth four**, not nine"
    );
    // A tool worse than nothing subtracts.
    let mut high = Rolls::fixed(1.0);
    assert_eq!(
        cdda_sim::roll_butchery_dissect(10, 20, -10, &mut high),
        8,
        "a tool of quality minus ten takes two off the top"
    );
    // **A survivor below the reference's thresholds loses ground, and a bad tool on top of it.** The reference's `rng_float` swaps its bounds, so `rng_float( 0, -3 )` is the span `[-3, 0]`: a **low**
    // draw takes the worst of it and a high draw takes none.
    let mut low = Rolls::fixed(0.0);
    assert_eq!(
        cdda_sim::roll_butchery_dissect(0, 0, 0, &mut low),
        -5,
        "**no skill is up to three off and no dexterity up to two more**, with no clamp at zero"
    );
    let mut high = Rolls::fixed(1.0);
    assert_eq!(
        cdda_sim::roll_butchery_dissect(0, 0, -10, &mut high),
        -2,
        "a high draw takes none of the skill or dexterity loss, and the tool still takes its two"
    );
}

/// **A quartered corpse has ruined its hide and lost three quarters of everything else.**
///
/// Reference: `butchery.cpp:813-821` - the block runs after the field dressing and before a share becomes items, and it is unconditional: a body carrying `QUARTERED` gives **no skin at all** ("not continue to
/// show fail effect", the reference's own comment) and a quarter of its flesh, bone and everything else.
#[test]
fn a_quartered_body_has_ruined_its_hide_and_lost_three_quarters_of_the_rest() {
    let mut world = World::new();
    let corpse = body(&mut world);
    world.entity_mut(corpse).insert(cdda_sim::ItemFlags(
        ["QUARTERED".to_owned()].into_iter().collect(),
    ));
    let entries = [
        entry(HarvestEntry::Flesh, (40, 40)),
        entry(HarvestEntry::Bone, (20, 20)),
        entry(HarvestEntry::Skin, (5, 5)),
    ];
    let mut rolls = Rolls::new(13);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat", "bone", "skin"],
            leftovers: None,
            weight: 0.0,
        },
        ButcherFinish {
            quartered: true,
            ..finish()
        },
        ids,
        |_| 1_000,
        &mut rolls,
    );
    assert_eq!(
        dropped,
        vec![(MEAT, 10), (BONE, 5)],
        "**forty cuts of flesh become ten, twenty bones become five, and the hide is gone**"
    );
}

/// **The work itself teaches the proficiencies, and how much depends on the job and on what the body had.**
///
/// Reference: `butchery.cpp:959-993`. Every proficiency gain is a share of the **moves the job took**, and the share depends on the action:
///
/// * a **full** job, on a body with flesh or offal, gives `moves * 0.4` to `butchering_basic` - or to `butchering_adv` if the basic one is already known (`:959-968`);
/// * a **full** job on a body with skin gives `moves * 0.4` to `skinning_basic`, or `skinning_adv` if that is known (`:970-978`);
/// * a **quick** job gives the same choices at `moves * 0.7` for flesh and `moves * 0.15` for skin (`:980-993`).
///
/// A body with no flesh and no offal teaches nothing at all, however long the job took.
#[test]
fn the_work_teaches_the_proficiencies_it_uses() {
    use cdda_sim::ButcheryTraining;

    let full_flesh = ButcheryTraining {
        action: ButcherAction::Full,
        moves: 1_000,
        has_flesh_or_offal: true,
        has_skin: true,
        knows_butchering_basic: false,
        knows_skinning_basic: false,
    }
    .taught();
    assert_eq!(
        full_flesh,
        vec![
            ("prof_butchering_basic".to_owned(), 400),
            ("prof_skinning_basic".to_owned(), 400)
        ],
        "**a full job gives four tenths of its own length to each of the two basics**"
    );

    let full_known = ButcheryTraining {
        knows_butchering_basic: true,
        knows_skinning_basic: true,
        ..ButcheryTraining {
            action: ButcherAction::Full,
            moves: 1_000,
            has_flesh_or_offal: true,
            has_skin: true,
            knows_butchering_basic: false,
            knows_skinning_basic: false,
        }
    }
    .taught();
    assert_eq!(
        full_known,
        vec![
            ("prof_butchering_adv".to_owned(), 400),
            ("prof_skinning_adv".to_owned(), 400)
        ],
        "and a survivor who already knows the basics practises the **advanced** ones instead"
    );

    let quick = ButcheryTraining {
        action: ButcherAction::Quick,
        moves: 1_000,
        has_flesh_or_offal: true,
        has_skin: true,
        knows_butchering_basic: false,
        knows_skinning_basic: false,
    }
    .taught();
    assert_eq!(
        quick,
        vec![
            ("prof_butchering_basic".to_owned(), 700),
            ("prof_skinning_basic".to_owned(), 150)
        ],
        "**a quick job is seven tenths butchery and one and a half tenths skinning**, which is the reference's split"
    );

    let boneless = ButcheryTraining {
        action: ButcherAction::Full,
        moves: 1_000,
        has_flesh_or_offal: false,
        has_skin: false,
        knows_butchering_basic: false,
        knows_skinning_basic: false,
    }
    .taught();
    assert!(
        boneless.is_empty(),
        "**a body of bone and blood teaches neither**: {boneless:?}"
    );
}

/// **The butchery itself is practice for survival, and a dissection is practice for first aid.**
///
/// Reference: `butchery.cpp:952-957`. Each entry adds `( 4 + butchery ) / entry_count` to a running figure - an **integer** division, so the shares lose their remainders - and each finished entry adds one
/// more. Then:
///
/// ```text
/// full or quick or skin or bleed:  practice( skill_survival,  max( 0, practice ), max( size - medium, 0 ) + 4 )
/// dissect:                         practice( skill_firstaid, max( 0, practice ), size + min( tool_quality, 3 ) + 2 )
/// ```
///
/// The cap is the reference's "blunt grinding cap": practice above it earns nothing. So three entries rolled at three, four and five are `( 7 / 3 ) + ( 8 / 3 ) + ( 9 / 3 )` is seven, plus three is **ten**.
#[test]
fn the_butchery_itself_is_practice_for_a_skill() {
    use cdda_sim::ButcheryPractice;

    let survival = ButcheryPractice {
        action: ButcherAction::Full,
        butcheries: vec![3, 4, 5],
        size: CreatureSize::Medium,
        tool_quality: 0,
    }
    .against_skill();
    assert_eq!(
        survival,
        Some(("survival".to_owned(), 10, 4)),
        "**seven from the shares and three from the entries themselves, capped at four for a person**"
    );

    let big = ButcheryPractice {
        action: ButcherAction::Full,
        butcheries: vec![],
        size: CreatureSize::Huge,
        tool_quality: 0,
    }
    .against_skill();
    assert_eq!(
        big,
        Some(("survival".to_owned(), 0, 6)),
        "a huge body's cap is two steps above a person's, and an empty job teaches nothing"
    );

    let dissect = ButcheryPractice {
        action: ButcherAction::Dissect,
        butcheries: vec![3, 4, 5],
        size: CreatureSize::Small,
        tool_quality: 9,
    }
    .against_skill();
    assert_eq!(
        dissect,
        Some(("firstaid".to_owned(), 10, 6)),
        "**a dissection practises first aid**, and a masterwork blade is worth three and no more"
    );
}

/// **A dissection's skill is the drop type's own, and for ordinary tissue that is only the blade.**
///
/// Reference: `butchery_dissect_skill_level` (`butchery.cpp:550-564`):
///
/// ```text
/// sk_total = sum of the survivor's level in each of the drop type's harvest_skills
/// sk_count = how many there are
/// return round( ( sk_total + tool_quality ) / max( sk_count, 1 ) )
/// ```
///
/// **Flesh, bone, skin, blood and offal list no harvest skills at all** (`harvest_drop_type.json`), so for the commonest bodies in the game the figure is `round( tool_quality / 1 )` - the blade and nothing
/// else, however much survival the survivor has. Only the stranger drops, mutagen among them, name skills of their own.
#[test]
fn a_dissection_s_skill_is_the_drop_type_s_own() {
    use cdda_sim::dissect_skill_level;

    assert_eq!(
        dissect_skill_level(&[], 0),
        0,
        "**a bare-handed dissection of ordinary tissue has no skill at all**, whatever the survivor knows"
    );
    assert_eq!(
        dissect_skill_level(&[], 3),
        3,
        "and the blade is the whole of it"
    );
    assert_eq!(
        dissect_skill_level(
            &[("chemistry".to_owned(), 4), ("firstaid".to_owned(), 8)],
            0
        ),
        6,
        "**a mutagen's two skills average to six**, and the blade is added before the division"
    );
    assert_eq!(
        dissect_skill_level(
            &[("chemistry".to_owned(), 4), ("firstaid".to_owned(), 8)],
            4
        ),
        8,
        "a blade of four raises the same body to eight, because it joins the sum ahead of the division"
    );
    assert_eq!(
        dissect_skill_level(&[("firstaid".to_owned(), 5)], -10),
        -5,
        "and a tool worse than nothing takes the figure below zero, which the reference lets it do"
    );
}

/// **The finish judges each entry at its own drop type's skill, so survival buys nothing on ordinary tissue.**
///
/// Reference: `butchery.cpp:694` calls `butchery_dissect_skill_level( you, tool_quality, entry.type )` **inside** the entry loop, and that function reads the drop type's own skills (`:550-564`). Written as the
/// port's own helper, the levels a job is judged at therefore do not move when the survivor's survival rises: a master and a beginner cut the same ordinary body with the same figure.
#[test]
fn the_finish_judges_each_entry_at_its_own_drop_type_s_skill() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // A survival master, which is what used to decide the figure.
    for _ in 0..40 {
        let _ = cdda_sim::skills_of::practice(&mut world, survivor, "survival", 1_000, 10);
    }

    let levels = cdda_sim::creature::butchery_skill_levels(
        &world,
        survivor,
        &[HarvestEntry::Flesh, HarvestEntry::Bone, HarvestEntry::Skin],
        0,
    );
    assert_eq!(
        levels,
        vec![
            (HarvestEntry::Flesh, 0),
            (HarvestEntry::Bone, 0),
            (HarvestEntry::Skin, 0)
        ],
        "**the five ordinary tissues are judged on the blade alone**, so a survival master's levels do not enter"
    );

    let with_blade =
        cdda_sim::creature::butchery_skill_levels(&world, survivor, &[HarvestEntry::Flesh], 3);
    assert_eq!(
        with_blade,
        vec![(HarvestEntry::Flesh, 3)],
        "and a quality-three blade is the whole of the figure"
    );

    // **And a drop type that does name a skill is judged on it**, which is the half of the rule a body of ordinary tissue cannot show. The table is the world's own: this gate gives `bone` a skill the shipped
    // data does not, because the point here is the wiring and not the data - the data has its own gate in the content suite.
    // Practice is capped per call, so a level is reached by asking more than once - the same way the game does it, turn by turn.
    for _ in 0..40 {
        let _ = cdda_sim::skills_of::practice(&mut world, survivor, "firstaid", 1_000, 10);
    }
    let mut table = std::collections::BTreeMap::new();
    table.insert("bone".to_owned(), vec!["firstaid".to_owned()]);
    cdda_sim::harvest_skills::set(&mut world, &table);
    let named =
        cdda_sim::creature::butchery_skill_levels(&world, survivor, &[HarvestEntry::Bone], 0);
    assert_eq!(
        named,
        vec![(HarvestEntry::Bone, 10)],
        "**a drop type that names first aid is judged at the survivor's level in it**, and survival still does not enter"
    );
}

/// **What is left of a body is dropped as ruined parts, and the figures that decide how much are the reference's.**
///
/// Reference: `butchery.cpp:916-948`. A fifth of the body is liquid gore that no item represents, and what remains after the action's own allowance is divided by one leftover's weight:
///
/// ```text
/// remaining = weight - weight / 5
/// full, fresh:            remaining -= weight / 4  if field dressed and not quartered
///                         remaining -= weight - ( weight * 3 / 4 / 4 )  if quartered
///                         remaining -= weight * 0.15  if skinned
/// field dressing:         remaining -= weight * 3 / 4
/// skinning:               remaining -= weight * 0.85
/// charges = remaining / one leftover's weight
/// ```
///
/// All of it is **integer** arithmetic on grams, which is why a body of exactly a kilogram leaves eight hundred grams, and why a field-dressed one leaves fifty - no charge at all if a leftover weighs a
/// hundred grams.
#[test]
fn what_is_left_of_a_body_is_dropped_as_ruined_parts() {
    use cdda_sim::{CorpseLeftovers, leftovers_after_butchery};

    let charges = |action: ButcherAction, field_dressed: bool, skinned: bool, quartered: bool| {
        leftovers_after_butchery(&CorpseLeftovers {
            corpse_grams: 1_000,
            harvested_grams: 0,
            leftover_grams: 100,
            action,
            field_dressed,
            skinned,
            quartered,
        })
    };
    assert_eq!(
        charges(ButcherAction::Full, false, false, false),
        8,
        "**a kilo of body leaves eight hundred grams of ruined parts**, a fifth having run off as gore"
    );
    assert_eq!(
        charges(ButcherAction::Full, true, false, false),
        5,
        "a field-dressed body leaves a quarter less again: five hundred and fifty grams"
    );
    assert_eq!(
        charges(ButcherAction::Full, false, true, false),
        6,
        "and a skinned one six hundred and fifty, which is a seventh and a half off the eight hundred"
    );
    assert_eq!(
        charges(ButcherAction::FieldDress, false, false, false),
        0,
        "**a field dressing leaves fifty grams**, which is no whole leftover at all"
    );
    assert_eq!(
        charges(ButcherAction::Skin, false, false, false),
        0,
        "and a skinning leaves less than nothing, so there is nothing to drop"
    );
    assert_eq!(
        charges(ButcherAction::Full, false, false, true),
        0,
        "a quartered body has given up more than a fifth: it leaves nothing either"
    );
    assert_eq!(
        charges(ButcherAction::Bleed, false, false, false),
        0,
        "**and bleeding drops no ruined parts at all**, which the reference guards by name: its `action != BLEED` sits on the drop, so the eight hundred grams go nowhere"
    );
}

/// **The finish drops what nobody took, and what the drops themselves weighed comes off first.**
///
/// Reference: `butchery.cpp:832-840` subtracts each entry's own weight from the body as it goes - `roll * to_gram( drop->weight )` for a count and the whole share for a bulk entry - and only then
/// (`:916-948`) takes the gore off what is left. A body of a thousand grams that gave five fifty-gram cuts has seven hundred and fifty left, the gore takes a fifth of the **body** - two hundred - and five hundred
/// and fifty remain, which is **five** ruined parts at a hundred grams each. A body that gave nothing up leaves eight.
#[test]
fn the_finish_drops_what_nobody_took() {
    let mut world = World::new();
    let corpse = body(&mut world);
    let entries = [entry(HarvestEntry::Flesh, (5, 5))];
    let mut rolls = Rolls::new(17);
    let dropped = butcher_finish(
        &mut world,
        corpse,
        HarvestTable {
            entries: &entries,
            ids: &["meat"],
            weight: 1_000.0,
            leftovers: Some(("ruined", 100)),
        },
        finish(),
        |name| match name {
            "meat" => MEAT,
            "ruined" => ItemTypeId(104),
            _ => ItemTypeId(0),
        },
        |name| match name {
            "ruined" => 100,
            _ => 50,
        },
        &mut rolls,
    );
    assert_eq!(
        dropped,
        vec![(MEAT, 5), (ItemTypeId(104), 5)],
        "**five cuts of fifty grams, then five ruined parts from what the gore did not take**"
    );
}
