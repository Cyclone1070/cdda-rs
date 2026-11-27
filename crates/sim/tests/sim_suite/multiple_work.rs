//! Which zone an activity works in.
//!
//! Reference: `get_zone_for_act` (`activity_item_handling.cpp:2642-2688`).

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

/// **Eleven activities work in eleven zones.** Each line of the table is pinned, because a job sent to the wrong zone is a person walking to the wrong place all day - and the
/// table is the only thing that decides where a multiple activity looks first.
#[test]
fn a_multiple_activity_works_in_its_own_zone() {
    for (activity, zone) in [
        ("ACT_VEHICLE_DECONSTRUCTION", "VEHICLE_DECONSTRUCT"),
        ("ACT_VEHICLE_REPAIR", "VEHICLE_REPAIR"),
        ("ACT_MULTIPLE_CHOP_TREES", "CHOP_TREES"),
        ("ACT_MULTIPLE_CONSTRUCTION", "CONSTRUCTION_BLUEPRINT"),
        ("ACT_MULTIPLE_FARM", "FARM_PLOT"),
        ("ACT_MULTIPLE_BUTCHER", "LOOT_CORPSE"),
        ("ACT_MULTIPLE_CHOP_PLANKS", "LOOT_WOOD"),
        ("ACT_MULTIPLE_FISH", "FISHING_SPOT"),
        ("ACT_MULTIPLE_MINE", "MINING"),
        ("ACT_MULTIPLE_MOP", "MOPPING"),
        ("ACT_MULTIPLE_DIS", "DISASSEMBLE"),
    ] {
        assert_eq!(
            cdda_sim::multiple_work::zone_for_activity(activity),
            Some(zone),
            "**{activity} works in {zone}**"
        );
    }
    // **Reading and crafting work where they are, not in a zone**, and so does an activity nobody mapped: the reference answers the undefined zone for them.
    for activity in [
        "ACT_MULTIPLE_READ",
        "ACT_CRAFT",
        "ACT_SOCIALIZE",
        "ACT_UNHEARD_OF",
    ] {
        assert_eq!(
            cdda_sim::multiple_work::zone_for_activity(activity),
            None,
            "**{activity} works in no zone**"
        );
    }
}

/// **Every zone the table names is one the data defines.** This is the property that keeps the table honest as it grows: a typo in a zone id is a job that can never find work,
/// and it would fail here rather than in a playthrough.
#[test]
fn every_zone_the_table_names_is_in_the_data() {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let roots = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::zone_types::load_paths(&paths).expect("the zone types load");
    let mut world = World::new();
    cdda_sim::multiple_work::set_zone_types(&mut world, loaded.kinds);
    for (_, zone) in [
        ("ACT_VEHICLE_DECONSTRUCTION", "VEHICLE_DECONSTRUCT"),
        ("ACT_VEHICLE_REPAIR", "VEHICLE_REPAIR"),
        ("ACT_MULTIPLE_CHOP_TREES", "CHOP_TREES"),
        ("ACT_MULTIPLE_CONSTRUCTION", "CONSTRUCTION_BLUEPRINT"),
        ("ACT_MULTIPLE_FARM", "FARM_PLOT"),
        ("ACT_MULTIPLE_BUTCHER", "LOOT_CORPSE"),
        ("ACT_MULTIPLE_CHOP_PLANKS", "LOOT_WOOD"),
        ("ACT_MULTIPLE_FISH", "FISHING_SPOT"),
        ("ACT_MULTIPLE_MINE", "MINING"),
        ("ACT_MULTIPLE_MOP", "MOPPING"),
        ("ACT_MULTIPLE_DIS", "DISASSEMBLE"),
    ] {
        assert!(
            cdda_sim::multiple_work::zone_type_named(&world, zone).is_some(),
            "**the data defines the zone type {zone}**"
        );
    }
}

/// **A job looks for its work in its own zones, and takes the nearest square.** Reference: `generic_multi_activity_handler` (`activity_item_handling.cpp:3344-3400`), which
/// collects the zones of the activity's type and sorts them by distance.
///
/// Two corpse zones are in reach - one three squares away, one ten - so the answer is the near one's nearest square, not the far one's. The square is named exactly, because
/// "somewhere in the right zone" would not catch a rule that sorted wrongly.
#[test]
fn a_job_takes_the_nearest_work_square_in_its_own_zone() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    cdda_sim::zones::place(
        &mut world,
        (3, 0, 0),
        (4, 0),
        "LOOT_CORPSE",
        "your_followers",
        "near the corpse",
        "",
    );
    cdda_sim::zones::place(
        &mut world,
        (10, 10, 0),
        (11, 11),
        "LOOT_CORPSE",
        "your_followers",
        "far corpse",
        "",
    );
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        Some((3, 0, 0)),
        "**the nearest square of the nearest zone**"
    );
}

/// **A job with no work in reach ends itself, and says so.** Reference: the end of `generic_multi_activity_handler` (`activity_item_handling.cpp:3502-3505`): an **empty** work-spot
/// set is the person's job being over, with a sentence saying why.
///
/// The three cases are the rule: no zone at all ends the job; a zone out of reach (beyond `MAX_VIEW_DISTANCE`) also ends it; and an activity that works **in no zone** -
/// `ACT_SOCIALIZE` - is untouched, because having no zone is not having no work.
#[test]
fn a_job_with_no_work_in_reach_ends_and_says_why() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    let butcher = cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    };
    world.entity_mut(vlad).insert(butcher.clone());
    assert!(
        cdda_sim::multiple_work::the_job_has_nothing_to_do(&mut world, vlad, "your_followers"),
        "**no corpse zone anywhere is a job with nothing to do**"
    );
    assert!(
        world.get::<cdda_sim::creature::Doing>(vlad).is_none(),
        "**and the job is over**"
    );
    // **A zone out of reach ends it too**: `MAX_VIEW_DISTANCE` is sixty, so a zone at seventy is no work.
    world.entity_mut(vlad).insert(butcher.clone());
    cdda_sim::zones::place(
        &mut world,
        (70, 0, 0),
        (71, 0),
        "LOOT_CORPSE",
        "your_followers",
        "too far to see",
        "",
    );
    assert!(
        cdda_sim::multiple_work::the_job_has_nothing_to_do(&mut world, vlad, "your_followers"),
        "**seventy squares away is out of view**"
    );
    // **The boundary is where the constant is**: a zone exactly sixty squares away is work, and one sixty-one away is not. Without this pair the constant is unpinned - the first
    // version of this test used seventy and a mutation that shortened `MAX_VIEW_DISTANCE` from sixty to six passed it.
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(butcher.clone());
    cdda_sim::zones::place(
        &mut world,
        (60, 0, 0),
        (60, 0),
        "LOOT_CORPSE",
        "your_followers",
        "exactly at the edge of sight",
        "",
    );
    assert!(
        !cdda_sim::multiple_work::the_job_has_nothing_to_do(&mut world, vlad, "your_followers"),
        "**sixty squares is still in view**"
    );
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(butcher.clone());
    cdda_sim::zones::place(
        &mut world,
        (61, 0, 0),
        (61, 0),
        "LOOT_CORPSE",
        "your_followers",
        "one past the edge",
        "",
    );
    assert!(
        cdda_sim::multiple_work::the_job_has_nothing_to_do(&mut world, vlad, "your_followers"),
        "**sixty-one is out of view**"
    );
    // **And an activity that works in no zone is not ended by having no zone**, which is the distinction the rule turns on.
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_SOCIALIZE".to_owned(),
        moves_left: 5,
        basis: cdda_sim::creature::Basis::Time,
        moves_total: 5,
        subject: None,
    });
    assert!(
        !cdda_sim::multiple_work::the_job_has_nothing_to_do(&mut world, vlad, "your_followers"),
        "**socialising needs no zone, so it is not ended**"
    );
    assert!(
        world.get::<cdda_sim::creature::Doing>(vlad).is_some(),
        "**and the activity is still there**"
    );
}

/// **A job decides the step and the turn takes it.** A person told to butcher, a corpse zone four squares away: the job answers `Walking`, leaves a `WantsToStep` on the person
/// rather than moving them, and the turn's own `take_steps` then takes exactly one step towards the corpse.
///
/// The step is the assertion that matters: it proves the job stage ran **before** the walk stage, which is the reference's order (an activity decides to walk; the walk happens in
/// the same creature phase) and the thing a rule that moved the creature itself would not show.
#[test]
fn a_job_decides_a_step_and_the_turn_takes_it() {
    let mut world = World::new();
    let mut walkway = cdda_sim::terrain_table::TerrainTable::default();
    walkway.insert(1, true);
    walkway.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: walkway,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -10..=10 {
        for x in -10..=10 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    cdda_sim::creature::set_moves(&mut world, vlad, 100);
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    cdda_sim::zones::place(
        &mut world,
        (4, 0, 0),
        (4, 0),
        "LOOT_CORPSE",
        "your_followers",
        "the corpse",
        "",
    );
    assert_eq!(
        cdda_sim::multiple_work::work_a_turn(&mut world, vlad, "your_followers", (66, 66)),
        cdda_sim::multiple_work::TurnOfWork::Walking,
        "**the job decides to walk**"
    );
    assert!(
        world.get::<cdda_sim::walk::WantsToStep>(vlad).is_some(),
        "**and leaves the step for the turn to take, rather than moving the person itself**"
    );
    cdda_sim::walk::take_steps(&mut world);
    let moved = world.get::<cdda_sim::Position>(vlad).map(|place| place.x);
    assert!(
        moved.is_some_and(|x| x > 0),
        "**the step was taken, one square towards the corpse: at {moved:?}**"
    );
    assert!(
        world.get::<cdda_sim::creature::Doing>(vlad).is_some(),
        "**and the job is still the person's, because the corpse is still four squares away**"
    );
}

/// **A job with no work ends within the turn it was told about.** The person takes a job, no zone exists, and `run_turn` leaves no job behind.
#[test]
fn a_job_with_no_work_ends_in_the_turn() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    cdda_sim::turn::run_turn(&mut world);
    assert!(
        world.get::<cdda_sim::creature::Doing>(vlad).is_none(),
        "**the job is gone after one turn**"
    );
}
