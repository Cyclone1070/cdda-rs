//! How likely a craft is to go right, and when it is checked.
//!
//! Reference: `Character::recipe_success_roll_data` and `item::set_next_failure_point` (`crafting.cpp:1172-1344`).

use bevy_ecs::prelude::*;

/// The centre is the crafter's own level, the spread narrows as the crafter outclasses the recipe, and the difficulty is one more than the recipe's.
///
/// Reference: `recipe_success_roll_data` (`crafting.cpp:1172-1263`). A level-five crafter on a level-two recipe: centre five, difficulty three, and the spread two
/// narrowed by a quarter of the two points it is ahead - one and a half.
#[test]
fn the_roll_widens_when_the_recipe_is_hard_and_narrows_when_it_is_easy() {
    let easy = cdda_sim::craft_roll::success_data(2, 5);
    assert!((easy.center - 5.0).abs() < 1e-9, "the centre is the crafter's own level");
    assert!((easy.final_difficulty - 3.0).abs() < 1e-9, "the difficulty is the recipe's plus one");
    assert!((easy.stddev - 1.5).abs() < 1e-9, "two narrowed by a quarter of the two-point lead");

    // A level-two crafter on a level-six recipe: centre two, difficulty seven, and the spread widened by a third of the five-point gap.
    let hard = cdda_sim::craft_roll::success_data(6, 2);
    assert!((hard.center - 2.0).abs() < 1e-9);
    assert!((hard.final_difficulty - 7.0).abs() < 1e-9);
    assert!((hard.stddev - (2.0 + 5.0 / 3.0)).abs() < 1e-9, "got {}", hard.stddev);

    // The failure check is the same numbers, quartered and halved: difficulty ( 3 - 1 ) / 4 = 0.5 and a spread of 0.75.
    let failure = cdda_sim::craft_roll::failure_data(2, 5);
    assert!((failure.center - 5.0).abs() < 1e-9);
    assert!((failure.final_difficulty - 0.5).abs() < 1e-9);
    assert!((failure.stddev - 0.75).abs() < 1e-9);
}

/// The next check is the progress now plus the roll, so a good crafter is left alone and a poor one is checked again soon.
///
/// Reference: `item::set_next_failure_point` (`crafting.cpp:1337`): `next = item_counter + roll * 10000000`.
#[test]
fn the_next_failure_point_is_where_the_roll_puts_it() {
    assert_eq!(cdda_sim::craft_roll::next_failure_point(0, 1.0), 10_000_000);
    assert_eq!(cdda_sim::craft_roll::next_failure_point(2_000_000, 0.5), 7_000_000);
    // A roll of nothing puts the next check where the work already is, which is the reference's own "check again immediately".
    assert_eq!(cdda_sim::craft_roll::next_failure_point(4_000_000, 0.0), 4_000_000);
}

/// The roll is drawn from the world's dice, and a better crafter rolls higher.
///
/// Reference: `crafting_success_roll` (`crafting.cpp:1275-1284`): the draw is `normal_roll( center, stddev )` with the difficulty taken off and the answer floored
/// at zero.
#[test]
fn a_better_crafter_rolls_higher_on_the_same_dice() {
    let mut world = World::new();
    let mut better = 0.0;
    let mut worse = 0.0;
    for seed in 0..32 {
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        better += cdda_sim::craft_roll::success_roll(&mut world, cdda_sim::craft_roll::success_data(3, 8));
    }
    for seed in 0..32 {
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        worse += cdda_sim::craft_roll::success_roll(&mut world, cdda_sim::craft_roll::success_data(8, 3));
    }
    assert!(
        better > worse,
        "a level-eight crafter on a level-three recipe must beat a level-three crafter on a level-eight one: {better} against {worse}"
    );
    assert!(worse >= 0.0, "a roll never goes below zero");
}

/// A failed check destroys some of what went in, and a bad enough one destroys all of it.
///
/// Reference: `item::handle_craft_failure` (`crafting.cpp:1360-1400`): at most three quarters of the components are tried, each skipped on the failure roll, and a
/// craft with nothing left is catastrophic. A crafter who is hopeless at the work has a failure roll near zero, so nothing is skipped.
#[test]
fn a_failure_destroys_what_went_in_and_can_destroy_all_of_it() {
    let mut world = World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned()],
        by_name: [("rag".to_owned(), cdda_sim::ItemTypeId(1))].into_iter().collect(),
        volumes_ml: vec![0, 250],
        weights_g: vec![0, 5],
        holds_ml: vec![0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0)],
    };
    let mut recipe = cdda_content::recipes::Recipe {
        id: "patch_a_shirt".to_owned(),
        result: "rag".to_owned(),
        blacklisted: false,
        components: vec![vec![cdda_content::recipes::Component {
            item: "rag".to_owned(),
            count: 1,
        }]],
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: "tailor".to_owned(),
        difficulty: 9,
        batch_rscale: 0.0,
        batch_rsize: 0,
        byproducts: Vec::new(),
        made_per_batch: 1,
        byproduct_group: Vec::new(),
        proficiencies: Vec::new(),
        container: None,
        time_moves: 6_000,
    };
    for _ in 0..4 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    }
    // The roll decides how much is skipped, so the rule is judged over many seeds rather than one: **some** failures must destroy something, and none may destroy
    // more than three quarters of what went in.
    let mut ever_destroyed = 0;
    let mut most_destroyed = 0;
    let mut lost_progress = 0;
    for seed in 0..64 {
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let failure = cdda_sim::craft_roll::handle_failure(&mut world, crafter, &recipe, 5_000_000, &names);
        ever_destroyed += i32::from(failure.destroyed > 0);
        most_destroyed = most_destroyed.max(failure.destroyed);
        lost_progress += i32::from(failure.progress_lost > 0);
    }
    assert!(
        ever_destroyed > 0,
        "a failure that can only skip every component proves nothing"
    );
    assert!(
        most_destroyed <= 3,
        "at most three quarters of four components are ever tried, and that is three, not {most_destroyed}"
    );
    assert!(
        lost_progress > 0,
        "a failure that destroys something also costs progress"
    );

    // With every component destroyed by the same rule, the failure is catastrophic: the craft cannot continue.
    let mut world = World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
    world.entity_mut(rag).remove::<cdda_sim::Position>();
    world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
    recipe.components = vec![vec![cdda_content::recipes::Component {
        item: "rag".to_owned(),
        count: 1,
    }]];
    let mut saw_catastrophic = false;
    for seed in 0..64 {
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let last = cdda_sim::craft_roll::handle_failure(&mut world, crafter, &recipe, 5_000_000, &names);
        if last.destroyed == 1 {
            assert!(last.catastrophic, "losing every component ends the craft");
            saw_catastrophic = true;
        }
        if last.destroyed == 0 {
            assert!(!last.catastrophic, "a craft that lost nothing is not catastrophic");
        }
    }
    assert!(
        saw_catastrophic,
        "a one-component craft must sometimes lose it, or the catastrophic rule is unreachable"
    );
}

/// A craft in progress is checked where its roll says, and a hopeless crafter is checked again and again.
///
/// Reference: `craft_activity_actor::do_turn` (`activity_actor.cpp:4299`): `craft.item_counter >= craft.get_next_failure_point()` starts the failure handling. The
/// reach is the point of this test - `handle_failure` was written a round earlier with **nothing firing it**.
#[test]
fn a_craft_in_progress_is_checked_where_its_roll_says() {
    let mut ever_failed = 0;
    for seed in 0..24 {
        let mut world = World::new();
        let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let names = cdda_sim::item_registry::ItemNames {
            names: vec![String::new(), "rag".to_owned()],
            by_name: [("rag".to_owned(), cdda_sim::ItemTypeId(1))].into_iter().collect(),
            volumes_ml: vec![0, 250],
            weights_g: vec![0, 5],
            holds_ml: vec![0, 0],
            pockets: Vec::new(),
            pocket_ranges: vec![(0, 0)],
        };
        // A trade at zero against a recipe that needs a lot of it: the roll is low, so the craft is checked almost at once and again after every failure.
        let recipe = cdda_content::recipes::Recipe {
            id: "patch_a_shirt".to_owned(),
            result: "rag".to_owned(),
            blacklisted: false,
            components: vec![vec![cdda_content::recipes::Component {
                item: "rag".to_owned(),
                count: 1,
            }]],
            tools: Vec::new(),
            qualities: Vec::new(),
            skill: "tailor".to_owned(),
            difficulty: 9,
            batch_rscale: 0.0,
            batch_rsize: 0,
            byproducts: Vec::new(),
            made_per_batch: 1,
            byproduct_group: Vec::new(),
            proficiencies: Vec::new(),
            container: None,
            time_moves: 2_000,
        };
        for _ in 0..8 {
            let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
            world.entity_mut(rag).remove::<cdda_sim::Position>();
            world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
        }
        cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
        assert_eq!(
            cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
            cdda_sim::crafting::Craft::Started
        );
        let rags_before = {
            let mut query = world.query::<&cdda_sim::creature::Carried>();
            query.iter(&world).filter(|carried| carried.0 == crafter).count()
        };
        for _ in 0..40 {
            cdda_sim::creature::give_moves(&mut world, crafter, 100);
            cdda_sim::turn::run_turn(&mut world);
        }
        let rags_after = {
            let mut query = world.query::<&cdda_sim::creature::Carried>();
            query.iter(&world).filter(|carried| carried.0 == crafter).count()
        };
        if rags_after < rags_before
            || world.get::<cdda_sim::crafting::Making>(crafter).is_none()
        {
            ever_failed += 1;
        }
    }
    assert!(
        ever_failed > 0,
        "a survivor who cannot do the trade must sometimes ruin the work, or the check never fires"
    );
}
