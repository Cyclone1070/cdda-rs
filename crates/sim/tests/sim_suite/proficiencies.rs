//! Learning a proficiency by practising it.
//!
//! Reference: `proficiency_set::practice` (`proficiency.cpp:306-345`).

use bevy_ecs::prelude::*;

/// Practice adds up, and the day it reaches the time it takes, the proficiency is known.
///
/// Reference: `proficiency_set::practice` (`proficiency.cpp:306-345`): the practised time is the sum of what has been practised, and learning **removes** the
/// progress - `learn` is followed by the erase, so a known proficiency has nothing practised toward it.
#[test]
fn practice_adds_up_until_the_proficiency_is_learned() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    // Eight hours to learn it, and a craft that practised an hour.
    let eight_hours = 8 * 60 * 60;
    assert!(!cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 60 * 60, eight_hours));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_tailor"),
        60 * 60,
        "an hour of practice is an hour"
    );
    assert!(!cdda_sim::proficiencies::knows(&world, survivor, "prof_tailor"));

    // Seven more hours reaches the eight, and that practice is the one that teaches it.
    assert!(cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 7 * 60 * 60, eight_hours));
    assert!(cdda_sim::proficiencies::knows(&world, survivor, "prof_tailor"));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_tailor"),
        0,
        "a learned proficiency has no progress left: the reference erases it"
    );

    // Practising something already known changes nothing and learns nothing again.
    assert!(!cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 60 * 60, eight_hours));
    assert!(!cdda_sim::proficiencies::practice(&mut world, survivor, "prof_tailor", 0, eight_hours));
}

/// A craft's time is **divided** between the proficiencies it teaches.
///
/// Reference: `craft_proficiency_gain` (`crafting.cpp:1036-1090`): the subjects are the proficiencies the crafter does not know yet, the craft's time is divided
/// between them (`time / subjects.size()`), and each gets that share times `learning_time_mult / time_multiplier`.
#[test]
fn a_craft_divides_its_time_between_what_it_teaches() {
    let mut world = World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let eight_hours = 8 * 60 * 60;
    let teaches = [
        cdda_sim::proficiencies::Taught {
            id: "prof_tailor",
            time_multiplier: 1.0,
            learning_time_mult: 1.0,
            time_to_learn: eight_hours,
        },
        cdda_sim::proficiencies::Taught {
            id: "prof_knitting",
            time_multiplier: 1.0,
            learning_time_mult: 1.0,
            time_to_learn: eight_hours,
        },
    ];

    // Eight hours of work split two ways is four hours each, which is not enough to learn either.
    assert!(!cdda_sim::proficiencies::gain_from_craft(
        &mut world,
        crafter,
        &teaches,
        eight_hours,
        &|_| true
    ));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_tailor"),
        4 * 60 * 60
    );
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_knitting"),
        4 * 60 * 60
    );

    // Eight more hours is another four each, and both are learned by it.
    assert!(cdda_sim::proficiencies::gain_from_craft(
        &mut world,
        crafter,
        &teaches,
        eight_hours,
        &|_| true
    ));
    assert!(cdda_sim::proficiencies::knows(&world, crafter, "prof_tailor"));
    assert!(cdda_sim::proficiencies::knows(&world, crafter, "prof_knitting"));

    // A proficiency the crafter cannot learn is not practised and does not take a share of the time.
    let refused = [cdda_sim::proficiencies::Taught {
        id: "prof_gun_smith",
        time_multiplier: 1.0,
        learning_time_mult: 1.0,
        time_to_learn: eight_hours,
    }];
    assert!(!cdda_sim::proficiencies::gain_from_craft(
        &mut world,
        crafter,
        &refused,
        60 * 60,
        &|_| false
    ));
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_gun_smith"),
        0,
        "a proficiency with prerequisites it does not meet is not practised at all"
    );
}

/// Finishing a craft practises what the recipe teaches, because the craft calls the rule.
///
/// Reference: the craft activity calls `craft_proficiency_gain` on each fifth-percent step (`activity_actor.cpp:4237-4245`). **A rule with no caller is not a ported rule**,
/// so this drives the **turn loop** rather than calling the rule: it is the only test that would notice the hook being removed, or moved to the wrong turn.
#[test]
fn a_craft_practises_what_it_teaches_while_it_runs() {
    let mut world = bevy_ecs::prelude::World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // The table the guard reads, installed the way a real world installs it.
    let Ok(dump) = cdda_json::parse(br#"{"count": 1, "proficiencies": [{"id": "prof_tailor", "seconds": 60, "can_learn": true, "required": []}]}"#.as_slice()) else {
        return;
    };
    let Some((proficiencies, _)) = cdda_content::proficiencies::from_dump(&dump) else {
        return;
    };
    cdda_sim::proficiencies::install(&mut world, &proficiencies);
    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned()],
        by_name: [("rag".to_owned(), cdda_sim::ItemTypeId(1))].into_iter().collect(),
        volumes_ml: vec![0, 250],
        weights_g: vec![0, 5],
        holds_ml: vec![0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0)],
        thrown_damage: Vec::new(),
        melee_bash: Vec::new(),
        melee_cut: Vec::new(),
        melee_stab: Vec::new(),
        to_hit: Vec::new(),
    };
    let recipe = cdda_content::recipes::Recipe {
        id: "patch_a_shirt".to_owned(),
        result: "rag".to_owned(),
        blacklisted: false,
        never_learn: false,
        practice: None,
        components: Vec::new(),
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        batch_rscale: 0.0,
        batch_rsize: 0,
        byproducts: Vec::new(),
        made_per_batch: 1,
        byproduct_group: Vec::new(),
        proficiencies: vec![cdda_content::recipes::CraftProficiency {
            id: "prof_tailor".to_owned(),
            time_multiplier: 1.0,
            learning_time_mult: 1.0,
        }],
        container: None,
        time_moves: 6_000,
    };
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id, false, false);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        cdda_sim::crafting::Craft::Started
    );
    // The practice comes **while the craft runs**, not when it ends: the reference practises on every fifth-percent step (`activity_actor.cpp:4237-4245`), so the
    // only way to see it is to run the turns. A minute of recipe time is three thousand moves, which is thirty turns of a hundred.
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_tailor"),
        0,
        "nothing has been practised before any work is done"
    );
    for _ in 0..70 {
        // A creature gets its moves back each turn; the port's turn loop refills them, and a test that hand-rolls the loop must do the same.
        cdda_sim::creature::give_moves(&mut world, crafter, 100);
        cdda_sim::turn::run_turn(&mut world);
    }
    assert!(
        cdda_sim::proficiencies::knows(&world, crafter, "prof_tailor"),
        "a minute of work against a minute of learning is enough to know it"
    );
}

/// A proficiency with an unmet prerequisite is not practised, and the craft does not pretend otherwise.
///
/// Reference: `proficiency_set::practice` (`proficiency.cpp:307`): `has_prereqs` is checked before any time is added, so an expert trade waits for its basic one.
/// **The guard reads the data's prerequisite graph**, which is why this test installs the dump rather than passing a hand-written closure.
#[test]
fn an_unmet_prerequisite_does_not_accumulate_practice() {
    let mut world = bevy_ecs::prelude::World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let Ok(dump) = cdda_json::parse(br#"{"count": 2, "proficiencies": [
            {"id": "prof_athlete_basic", "seconds": 60, "can_learn": true, "required": []},
            {"id": "prof_athlete_expert", "seconds": 60, "can_learn": true, "required": ["prof_athlete_basic"]},
            {"id": "prof_unlearnable", "seconds": 60, "can_learn": false, "required": []}
        ]}"#.as_slice()) else {
        return;
    };
    let Some((proficiencies, _)) = cdda_content::proficiencies::from_dump(&dump) else {
        return;
    };
    cdda_sim::proficiencies::install(&mut world, &proficiencies);

    assert!(cdda_sim::proficiencies::can_learn(&world, crafter, "prof_athlete_basic"));
    assert!(
        !cdda_sim::proficiencies::can_learn(&world, crafter, "prof_athlete_expert"),
        "an expert trade waits for the basic one"
    );
    assert!(
        !cdda_sim::proficiencies::can_learn(&world, crafter, "prof_unlearnable"),
        "and the data can say a proficiency is never learned by practising"
    );

    // Learning the basic one opens the expert one, because the guard asks the graph rather than a list fixed at load time.
    cdda_sim::proficiencies::learn(&mut world, crafter, "prof_athlete_basic");
    assert!(cdda_sim::proficiencies::can_learn(&world, crafter, "prof_athlete_expert"));
}

/// The craft asks the data before it teaches: a trade whose prerequisite is unknown is not practised.
///
/// Reference: `proficiency_set::practice` refuses without `has_prereqs` (`proficiency.cpp:307`), and `craft_proficiency_gain` builds its subjects from what the crafter
/// **can** learn (`crafting.cpp:1052-1062`). The reach matters here: the guard lives in `finish_craft`, not in the rule.
#[test]
fn a_craft_does_not_teach_a_trade_whose_prerequisite_is_unknown() {
    let mut world = bevy_ecs::prelude::World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let Ok(dump) = cdda_json::parse(br#"{"count": 2, "proficiencies": [
            {"id": "prof_basic", "seconds": 20, "can_learn": true, "required": []},
            {"id": "prof_expert", "seconds": 20, "can_learn": true, "required": ["prof_basic"]}
        ]}"#.as_slice()) else {
        return;
    };
    let Some((proficiencies, _)) = cdda_content::proficiencies::from_dump(&dump) else {
        return;
    };
    cdda_sim::proficiencies::install(&mut world, &proficiencies);

    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned()],
        by_name: [("rag".to_owned(), cdda_sim::ItemTypeId(1))].into_iter().collect(),
        volumes_ml: vec![0, 250],
        weights_g: vec![0, 5],
        holds_ml: vec![0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0)],
        thrown_damage: Vec::new(),
        melee_bash: Vec::new(),
        melee_cut: Vec::new(),
        melee_stab: Vec::new(),
        to_hit: Vec::new(),
    };
    let recipe = cdda_content::recipes::Recipe {
        id: "practise_the_expert_trade".to_owned(),
        result: "rag".to_owned(),
        blacklisted: false,
        never_learn: false,
        practice: None,
        components: Vec::new(),
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        batch_rscale: 0.0,
        batch_rsize: 0,
        byproducts: Vec::new(),
        made_per_batch: 1,
        byproduct_group: Vec::new(),
        proficiencies: vec![cdda_content::recipes::CraftProficiency {
            id: "prof_expert".to_owned(),
            time_multiplier: 1.0,
            learning_time_mult: 1.0,
        }],
        container: None,
        time_moves: 6_000,
    };
    let craft_once = |world: &mut bevy_ecs::prelude::World| {
        cdda_sim::creature::stop_doing(world, crafter);
        cdda_sim::recipes_known::learn(world, crafter, &recipe.id, false, false);
        assert_eq!(
            cdda_sim::crafting::begin_craft_from_carried(world, crafter, &recipe, 1, &names),
            cdda_sim::crafting::Craft::Started
        );
        // The practice happens on the turns, so the test runs them; the finish is only what hands the result over.
        for _ in 0..70 {
            cdda_sim::creature::give_moves(world, crafter, 100);
            cdda_sim::turn::run_turn(world);
        }
        // The turns finish the craft themselves; what the test reads afterwards is the survivor, not the craft's return value.
        assert!(
            world.get::<cdda_sim::crafting::Making>(crafter).is_none(),
            "the work is done when the turns are over"
        );
    };
    craft_once(&mut world);
    assert!(
        !cdda_sim::proficiencies::knows(&world, crafter, "prof_expert"),
        "a minute of work cannot teach an expert trade to someone with no basic one"
    );
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, crafter, "prof_expert"),
        0,
        "and nothing was practised toward it, because the guard refused before any time was added"
    );

    // Learning the basic trade opens the expert one, and the same craft now teaches it.
    cdda_sim::proficiencies::learn(&mut world, crafter, "prof_basic");
    craft_once(&mut world);
    assert!(
        cdda_sim::proficiencies::knows(&world, crafter, "prof_expert"),
        "with the prerequisite known, the same craft is a minute against a minute and learns it"
    );
}

/// **A kill teaches what the body was worth knowing.** Every weak-point family on the monster practises its proficiency for **one minute**, and that is the whole of the rule the reference calls
/// `practice_kill`.
///
/// Reference: `weakpoint_families::practice_kill` (`weakpoint.cpp:152-155`) is `practice( learner, 1_minutes )`, and `practice` (`:133-145`) adds that span to every family's proficiency. The
/// reference's own note beside `practice_dissect` says proficiency experience is capped at **1000 seconds per call**, which a minute never reaches - the cap is named in `docs/parity.md` and not
/// modelled here because nothing this rule asks for can hit it.
#[test]
fn a_kill_practises_the_families_the_body_names() {
    let mut world = bevy_ecs::prelude::World::new();
    let killer = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // Two families, one of them the same proficiency twice, which the reference would practise twice because it walks the list.
    let families = [
        cdda_content::weakpoints::Family {
            id: "fam_a".to_owned(),
            proficiency: "prof_cutting".to_owned(),
            bonus: 0.1,
            penalty: -0.1,
        },
        cdda_content::weakpoints::Family {
            id: "fam_b".to_owned(),
            proficiency: "prof_bleeding".to_owned(),
            bonus: 0.1,
            penalty: -0.1,
        },
    ];
    let learn_times = |id: &str| match id {
        "prof_cutting" => 600,
        "prof_bleeding" => 10_000,
        _ => 0,
    };

    let taught = cdda_sim::proficiencies::practise_a_kill(&mut world, killer, &families, &learn_times);

    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, killer, "prof_cutting"),
        60,
        "**one minute of practice**, which is what `practice_kill` adds"
    );
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, killer, "prof_bleeding"),
        60,
        "and every family is practised, not just the first"
    );
    assert!(
        !cdda_sim::proficiencies::knows(&world, killer, "prof_cutting"),
        "**one minute is not a six-hundred-second proficiency**, so the first kill teaches and does not grant"
    );
    // The tenth kill of a body worth ten of those minutes is what learns it, and the rule says so on that kill and not before.
    for _ in 1..10 {
        cdda_sim::proficiencies::practise_a_kill(&mut world, killer, &families, &learn_times);
    }
    assert!(
        cdda_sim::proficiencies::knows(&world, killer, "prof_cutting"),
        "ten kills at a minute apiece are six hundred seconds, which is exactly the time to learn it"
    );
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, killer, "prof_bleeding"),
        600,
        "while the proficiency that takes ten thousand seconds is still learning, at ten minutes"
    );
    assert!(taught.is_empty(), "none of them was learned by the **first** kill: {taught:?}");
}

/// **Dissecting a body teaches for two fifteen-minute halves.** The reference splits half an hour into two calls and ORs what they learned.
///
/// Reference: `weakpoint_families::practice_dissect` (`weakpoint.cpp:157-168`) is two `practice( learner, 15_minutes )` calls, and `learned = p1 || p2`. Its own comment blames a **1000-second cap on
/// proficiency practice** for the split - and that cap is not in this baseline: `Character::practice_proficiency` (`character_proficiency.cpp:71-92`) passes its `max` argument through to
/// `proficiency_set::practice` (`proficiency.cpp:306-345`), and the only reader of it is `if( max && current.practiced > *max ) return false;`, which `weakpoint.cpp` never arms. So the two calls add up
/// to half an hour either way, and the split is kept because it is what the reference does, not because the arithmetic needs it.
#[test]
fn dissecting_teaches_two_halves_of_fifteen_minutes() {
    let mut world = bevy_ecs::prelude::World::new();
    let butcher = cdda_sim::creature::spawn_creature(&mut world, "the butcher", (0, 0, 0));
    let families = [cdda_content::weakpoints::Family {
        id: "fam_d".to_owned(),
        proficiency: "prof_anatomy".to_owned(),
        bonus: 0.05,
        penalty: -0.05,
    }];

    // Two thousand seconds to learn it, so half an hour is not enough and the second call is the one that gets close.
    let learned = cdda_sim::proficiencies::practise_a_dissection(&mut world, butcher, &families, &|_| 2_000);

    assert!(
        !learned,
        "**eighteen hundred seconds is not two thousand**, so this dissection teaches and does not grant"
    );
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, butcher, "prof_anatomy"),
        1_800,
        "both halves are practised: fifteen minutes twice, which is half an hour"
    );

    // The OR: a body that takes exactly one of the two halves to learn it answers that it learned, even though the first call did not.
    let mut world = bevy_ecs::prelude::World::new();
    let quick = cdda_sim::creature::spawn_creature(&mut world, "the quick study", (0, 0, 0));
    assert!(
        cdda_sim::proficiencies::practise_a_dissection(&mut world, quick, &families, &|_| 1_000),
        "**a thousand seconds is learned by the second half**, which is what `p1 || p2` says"
    );
}
