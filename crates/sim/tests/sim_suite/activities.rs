//! Things that take more than one turn.
//!
//! Reference: `player_activity::do_turn` (`player_activity.cpp:276-294`). The rule has two halves that look
//! interchangeable and are not: a **time**-based activity takes a hundred moves a turn whatever the creature's speed,
//! and a **speed**-based one takes everything the creature has. A quick creature therefore finishes a speed-based job
//! sooner and a time-based one no sooner at all - which is the difference between reading a book and digging a hole.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::creature::{Basis, Doing, give_moves, moves_left, set_speed, spawn_creature};
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Prototypes, singletons};

const FLOOR: u32 = 1;

fn a_world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..4 {
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
    singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    world
}

fn doing(activity: &str, moves: i32, basis: Basis) -> Doing {
    Doing {
        activity: activity.to_owned(),
        moves_left: moves,
        basis,
        subject: None,
    }
}

#[test]
fn a_time_based_job_takes_the_same_three_turns_for_a_quick_creature_and_a_slow_one() {
    let turns = |speed: i32| {
        let mut world = a_world();
        // Two creatures so the turn loop has something to run for; the busy one is the one being timed.
        let busy = spawn_creature(&mut world, "the survivor", (0, 0, 0));
        set_speed(&mut world, busy, speed, 0);
        world
            .entity_mut(busy)
            .insert(doing("reading", 250, Basis::Time));
        let mut turns = 0;
        for _ in 0..10 {
            cdda_sim::turn::run_turn(&mut world);
            turns += 1;
            if world.get::<Doing>(busy).is_none() {
                break;
            }
        }
        turns
    };

    assert_eq!(turns(100), 3, "two hundred and fifty moves at a hundred a turn");
    assert_eq!(
        turns(200),
        3,
        "and the same for a creature twice as quick: a time-based job is not quicker for being quick"
    );
}

#[test]
fn a_speed_based_job_is_quicker_for_a_quicker_creature() {
    let turns = |speed: i32| {
        let mut world = a_world();
        let busy = spawn_creature(&mut world, "the survivor", (0, 0, 0));
        set_speed(&mut world, busy, speed, 0);
        world
            .entity_mut(busy)
            .insert(doing("digging", 400, Basis::Speed));
        let mut turns = 0;
        for _ in 0..20 {
            cdda_sim::turn::run_turn(&mut world);
            turns += 1;
            if world.get::<Doing>(busy).is_none() {
                break;
            }
        }
        turns
    };

    assert_eq!(turns(100), 4, "four hundred moves at a hundred a turn");
    assert_eq!(turns(200), 2, "and two turns for a creature that has twice the moves");
}

#[test]
fn a_busy_creature_does_not_walk() {
    let mut world = a_world();
    let busy = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, busy, 100);
    world
        .entity_mut(busy)
        .insert(doing("reading", 1_000, Basis::Time));
    world
        .entity_mut(busy)
        .insert(cdda_sim::creature::Goal {
            square: (3, 0, 0),
        });

    cdda_sim::turn::run_turn(&mut world);

    let place = world.get::<cdda_sim::Position>(busy).expect("it has a square");
    assert_eq!(
        (place.x, place.y),
        (0, 0),
        "a creature doing something spends its turn on that, not on walking"
    );
    assert!(moves_left(&world, busy) <= 0, "and it has no moves left to spend");
}

/// An activity that finishes does what it was for.
///
/// Reference: `activity_handlers::pickup_finish` - the transfer happens when the activity ends, not when it starts.
/// The gate walks the whole errand: something too heavy to handle in one turn, a job that takes two, and the item in
/// hand at the end with the floor empty.
#[test]
fn a_pickup_too_heavy_for_one_turn_finishes_on_the_turn_it_should() {
    let mut world = a_world();
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    // A hundred moves a turn, banked by the turn loop itself rather than handed over here.
    set_speed(&mut world, survivor, 100, 0);
    // No moves given: each turn the creature banks its own hundred, so two litres of handling cost - two hundred
    // moves - is two turns' work. (Giving it a hundred up front would bank a second hundred on the first turn and
    // finish in one, which is the reference's own accounting, not a shortcut.)
    // Two litres: 100 + 100 moves.
    let rock = cdda_sim::drop_item(&mut world, (1, 0), cdda_sim::ItemTypeId(9), 1);

    assert_eq!(
        cdda_sim::creature::begin_pick_up(&mut world, survivor, rock, 2_000),
        cdda_sim::creature::Began::Started,
        "it does not fit in the moves the creature has"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        1,
        "so it is still on the floor"
    );

    cdda_sim::turn::run_turn(&mut world);
    assert!(
        world.get::<cdda_sim::creature::Doing>(survivor).is_some(),
        "one turn is not enough"
    );
    assert_eq!(cdda_sim::items_at(&mut world, (1, 0)).len(), 1);

    cdda_sim::turn::run_turn(&mut world);
    assert!(
        world.get::<cdda_sim::creature::Doing>(survivor).is_none(),
        "the second turn finishes it"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        0,
        "and the item is off the floor"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Carried>(rock),
        Some(&cdda_sim::creature::Carried(survivor)),
        "in the survivor's hands"
    );
}

#[test]
fn something_light_enough_to_handle_at_once_is_not_an_activity() {
    let mut world = a_world();
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 500);
    let rock = cdda_sim::drop_item(&mut world, (1, 0), cdda_sim::ItemTypeId(9), 1);

    assert_eq!(
        cdda_sim::creature::begin_pick_up(&mut world, survivor, rock, 250),
        cdda_sim::creature::Began::Done,
        "a quarter of a litre is a hundred and twelve moves, and the creature has five hundred"
    );
    assert!(world.get::<cdda_sim::creature::Doing>(survivor).is_none(), "no activity was needed");
    assert_eq!(cdda_sim::items_at(&mut world, (1, 0)).len(), 0, "and it is in hand already");
}

/// The rule on its own, without the turn loop around it.
#[test]
fn spending_a_turn_directly_moves_the_counter() {
    let mut world = a_world();
    let busy = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, busy, 100);
    world
        .entity_mut(busy)
        .insert(doing("digging", 200, Basis::Speed));
    let finished = cdda_sim::creature::spend_a_turn_on(&mut world, busy);
    assert!(!finished);
    assert_eq!(world.get::<Doing>(busy).map(|doing| doing.moves_left), Some(100));
}

/// Every activity the port starts has something that finishes it, and the table is what says so.
///
/// The port used to finish activities with a chain of `if let`s on a string. That works until the second activity, and
/// then it is a table written the long way: the reference keeps `activity_handlers::finish`'s dispatch table for exactly
/// this reason. The count here is the gate - a `begin_*` added without an entry would start something nothing can end.
#[test]
fn every_activity_has_a_finisher() {
    use cdda_sim::creature::FINISHERS;
    use std::collections::BTreeSet;

    let known: BTreeSet<&str> = FINISHERS.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        known.len(),
        FINISHERS.len(),
        "no activity is listed twice, or one of them would never run"
    );
    // Every activity the port can start. This list is the assertion: adding a `begin_*` means adding its id here, and a
    // missing finisher then fails this test instead of silently never finishing.
    let started = ["craft", "pickup", "wear"];
    assert_eq!(
        known,
        started.iter().copied().collect(),
        "the table and the starters disagree"
    );
    assert_eq!(FINISHERS.len(), 3, "three activities can be finished");
}

/// An activity nobody knows simply ends, which is the reference's answer to an unhandled activity.
#[test]
fn an_unknown_activity_ends_and_changes_nothing() {
    let mut world = World::new();
    let creature = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let before = cdda_sim::inspect::world_fingerprints(&world);
    cdda_sim::creature::finish_activity(&mut world, creature, "levitate", None);
    assert_eq!(
        before,
        cdda_sim::inspect::world_fingerprints(&world),
        "an activity with no handler must not touch the store"
    );
}

/// Giving up on a craft gives back everything it was using, and the work is lost.
///
/// Reference: `Character::cancel_activity` -> `player_activity::canceled` -> `craft_activity_actor::canceled`. The two
/// halves matter equally: a craft that kept its ingredients after being cancelled would let a survivor make a thing by
/// starting it and walking away, and one that consumed them would make cancelling a punishment the reference never
/// intended.
#[test]
fn cancelling_a_craft_keeps_the_ingredients_and_loses_the_work() {
    use cdda_sim::crafting::{Craft, Making, begin_craft_from_carried};
    use cdda_sim::creature::{Doing, spawn_creature, stop_doing};

    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned(), "rope".to_owned()],
        by_name: [
            ("rag".to_owned(), cdda_sim::ItemTypeId(1)),
            ("rope".to_owned(), cdda_sim::ItemTypeId(2)),
        ]
        .into_iter()
        .collect(),
        volumes_ml: vec![0, 250, 500],
        weights_g: vec![0, 100, 200],
        holds_ml: vec![0, 0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0), (0, 0), (0, 0)],
    };
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }
    let recipe = cdda_content::recipes::Recipe {
        id: "tie_a_rope".to_owned(),
        result: "rope".to_owned(),
        blacklisted: false,
        components: vec![vec![cdda_content::recipes::Component {
            item: "rag".to_owned(),
            count: 2,
        }]],
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves: 20_000,
        batch_rscale: 0.0,
        batch_rsize: 0,
        result_mult: 1,
        byproducts: Vec::new(),
        result_charges: None,
    };
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );

    assert!(stop_doing(&mut world, crafter), "there was something to give up on");

    assert!(world.get::<Doing>(crafter).is_none(), "the survivor is free");
    assert!(
        world.get::<Making>(crafter).is_some(),
        "but the recipe is still theirs: a paused craft is not a lost one"
    );
    let carried = cdda_sim::inventory::carried_by_id(&mut world, crafter, |kind| names.name_of(kind));
    assert_eq!(carried.get("rag"), Some(&2), "both rags are still in hand");
    assert_eq!(carried.get("rope"), None, "and no rope was made");
    assert!(
        !stop_doing(&mut world, crafter),
        "giving up on nothing answers that there was nothing"
    );
    // Starting again costs the whole recipe: a new craft is a new craft.
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );
    assert_eq!(
        world.get::<Doing>(crafter).map(|doing| doing.moves_left),
        Some(20_000),
        "twenty thousand moves, not the remainder"
    );
}

/// Every activity that can be finished can also be given up on.
///
/// The reference has a `canceled` on the actor for the same reason it has a `finish`: an activity that could only end by
/// completing would be one a survivor could never change their mind about.
#[test]
fn every_activity_has_a_canceller() {
    use cdda_sim::creature::{CANCELLERS, FINISHERS};
    use std::collections::BTreeSet;

    let finished: BTreeSet<&str> = FINISHERS.iter().map(|(id, _)| *id).collect();
    let cancelled: BTreeSet<&str> = CANCELLERS.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        finished, cancelled,
        "every activity that can end one way must be able to end the other"
    );
    assert_eq!(cancelled.len(), 3, "three activities");
}

/// Work that was paused is picked up where it was left, not redone.
///
/// Reference: `Character::backlog` and `resume_backlog_activity` (`character.cpp:9112-9126`). The distinction from
/// starting again is the whole point: an interruption should cost a survivor time, not the twenty thousand moves they
/// had already spent. A pickup, by contrast, cannot be resumed - the thing is still on the floor where it started - so
/// it is not queued at all.
#[test]
fn a_paused_craft_is_resumed_where_it_stopped() {
    use cdda_sim::creature::{Doing, resume_backlog, spawn_creature, stop_doing};
    use cdda_sim::crafting::{Craft, begin_craft_from_carried};

    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned(), "rope".to_owned()],
        by_name: [
            ("rag".to_owned(), cdda_sim::ItemTypeId(1)),
            ("rope".to_owned(), cdda_sim::ItemTypeId(2)),
        ]
        .into_iter()
        .collect(),
        volumes_ml: vec![0, 250, 500],
        weights_g: vec![0, 100, 200],
        holds_ml: vec![0, 0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0), (0, 0), (0, 0)],
    };
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }
    let recipe = cdda_content::recipes::Recipe {
        id: "tie_a_rope".to_owned(),
        result: "rope".to_owned(),
        blacklisted: false,
        components: vec![vec![cdda_content::recipes::Component {
            item: "rag".to_owned(),
            count: 2,
        }]],
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves: 20_000,
        batch_rscale: 0.0,
        batch_rsize: 0,
        result_mult: 1,
        byproducts: Vec::new(),
        result_charges: None,
    };
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );
    // Some of the work gets done, then something interrupts it.
    if let Some(mut doing) = world.get_mut::<Doing>(crafter) {
        doing.moves_left = 5_000;
    }
    assert!(stop_doing(&mut world, crafter));

    assert!(resume_backlog(&mut world, crafter), "there is work to resume");
    assert_eq!(
        world.get::<Doing>(crafter).map(|doing| doing.moves_left),
        Some(5_000),
        "fifteen thousand moves of work are not thrown away"
    );
    // The backlog is empty now, and a busy creature cannot resume.
    assert!(
        !resume_backlog(&mut world, crafter),
        "nothing is left to resume while the craft is under way again"
    );
}

#[test]
fn a_pickup_that_was_stopped_is_not_queued() {
    use cdda_sim::creature::{Basis, Doing, resume_backlog, spawn_creature, stop_doing};

    let mut world = World::new();
    let creature = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world.entity_mut(creature).insert(Doing {
        activity: "pickup".to_owned(),
        moves_left: 100,
        basis: Basis::Speed,
        subject: None,
    });
    assert!(stop_doing(&mut world, creature));
    assert!(
        !resume_backlog(&mut world, creature),
        "a pickup has nothing to resume: the thing is still on the floor"
    );
}

/// Being hurt interrupts a craft, and the work goes to the backlog rather than being lost.
///
/// Reference: `player_activity::is_distraction_ignored` (`player_activity.cpp:492-495`) decides whether the activity
/// heeds the distraction at all; the reference then cancels it, which for a resumable activity means it lands in the
/// backlog with its progress (`character.cpp:9112-9115`). The three rules compose, and this is the composition: pain
/// stops the work, and nothing is thrown away by being stopped.
#[test]
fn pain_interrupts_a_craft_and_the_work_is_kept() {
    use cdda_sim::creature::{
        Basis, Distraction, Doing, disturb, heeds, resume_backlog, spawn_creature,
    };
    use cdda_sim::crafting::{Craft, Making, begin_craft_from_carried};

    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned(), "rope".to_owned()],
        by_name: [
            ("rag".to_owned(), cdda_sim::ItemTypeId(1)),
            ("rope".to_owned(), cdda_sim::ItemTypeId(2)),
        ]
        .into_iter()
        .collect(),
        volumes_ml: vec![0, 250, 500],
        weights_g: vec![0, 100, 200],
        holds_ml: vec![0, 0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0), (0, 0), (0, 0)],
    };
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }
    let recipe = cdda_content::recipes::Recipe {
        id: "tie_a_rope".to_owned(),
        result: "rope".to_owned(),
        blacklisted: false,
        components: vec![vec![cdda_content::recipes::Component {
            item: "rag".to_owned(),
            count: 2,
        }]],
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves: 20_000,
        batch_rscale: 0.0,
        batch_rsize: 0,
        result_mult: 1,
        byproducts: Vec::new(),
        result_charges: None,
    };
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );
    if let Some(mut doing) = world.get_mut::<Doing>(crafter) {
        doing.moves_left = 8_000;
    }

    assert!(
        disturb(&mut world, crafter, Distraction::Pain),
        "pain is not something a craft heeds past"
    );
    assert!(world.get::<Doing>(crafter).is_none(), "the craft stopped");
    assert!(world.get::<Making>(crafter).is_some(), "but the recipe is still theirs");
    assert!(resume_backlog(&mut world, crafter));
    assert_eq!(
        world.get::<Doing>(crafter).map(|doing| doing.moves_left),
        Some(8_000),
        "and twelve thousand moves of work are still done"
    );

    // A creature doing nothing is not disturbed by anything.
    cdda_sim::creature::stop_doing(&mut world, crafter);
    assert!(!disturb(&mut world, crafter, Distraction::Pain));
    // And an activity nobody has an opinion about is interrupted by default.
    assert!(heeds("anything_at_all", Distraction::Noise));
    // A pickup, interrupted, is not queued - so what waits to be resumed is still the craft, not the errand.
    cdda_sim::creature::stop_doing(&mut world, crafter);
    world.entity_mut(crafter).insert(Doing {
        activity: "pickup".to_owned(),
        moves_left: 50,
        basis: Basis::Speed,
        subject: None,
    });
    assert!(disturb(&mut world, crafter, Distraction::Pain));
    assert!(resume_backlog(&mut world, crafter));
    assert_eq!(
        world.get::<Doing>(crafter).map(|doing| doing.activity.clone()),
        Some("craft".to_owned()),
        "the craft is what was waiting; the pickup left nothing behind"
    );
}

/// Being hit stops the work, and the world does the hitting rather than a test.
///
/// Reference: `Character::on_hurt` (`character.cpp:8564-8590`) raises `distraction_type::attacked` after damage lands.
/// The previous round ported the distraction path but nothing in the game called it; this is the caller, and the gate is
/// that harm arriving through `hurt` - the function every blow already goes through - is enough on its own.
#[test]
fn a_blow_stops_a_craft_without_anyone_asking_it_to() {
    use cdda_sim::creature::{Doing, health_of, hurt, resume_backlog, spawn_creature};
    use cdda_sim::crafting::{Craft, Making, begin_craft_from_carried};

    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(crafter)
        .insert(cdda_sim::creature::Health { hp: 80, max_hp: 80 });
    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned(), "rope".to_owned()],
        by_name: [
            ("rag".to_owned(), cdda_sim::ItemTypeId(1)),
            ("rope".to_owned(), cdda_sim::ItemTypeId(2)),
        ]
        .into_iter()
        .collect(),
        volumes_ml: vec![0, 250, 500],
        weights_g: vec![0, 100, 200],
        holds_ml: vec![0, 0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0), (0, 0), (0, 0)],
    };
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }
    let recipe = cdda_content::recipes::Recipe {
        id: "tie_a_rope".to_owned(),
        result: "rope".to_owned(),
        blacklisted: false,
        components: vec![vec![cdda_content::recipes::Component {
            item: "rag".to_owned(),
            count: 2,
        }]],
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves: 20_000,
        batch_rscale: 0.0,
        batch_rsize: 0,
        result_mult: 1,
        byproducts: Vec::new(),
        result_charges: None,
    };
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );

    hurt(&mut world, crafter, 9.0);

    assert_eq!(health_of(&world, crafter), Some(71), "nine points of harm landed");
    assert!(world.get::<Doing>(crafter).is_none(), "and stopped the craft");
    assert!(world.get::<Making>(crafter).is_some(), "without losing the recipe");
    assert!(resume_backlog(&mut world, crafter), "the work is waiting");
    assert_eq!(
        world.get::<Doing>(crafter).map(|doing| doing.moves_left),
        Some(20_000),
        "the craft had not begun to be paid down in this test, so all of it is left"
    );
    // A blow that does no harm is not a distraction.
    hurt(&mut world, crafter, 0.0);
    assert!(
        world.get::<Doing>(crafter).is_some(),
        "a scratch that takes nothing off does not interrupt anything"
    );
}

/// Focus scales what a practice amount is worth, and a fresh survivor starts at the reference's hundred.
///
/// Reference: `Character::adjust_for_focus` (`character.cpp:10172-10175`): `amount * ( get_effective_focus() / 100.0f )`. This is the first half of the crafting
/// practice rule - `Character::practice` multiplies the ticks by it - and it is here on its own because the focus scale is a rule about attention, not about crafting.
#[test]
fn focus_scales_what_a_practice_worths() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert_eq!(
        cdda_sim::skills::Focus::default().0,
        100,
        "a fresh survivor is at a hundred"
    );
    assert_eq!(
        world.get::<cdda_sim::skills::Focus>(survivor).map(|focus| focus.0),
        Some(100),
        "and the spawn gives it"
    );
    assert!((cdda_sim::skills::adjust_for_focus(&world, survivor, 40.0) - 40.0).abs() < f32::EPSILON);
    world.entity_mut(survivor).insert(cdda_sim::skills::Focus(50));
    assert!((cdda_sim::skills::adjust_for_focus(&world, survivor, 40.0) - 20.0).abs() < f32::EPSILON);
}

/// Focus drifts towards the equilibrium morale sets, a tenth of the gap a minute.
///
/// Reference: `Character::calc_focus_equilibrium` (`character.cpp:3852-3920`) and `update_mental_focus` (`character.cpp:3929-3934`), the first thing
/// `reset_stats` does once a minute. The port had a focus value that never moved - so a depleted survivor never rested and a cheerful one never benefited.
#[test]
fn focus_drifts_towards_the_equilibrium_morale_sets() {
    // The equilibrium's own shape: morale below -99 floors it, up to fifty it moves one for one, above that the blocks double.
    assert_eq!(cdda_sim::skills::focus_equilibrium(0), 100);
    assert_eq!(cdda_sim::skills::focus_equilibrium(-150), 1);
    assert_eq!(cdda_sim::skills::focus_equilibrium(50), 150);
    assert_eq!(cdda_sim::skills::focus_equilibrium(150), 200);
    assert_eq!(cdda_sim::skills::focus_equilibrium(350), 250);
    assert_eq!(cdda_sim::skills::focus_equilibrium(5_000), 400, "the cap");
    // And the drift: a tenth of the difference each minute, in the pool's hundredths.
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::skills::update_mental_focus(&mut world, survivor, 0);
    assert_eq!(
        world.get::<cdda_sim::skills::Focus>(survivor).map(|focus| focus.0),
        Some(100),
        "at the equilibrium it stays put"
    );
    world.entity_mut(survivor).insert(cdda_sim::skills::Focus(40));
    cdda_sim::skills::update_mental_focus(&mut world, survivor, 0);
    assert_eq!(
        world.get::<cdda_sim::skills::Focus>(survivor).map(|focus| focus.0),
        Some(46),
        "forty to a hundred closes a tenth of the sixty-point gap"
    );
}
