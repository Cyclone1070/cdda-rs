//! How long cutting up a corpse takes.
//!
//! Reference: `butcher_time_to_cut` (`butchery.cpp:382-487`). **The rule was already ported** - `butcher_time_to_cut` in `lib.rs`, beside the activity that spends it - and this file exists because
//! round 1376 wrote a second copy of it in a new module without finding the first. The duplicate is deleted; these are the numbers it should have brought to the rule that was already there.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_sim::{ButcherAction, ButcherState, CreatureSize, butcher_time_to_cut};

/// A butcher with no practice, no helpers, and a corpse with no flags: everything the gates add is set here.
fn state(size: CreatureSize, action: ButcherAction) -> ButcherState {
    ButcherState {
        size,
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

/// The base time is the body's size, and nothing else.
///
/// **The table is in turns and the answer is in moves**, which the multiplication by `100 - factor` bridges: with no blade a cat is `150 * 100`, not `150`. `Bleed` is the action that reads no other
/// rule, so the five sizes show through it unchanged.
#[test]
fn a_bigger_body_takes_longer_to_cut() {
    let bleed = |size: CreatureSize| butcher_time_to_cut(state(size, ButcherAction::Bleed));
    assert_eq!(bleed(CreatureSize::Tiny), 15_000, "a cat is a hundred and fifty turns, which is fifteen thousand moves");
    assert_eq!(bleed(CreatureSize::Small), 30_000, "a dog is three hundred turns");
    assert_eq!(bleed(CreatureSize::Medium), 45_000, "a person is four hundred and fifty turns");
    assert_eq!(bleed(CreatureSize::Large), 60_000, "a cow is six hundred turns");
    assert_eq!(
        bleed(CreatureSize::Huge),
        180_000,
        "something you need a ladder for is eighteen hundred turns"
    );
}

/// The tool's quality takes a percentage off, and **the floor is a quarter of the time rather than none of it**.
///
/// **A transcription lock, not a recording.** The reference scales by `max( 25, 100 - factor )` where the factor is a tool's *quality* — a small number, one to five in the content — so the floor of
/// twenty-five percent cannot be reached by any real knife and no corpse the reference can be handed reaches the two action floors either. The recorded table (`every_recorded_corpse_takes_the_reference_s_time_to_cut`)
/// therefore says nothing about the three clamps, which is why they are pinned here against the source's own arithmetic instead: a mutation of the quartering floor passes every recorded row and fails
/// this test.
///
/// Reference: `butchery.cpp:412-413` - `time_to_cut *= std::max( 25, 100 - factor )`, whose own comment says "at factor 50 it's 50, at factor 75 it's 25". A blade past quality seventy-five buys
/// nothing more, which is why a master's knife does not make a huge corpse free.
#[test]
fn a_better_blade_cuts_faster_until_it_stops_mattering() {
    let medium = |quality: i32| {
        let mut found = state(CreatureSize::Medium, ButcherAction::Bleed);
        found.tool_factor = quality;
        butcher_time_to_cut(found)
    };
    assert_eq!(medium(0), 45_000, "no blade gives the whole hundred times the table");
    assert_eq!(medium(1), 44_550, "**one point of quality is one percent off the hundred**: 450 * 99");
    assert_eq!(medium(50), 22_500, "half the time at quality fifty");
    assert_eq!(medium(75), 11_250, "**the floor: a quarter of the time at quality seventy-five**");
    assert_eq!(medium(90), 11_250, "and no quality past seventy-five buys anything");
}

/// The action decides the multiplier, two actions have **floors** of their own, and the proficiency tail multiplies what is left.
///
/// Reference: `butchery.cpp:421-487`. **The tail is why an untrained butcher is not at the base time**, and **each penalty is a truncation**: the reference holds `prof_skin_penalty` in an `int`, so the
/// half-point an untrained skinner costs is **zero** rather than a half. That makes a quick cut `0.7 * 3 + 0.15 * 1 + 0.15` and a full butchery `0.4 * 3 + 0.4 * 1 + 0.2`.
#[test]
fn the_action_decides_the_multiplier_and_the_floors_hold() {
    let medium = |action: ButcherAction| butcher_time_to_cut(state(CreatureSize::Medium, action));
    assert_eq!(medium(ButcherAction::Bleed), 45_000, "a bleed reads no other rule at all");
    assert_eq!(medium(ButcherAction::Quarter), 11_250, "a quartering is a quarter of the base when that is over the floor");
    assert_eq!(medium(ButcherAction::Dismember), 4_500, "and a dismembering is a tenth");
    assert_eq!(medium(ButcherAction::Dissect), 270_000, "a dissection is six times the base");
    assert_eq!(medium(ButcherAction::Quick), 107_999, "**a quick cut is 45,000 * 2.4, truncated from 108,000 by the float**");
    assert_eq!(medium(ButcherAction::FieldDress), 270_000, "a field dressing is twice the base, then times three");
    assert_eq!(
        medium(ButcherAction::Skin),
        90_000,
        "**a skinning is twice the base and then times one**, because the half-point skin penalty truncates to zero"
    );
    assert_eq!(medium(ButcherAction::Full), 486_000, "**a full butchery of an undressed body is six times the base, then times 1.8**");

    // The two floors: a body small enough that a tenth of its time is under 600 moves waits 600 anyway.
    let tiny = |action: ButcherAction, quality: i32| {
        let mut found = state(CreatureSize::Tiny, action);
        found.tool_factor = quality;
        butcher_time_to_cut(found)
    };
    assert_eq!(tiny(ButcherAction::Dismember, 0), 1_500, "a cat's tenth is fifteen hundred, over the floor");
    assert_eq!(tiny(ButcherAction::Dismember, 75), 600, "**but a cat with a good blade is under the floor, so the floor answers**");
    assert_eq!(tiny(ButcherAction::Quarter, 75), 1_200, "and a quartered cat would be under four hundred and fifty, so twelve hundred answers");
}

/// A field-dressed corpse is quicker to finish, and a quartered one quicker again.
///
/// Reference: `butchery.cpp:431-436` for the dressing, `:468-470` for the quartering - which is divided out **last**, after every multiplier and before the proficiencies.
#[test]
fn dressing_and_quartering_both_shorten_the_work() {
    let mut dressed = state(CreatureSize::Medium, ButcherAction::Full);
    dressed.field_dressed = true;
    assert_eq!(butcher_time_to_cut(dressed), 324_000, "**four times the base, then times 1.8**: 450 * 100 * 4 * 1.8");

    let mut failed = state(CreatureSize::Medium, ButcherAction::Full);
    failed.field_dressed = true;
    failed.field_dress_failed = true;
    assert_eq!(
        butcher_time_to_cut(failed),
        486_000,
        "**a failed dressing is as slow as none at all**, which is the reference's own `||`"
    );

    let whole = butcher_time_to_cut(state(CreatureSize::Medium, ButcherAction::Dissect));
    let mut quartered = state(CreatureSize::Medium, ButcherAction::Dissect);
    quartered.quartered = true;
    assert_eq!(whole, 270_000, "a dissection of a whole body is six times the base");
    assert_eq!(
        butcher_time_to_cut(quartered),
        67_500,
        "**and a quartered one is a quarter of that**"
    );
}

/// Helpers shorten the work by a tenth each, and a butcher who bleeds instantly takes one move.
///
/// Reference: `butchery.cpp:426-430` and the last line before the return, `time_to_cut *= ( 1.0f - helpers / 10.0f )`. **The fraction is applied as a fraction**: truncating it first would delete the
/// work rather than shorten it.
#[test]
fn helpers_shorten_the_work_and_an_instant_bleed_takes_a_move() {
    let mut helped = state(CreatureSize::Medium, ButcherAction::Bleed);
    helped.helpers = 3;
    assert_eq!(
        butcher_time_to_cut(helped),
        31_499,
        "**three helpers take three tenths off**: 45,000 * 0.7, truncated a move short by the float - the reference truncates the same way"
    );

    let mut bleeding = state(CreatureSize::Medium, ButcherAction::Bleed);
    bleeding.instant_bleed = true;
    assert_eq!(butcher_time_to_cut(bleeding), 1, "a butcher who bleeds instantly needs a single move");
}

/// **Every recorded corpse takes the time the reference takes to cut it up**, over sizes, actions, corpse flags and quality factors.
///
/// The reference's `butcher_time_to_cut` (`butchery.cpp:382-487`) was transcribed into the port long before there was a recording of it, which is exactly the kind of rule that drifts: the order of the
/// multiplications decides the number. `corpus/prototypes/butchery.json` is `--dump-butchery`: five bodies, one per size class, in four flag states, for all eight actions.
#[test]
fn every_recorded_corpse_takes_the_reference_s_time_to_cut() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/butchery.json");
    let text = std::fs::read_to_string(path).expect("the butchery recording");
    let recorded = cdda_json::parse(text.as_bytes()).expect("the recording parses");
    let cdda_json::Value::Object(top) = &recorded else {
        panic!("the recording is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = top.get("rows") else {
        panic!("the recording has rows");
    };
    let mut seen = 0;
    for row in rows {
        let cdda_json::Value::Object(object) = row else {
            panic!("a row is an object");
        };
        let word = |member: &str| match object.get(member) {
            Some(cdda_json::Value::Str(text)) => text.as_str(),
            _ => panic!("a row states {member}"),
        };
        let size = match word("size") {
            "tiny" => CreatureSize::Tiny,
            "small" => CreatureSize::Small,
            "medium" => CreatureSize::Medium,
            "large" => CreatureSize::Large,
            "huge" => CreatureSize::Huge,
            other => panic!("a size the port does not know: {other}"),
        };
        let action = match word("action") {
            "BLEED" => ButcherAction::Bleed,
            "QUICK" => ButcherAction::Quick,
            "FULL" => ButcherAction::Full,
            "FIELD_DRESS" => ButcherAction::FieldDress,
            "SKIN" => ButcherAction::Skin,
            "QUARTER" => ButcherAction::Quarter,
            "DISMEMBER" => ButcherAction::Dismember,
            "DISSECT" => ButcherAction::Dissect,
            other => panic!("an action the port does not know: {other}"),
        };
        let mut corpse = state(size, action);
        match word("state") {
            "plain" => {}
            "field_dressed" => corpse.field_dressed = true,
            "field_dress_failed" => {
                corpse.field_dressed = true;
                corpse.field_dress_failed = true;
            }
            "quartered" => corpse.quartered = true,
            other => panic!("a state the port does not know: {other}"),
        }
        let mine = butcher_time_to_cut(corpse);
        let Some(cdda_json::Value::Int(reference)) = object.get("moves") else {
            panic!("a row states its moves");
        };
        #[allow(clippy::cast_possible_truncation)]
        let reference = *reference as i32;
        assert_eq!(
            mine,
            reference,
            "{} / {} / {}: port {mine}, reference {reference}",
            word("monster"),
            word("state"),
            word("action"),
        );
        seen += 1;
    }
    assert_eq!(seen, 160, "**every recorded row is compared**, not a sample");
}

/// **A finished butchery puts the harvest on the ground and takes the body away.** The activity is what spends the turns; this is what the last one leaves behind.
///
/// Reference: `butchery.cpp`'s finish - each entry of the monster's harvest definition is rolled, damaged, and turned into items on the corpse's square, and the corpse is consumed. The port's
/// `butcher_finish` is that loop; this gate is the **activity's own finisher** calling it, which is the piece the sim was missing: the clock, the turn and the yields were all here, and nothing joined
/// them to a creature doing the work.
#[test]
fn a_finished_butchery_leaves_the_harvest_on_the_ground() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(7));
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "corpse",
                volume_ml: 1_000,
                weight_g: 1_000,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
            cdda_sim::item_registry::Kind {
                id: "meat",
                volume_ml: 250,
                weight_g: 200,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 0.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            },
        ],
    );
    let corpse_kind = cdda_sim::item_registry::id_of(&mut world, "corpse").expect("the corpse type");
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_butcherable", hp: 10 }],
    );
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_butcherable").expect("the kind");
    // One flesh entry: eight to twelve pieces, no scaling, no mass share, no ceiling.
    cdda_sim::monster_registry::set_harvests(
        &mut world,
        &[(
            kind,
            vec![cdda_sim::HarvestSpec {
                part: Some(cdda_sim::HarvestEntry::Flesh),
                base_num: (8, 12),
                scale_num: (0.0, 0.0),
                mass_ratio: None,
                max: None,
            }],
            vec!["meat".to_owned()],
        )],
    );

    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let corpse = cdda_sim::drop_item(&mut world, (0, 0), corpse_kind, 1);
    world.entity_mut(corpse).insert(cdda_sim::CorpseOf {
        kind,
        died_on: 0,
        revives: false,
        monster_volume_ml: 1_000,
    });
    world.entity_mut(survivor).insert(cdda_sim::creature::Doing {
        activity: "butcher".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Time,
        moves_total: 0,
        subject: Some(corpse),
    });

    cdda_sim::creature::finish_activity(&mut world, survivor, "butcher", Some(corpse));

    assert!(
        world.get_entity(corpse).is_err(),
        "**the body is used up**, which is what the reference's finish does at the end of the loop"
    );
    let meat = cdda_sim::item_registry::id_of(&mut world, "meat").expect("the meat type");
    let dropped: i32 = world
        .query::<(&cdda_sim::GroundItem, &cdda_sim::Position)>()
        .iter(&world)
        .filter(|(item, _)| item.kind == meat)
        .map(|(item, _)| item.charges)
        .sum();
    assert!(
        (8..=12).contains(&dropped),
        "**eight to twelve cuts of meat are on the ground**, the entry's own range: {dropped}"
    );
}
