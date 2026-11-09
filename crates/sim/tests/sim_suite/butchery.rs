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
    // A survivor who **knows the job**, so the gate is about the harvest landing on the ground and not about how much of it a beginner loses; that has its own gate.
    for proficiency in ["prof_butchering_basic", "prof_skinning_basic", "prof_skinning_adv"] {
        cdda_sim::proficiencies::learn(&mut world, survivor, proficiency);
    }
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

/// **A pulped corpse gives half the flesh a fresh one does.** The damage the body took before it died is what the finish reads, and the port had it reading nothing.
///
/// Reference: `corpse_damage_effect` (`butchery.cpp:492-561`) halves a pulped body's flesh, three-quarters a mangled one, and nine-tenths a damaged one; the level is the **corpse item's own**
/// `damage_level()`, which `itype::damage_level` computes from `damage_ / max_damage`.
///
/// **The entry is a share of the body's weight and not a count**, because a count entry's floor is applied *after* the damage: `roll = max( corpse_damage_effect( roll, ... ), base_num.first )`
/// (`butchery.cpp:705-708`), so a body whose entry promises eight cuts gives eight however mangled it is. A bulk entry has no such floor, which is why the difference is visible here and nowhere else.
#[test]
fn a_pulped_corpse_gives_half_the_flesh_of_a_fresh_one() {
    let butchered = |damage: i32| {
        let mut world = bevy_ecs::prelude::World::new();
        cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(11));
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
                // A gram a cut, so the item count reads the share of the body directly and the gate is about the damage and nothing else.
                cdda_sim::item_registry::Kind {
                    id: "cut",
                    volume_ml: 1,
                    weight_g: 1,
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
            &[cdda_sim::monster_registry::MonsterKind {
                id: "mon_beaten",
                hp: 10,
            }],
        );
        let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_beaten").expect("the kind");
        // A quarter of the body's weight in flesh, which is the reference's bulk path: no floor, so the damage shows.
        cdda_sim::monster_registry::set_weights(&mut world, &[(kind, 1_000)]);
        cdda_sim::monster_registry::set_harvests(
            &mut world,
            &[(
                kind,
                vec![cdda_sim::HarvestSpec {
                    part: Some(cdda_sim::HarvestEntry::Flesh),
                    base_num: (0, 0),
                    scale_num: (0.0, 0.0),
                    mass_ratio: Some(0.25),
                    max: None,
                }],
                vec!["cut".to_owned()],
            )],
        );
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let corpse = cdda_sim::drop_item(&mut world, (0, 0), corpse_kind, 1);
        if let Some(mut item) = world.get_mut::<cdda_sim::GroundItem>(corpse) {
            item.damage = damage;
        }
        world.entity_mut(corpse).insert(cdda_sim::CorpseOf {
            kind,
            died_on: 0,
            revives: false,
            monster_volume_ml: 1_000,
        });
        // The multiplier is held at one and the proficiencies are known, so the gate is about the **damage** and not about the survivor's hands; both have their own gates.
        for proficiency in ["prof_butchering_basic", "prof_skinning_basic", "prof_skinning_adv"] {
            cdda_sim::proficiencies::learn(&mut world, survivor, proficiency);
        }
        cdda_sim::creature::finish_butchering_with(&mut world, survivor, corpse, 1.0);
        let cut = cdda_sim::item_registry::id_of(&mut world, "cut").expect("the cut type");
        // **Only the cuts**, because the body also leaves ruined chunks now, and they are not what this gate is about.
        world
            .query::<&cdda_sim::GroundItem>()
            .iter(&world)
            .filter(|item| item.kind == cut)
            .map(|item| item.charges)
            .sum::<i32>()
    };

    let fresh = butchered(0);
    // Level four is "pulped", and `itype::damage_level` reaches it at three quarters of the maximum: `1 + 4 * 3000 / 4000` is four.
    let pulped = butchered(3 * cdda_content::armour::DAMAGE_MAX / 4);
    assert_eq!(fresh, 250, "a quarter of a thousand grams, undamaged");
    assert_eq!(
        pulped, 125,
        "**a pulped one gives half**, which is `corpse_damage_effect`'s own figure for flesh"
    );
    // **And a body at exactly its maximum damage is `pulped` no longer.** `itype::damage_level` clamps to five, `corpse_damage_effect`'s switch names two, three and four, and its `default: break`
    // therefore returns the weight untouched - so the worst-damaged corpse in the game gives a full harvest. That is the reference's own arithmetic rather than a port's slip, and a port that
    // "fixed" it would disagree with the game on every corpse beaten to its last point of damage.
    assert_eq!(
        butchered(cdda_content::armour::DAMAGE_MAX),
        250,
        "a corpse at the maximum damage level falls to the switch's default and gives everything"
    );
}

/// **A butcher's skill, dexterity and blade decide how much of a body's bulk becomes items**, and the arithmetic is the reference's own three-way average.
///
/// Reference: `butchery_dissect_yield_mult` (`butchery.cpp:585-593`) is `0.5 * clamp(skill/10) + 0.3 * clamp((quality + 50)/100) + 0.2 * clamp(dex/20)`, each term clamped to **one** - so a survivor with
/// ten survival, a quality-fifty blade and twenty dexterity is the maximum, and anything above those numbers adds nothing. The three weights are half skill, three tenths tool, two tenths dexterity.
#[test]
fn a_butcher_s_skill_and_blade_decide_how_much_bulk_comes_off() {
    // Nothing at all: no skill, a blade fifty worse than the best (-50 makes the tool term zero), and no dexterity.
    assert_eq!(
        cdda_sim::butchery_yield_multiplier(0, 0, -50),
        0.0,
        "a hopeless butcher with a useless blade gets nothing extra"
    );
    // Skill alone, at its ceiling: half of one.
    assert_eq!(cdda_sim::butchery_yield_multiplier(10, 0, -50), 0.5, "ten survival is half the multiplier");
    // The tool alone at its ceiling: three tenths.
    assert_eq!(cdda_sim::butchery_yield_multiplier(0, 0, 50), 0.3, "a fine blade is three tenths");
    // Dexterity alone at its ceiling: two tenths.
    assert_eq!(cdda_sim::butchery_yield_multiplier(0, 20, -50), 0.2, "twenty dexterity is two tenths");
    // Everything at its ceiling: exactly one, and no more however far past it the numbers go.
    assert_eq!(
        cdda_sim::butchery_yield_multiplier(10, 20, 50),
        1.0,
        "**the reference clamps each term**, so a master with a perfect blade and twenty dexterity is one, not one and a half"
    );
    assert_eq!(
        cdda_sim::butchery_yield_multiplier(40, 80, 400),
        1.0,
        "and the clamps hold far past the ceiling"
    );
    // A middling survivor: five survival, a quality-zero blade, ten dexterity.
    let middling = cdda_sim::butchery_yield_multiplier(5, 10, 0);
    assert!(
        (middling - (0.25 + 0.15 + 0.1)).abs() < 1e-12,
        "five survival is a quarter, a middling blade three twentieths past its half, ten dexterity a tenth: {middling}"
    );
}

/// **A share of a body's weight becomes items only after it is divided by one item's weight**, and the division rounds **up**.
///
/// Reference: `butchery.cpp:829-840`: a `mass_ratio` entry's roll is a weight in grams, it is multiplied by the yield multiplier, and then `roll = ceil( roll / to_gram( drop->weight ) )` - so a
/// kilogram's share of meat at a hundred grams a cut is **ten** cuts, and one gram over a kilogram is eleven. The port used to report the weight itself as the count, which is right only for an item
/// that happens to weigh a gram.
#[test]
fn a_weight_share_becomes_as_many_items_as_fit_in_it() {
    // A thousand grams of a body, a quarter of it flesh: 250 grams' worth.
    let cuts = |drop_grams: i64, multiplier: f64| {
        let mut world = bevy_ecs::prelude::World::new();
        cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(3));
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
                    id: "cut",
                    volume_ml: 100,
                    weight_g: drop_grams,
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
            &[cdda_sim::monster_registry::MonsterKind { id: "mon_bulk", hp: 10 }],
        );
        let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_bulk").expect("the kind");
        cdda_sim::monster_registry::set_weights(&mut world, &[(kind, 1_000)]);
        cdda_sim::monster_registry::set_harvests(
            &mut world,
            &[(
                kind,
                vec![cdda_sim::HarvestSpec {
                    part: Some(cdda_sim::HarvestEntry::Flesh),
                    base_num: (0, 0),
                    scale_num: (0.0, 0.0),
                    mass_ratio: Some(0.25),
                    max: None,
                }],
                vec!["cut".to_owned()],
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
        cdda_sim::creature::finish_butchering_with(&mut world, survivor, corpse, multiplier);
        let cut = cdda_sim::item_registry::id_of(&mut world, "cut").expect("the cut type");
        world
            .query::<&cdda_sim::GroundItem>()
            .iter(&world)
            .filter(|item| item.kind == cut)
            .map(|item| item.charges)
            .sum::<i32>()
    };

    assert_eq!(
        cuts(100, 1.0),
        3,
        "**two hundred and fifty grams at a hundred grams a cut is two and a half, which rounds up to three**"
    );
    assert_eq!(cuts(250, 1.0), 1, "at a quarter of a kilo a cut it is exactly one");
    assert_eq!(cuts(100, 0.5), 2, "half the multiplier halves the weight before the division: 125 grams is two cuts");
    assert_eq!(
        cuts(1_000, 1.0),
        1,
        "and a cut heavier than the share is still one cut, because nothing is left over to make zero"
    );
}
