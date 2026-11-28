//! Which zone an activity works in.
//!
//! Reference: `get_zone_for_act` (`activity_item_handling.cpp:2642-2688`).

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

/// Gives a person a tool, because the activity gates ask what they carry.
///
/// Reference: the mining branch wants a `DIG_TOOL` **flag** (`:1165-1171`), fishing a `FISHING_ROD` quality and chopping an `AXE` (`:1191`, `:1201`) - both read with the
/// default level of zero, which is "has the quality at all".
fn give_tool(
    world: &mut World,
    npc: bevy_ecs::prelude::Entity,
    id: &str,
    flags: &[&str],
    qualities: &[(&str, i32)],
) {
    cdda_sim::item_registry::set_item_types(
        world,
        &[cdda_sim::item_registry::Kind {
            id,
            volume_ml: 500,
            weight_g: 1_000,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let kind = cdda_sim::item_registry::id_of(world, id).expect("the tool kind");
    if !flags.is_empty() {
        cdda_sim::item_registry::set_item_flags(world, &[(kind, flags)]);
    }
    if !qualities.is_empty() {
        let held: Vec<(String, i32)> = qualities
            .iter()
            .map(|(name, level)| ((*name).to_owned(), *level))
            .collect();
        cdda_sim::item_registry::set_item_qualities(world, &[(kind, &held)]);
    }
    world.spawn((
        cdda_sim::GroundItem {
            kind,
            charges: 1,
            age: 0,
            burnt: 0,
            damage: 0,
            burn: None,
        },
        cdda_sim::creature::Carried(npc),
    ));
}

/// Gives a person a butchering knife, because the gate asks what they are carrying.
///
/// Reference: `:1254` wants `has_quality( qual_BUTCHER, 1 )`. The knife is a real kind the data has, with the level the data gives it.
fn give_a_knife(world: &mut World, npc: bevy_ecs::prelude::Entity) {
    cdda_sim::item_registry::set_item_types(
        world,
        &[cdda_sim::item_registry::Kind {
            id: "knife_butcher",
            volume_ml: 250,
            weight_g: 200,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 8.0,
            melee_stab: 10.0,
            to_hit: 1,
        }],
    );
    let knife = cdda_sim::item_registry::id_of(world, "knife_butcher").expect("the knife kind");
    cdda_sim::item_registry::set_item_qualities(world, &[(knife, &[("BUTCHER".to_owned(), 1)])]);
    world.spawn((
        cdda_sim::GroundItem {
            kind: knife,
            charges: 1,
            age: 0,
            burnt: 0,
            damage: 0,
            burn: None,
        },
        cdda_sim::creature::Carried(npc),
    ));
}

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
    give_a_knife(&mut world, vlad);
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    // **Both zones hold a corpse**, because a zone without one is not work at all (see `can_work_at`) - and this test is about distance, not about the work being there.
    for square in [(3, 0, 0), (10, 10, 0)] {
        world.spawn((
            cdda_sim::Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            cdda_sim::CorpseOf {
                kind: cdda_sim::MonsterKindId(1),
                died_on: 0,
                revives: false,
                monster_volume_ml: 62_500,
            },
        ));
    }
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
    give_a_knife(&mut world, vlad);
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
    give_a_knife(&mut world, vlad);
    world.entity_mut(vlad).insert(butcher.clone());
    world.spawn((
        cdda_sim::Position { x: 60, y: 0, z: 0 },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 62_500,
        },
    ));
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
    give_a_knife(&mut world, vlad);
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
    give_a_knife(&mut world, vlad);
    cdda_sim::creature::set_moves(&mut world, vlad, 100);
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    world.spawn((
        cdda_sim::Position { x: 4, y: 0, z: 0 },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 62_500,
        },
    ));
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
    give_a_knife(&mut world, vlad);
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

/// **A person standing on a corpse in a corpse zone starts butchering it.** This is the first job that does **work** rather than walking: `Ready` becomes the butchery activity,
/// with the corpse as its subject, which is what the reference's `can_butcher_at` + `ACT_BUTCHER` do (`activity_item_handling.cpp`, the butchering branch).
///
/// The subject matters more than anything else here: a butchering activity with no subject, or with the wrong one, finishes over a person who is not standing on anything and hands
/// out a harvest that never existed.
#[test]
fn a_job_standing_on_a_corpse_starts_butchering_it() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (2, 2, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    give_a_knife(&mut world, vlad);
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    let corpse = world
        .spawn((
            cdda_sim::Position { x: 2, y: 2, z: 0 },
            cdda_sim::CorpseOf {
                kind: cdda_sim::MonsterKindId(1),
                died_on: 0,
                revives: false,
                monster_volume_ml: 62_500,
            },
        ))
        .id();
    cdda_sim::zones::place(
        &mut world,
        (2, 2, 0),
        (2, 2),
        "LOOT_CORPSE",
        "your_followers",
        "the corpse",
        "",
    );
    assert_eq!(
        cdda_sim::multiple_work::work_a_turn(&mut world, vlad, "your_followers", (66, 66)),
        cdda_sim::multiple_work::TurnOfWork::Started,
        "**the job starts the work it is standing on**"
    );
    let doing = world
        .get::<cdda_sim::creature::Doing>(vlad)
        .expect("the person is doing something");
    assert_eq!(
        doing.activity, "butcher",
        "**butchering, by the name the finisher answers to**"
    );
    assert_eq!(
        doing.subject,
        Some(corpse),
        "**and the corpse is the subject, not the job's old activity**"
    );
    assert!(
        doing.moves_left > 0 && doing.moves_left == doing.moves_total,
        "**with the cut's own length: {} of {}**",
        doing.moves_left,
        doing.moves_total
    );
}

/// **A work square with no corpse in the zone is not work at all, and the job ends.** The reference's butcher branch does not merely refuse to start the cut on an empty square -
/// `can_butcher_at` collects the corpses at the spot and answers `NO_ZONE` when there are none (`activity_item_handling.cpp:1210-1265`), which is a **failure for that spot**. With no
/// other spot, the job has nothing to do and ends.
#[test]
fn a_job_whose_zone_has_no_corpse_ends() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (2, 2, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    give_a_knife(&mut world, vlad);
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    // **A corpse three squares away is not the corpse here**, and it is not in the zone either.
    world.spawn((
        cdda_sim::Position { x: 5, y: 5, z: 0 },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 62_500,
        },
    ));
    cdda_sim::zones::place(
        &mut world,
        (2, 2, 0),
        (2, 2),
        "LOOT_CORPSE",
        "your_followers",
        "the square",
        "",
    );
    assert_eq!(
        cdda_sim::multiple_work::work_a_turn(&mut world, vlad, "your_followers", (66, 66)),
        cdda_sim::multiple_work::TurnOfWork::Ended,
        "**the zone has no body in it, so there is nothing to do**"
    );
    assert!(
        world.get::<cdda_sim::creature::Doing>(vlad).is_none(),
        "**and the job is over**"
    );
}

/// **A job walks past a zone square it cannot work to reach one it can.** The near zone holds no corpse; the far zone holds one. The reference collects the spots, sorts them by
/// distance and **skips** the ones `can_do_activity_there` fails (`activity_item_handling.cpp:3367-3420`), so the person's nearest *workable* square is in the far zone - and that is
/// the square the job walks to.
#[test]
fn a_job_skips_a_zone_square_it_cannot_work() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    give_a_knife(&mut world, vlad);
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    // **The near zone is empty.**
    cdda_sim::zones::place(
        &mut world,
        (2, 0, 0),
        (2, 0),
        "LOOT_CORPSE",
        "your_followers",
        "empty",
        "",
    );
    // **The far zone has the body**, and it is farther than the empty square.
    world.spawn((
        cdda_sim::Position { x: 6, y: 0, z: 0 },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 62_500,
        },
    ));
    cdda_sim::zones::place(
        &mut world,
        (6, 0, 0),
        (6, 0),
        "LOOT_CORPSE",
        "your_followers",
        "with a body",
        "",
    );
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        Some((6, 0, 0)),
        "**the workable square, not the nearest one**"
    );
}

/// **The whole loop, over turns**: a person walks to the corpse, starts butchering it, and the cut is the reference's own length.
///
/// The last assertion is the number the reference's arithmetic produces for this state - `450` (a medium body) `* 100` (`max( 25, 100 - factor )` with no tool)
/// `* 6` (full butchering, not field dressed) `* 1.8` (no butchering, advanced or skinning practice) `= 486,000` moves, which a time-based activity spends a hundred at a time.
/// It is pinned because **every factor is a rule**: a cut that starts at the wrong length is a person stuck for hours, and that is exactly what the first version of this test found -
/// it expected forty turns, and forty turns is not the reference's answer.
///
/// The finish is **not** waited for here: 4,860 turns of a real turn loop is thirty seconds of test for a rule that has its own test below.
#[test]
fn a_person_walks_to_the_corpse_and_starts_the_cut() {
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
    give_a_knife(&mut world, vlad);
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    world.spawn((
        cdda_sim::Position { x: 3, y: 0, z: 0 },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 62_500,
        },
    ));
    cdda_sim::zones::place(
        &mut world,
        (3, 0, 0),
        (3, 0),
        "LOOT_CORPSE",
        "your_followers",
        "the corpse",
        "",
    );
    let mut walked = false;
    let mut cut = None;
    for _ in 0..12 {
        cdda_sim::creature::set_moves(&mut world, vlad, 100);
        cdda_sim::turn::run_turn(&mut world);
        if world
            .get::<cdda_sim::Position>(vlad)
            .is_some_and(|place| place.x > 0)
        {
            walked = true;
        }
        if let Some(doing) = world
            .get::<cdda_sim::creature::Doing>(vlad)
            .filter(|doing| doing.activity == "butcher")
        {
            cut = Some((doing.moves_left, doing.moves_total));
        }
    }
    assert!(walked, "**the person walked towards the corpse**");
    let cut = cut.expect("**and started butchering it**");
    // **The blade is a speed, not just a permission.** Reference: `butchery.cpp` scales the cut by `( 100 - tool_factor ) / 100`, which is what
    // `butcher_time_to_cut` calls the tool's percentage; a BUTCHER level of one therefore takes one percent off. The base is 450 (a medium body) * 6 (a full butchery) * 1.8
    // (the practice penalty the reference keeps as a factor), and the knife's percent comes off that.
    assert_eq!(
        cut.1, 481_140,
        "**the cut is the reference's own length: 450 * 100 * 6 * 1.8, less the blade's one percent**"
    );
}

/// **A cut whose moves run out finishes in that turn.** The activity is handed a hundred moves and run out, so the finisher is called by the turn loop itself - which is the
/// half of the pipeline the test above deliberately does not wait for.
#[test]
fn a_cut_whose_moves_run_out_finishes_in_that_turn() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (2, 2, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    cdda_sim::creature::set_moves(&mut world, vlad, 100);
    let corpse = world
        .spawn((
            cdda_sim::Position { x: 2, y: 2, z: 0 },
            cdda_sim::CorpseOf {
                kind: cdda_sim::MonsterKindId(1),
                died_on: 0,
                revives: false,
                monster_volume_ml: 62_500,
            },
        ))
        .id();
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "butcher".to_owned(),
        moves_left: 100,
        basis: cdda_sim::creature::Basis::Time,
        moves_total: 100,
        subject: Some(corpse),
    });
    cdda_sim::turn::run_turn(&mut world);
    assert!(
        world.get::<cdda_sim::creature::Doing>(vlad).is_none(),
        "**the cut ran out and the activity is cleared**"
    );
}

/// **A practised butcher's cut is shorter, and by the reference's own proportion.** `butchery.cpp:452-470` turns the three practice terms into penalties (truncated to whole numbers) and
/// then into a fraction of the cut: `0.4 * (1 + butch) + 0.4 * (1 + skin) + 0.2`, which is `1.8` with no practice, `1.0` with butchering-basic known, and `1.4` with
/// butchering-advanced known but not basic.
///
/// The three numbers are pinned because each is a different combination of the terms: a cut that ignores practice would give `486,000` for all three, and one that only reads basic
/// would give `270,000` where `378,000` is right.
#[test]
fn a_practised_butcher_cuts_faster() {
    fn cut_for(proficiencies: &[&str]) -> i32 {
        let mut world = World::new();
        let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (2, 2, 0));
        cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
        give_a_knife(&mut world, vlad);
        for proficiency in proficiencies {
            cdda_sim::proficiencies::learn(&mut world, vlad, proficiency);
        }
        world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
            activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
            moves_left: 0,
            basis: cdda_sim::creature::Basis::Neither,
            moves_total: 0,
            subject: None,
        });
        world.spawn((
            cdda_sim::Position { x: 2, y: 2, z: 0 },
            cdda_sim::CorpseOf {
                kind: cdda_sim::MonsterKindId(1),
                died_on: 0,
                revives: false,
                monster_volume_ml: 62_500,
            },
        ));
        cdda_sim::zones::place(
            &mut world,
            (2, 2, 0),
            (2, 2),
            "LOOT_CORPSE",
            "your_followers",
            "the corpse",
            "",
        );
        cdda_sim::multiple_work::work_a_turn(&mut world, vlad, "your_followers", (66, 66));
        world
            .get::<cdda_sim::creature::Doing>(vlad)
            .map_or(0, |doing| doing.moves_total)
    }
    // Every figure is the reference's `450 * 100 * 6` less the blade's one percent, because the butcher carries a knife and `butcher_time_to_cut` scales by
    // `( 100 - tool_factor )`: 486,000 -> 481,140, 270,000 -> 267,300, 378,000 -> 374,220.
    assert_eq!(
        cut_for(&[]),
        481_140,
        "**no practice: 450 * 100 * 6 * 1.8, less the blade's one percent**"
    );
    assert_eq!(
        cut_for(&["prof_butchering_basic"]),
        267_300,
        "**butchering-basic: the butchering penalty drops to nothing, so the fraction is 1.0**"
    );
    assert_eq!(
        cut_for(&["prof_butchering_advanced"]),
        374_220,
        "**advanced without basic: one of the two butchering terms is gone, so 1.4**"
    );
}

/// **A job whose gate is not ported still finds its spots.** Mining is the example: its real gate wants the terrain's `MINEABLE` flag and a digging tool, neither of which the port
/// has, so `can_work_at` answers `true` for it - and a mining zone with no corpse in it is still a mining spot. Answering `false` for the unported activities would end every one of
/// them on its first turn, which is why this test exists.
#[test]
fn an_unported_jobs_spots_are_not_gated_on_corpses() {
    let mut world = World::new();
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_MOP".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    cdda_sim::zones::place(
        &mut world,
        (3, 0, 0),
        (3, 0),
        "MOPPING",
        "your_followers",
        "a spill",
        "",
    );
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        Some((3, 0, 0)),
        "**a mopping spot needs no corpse**"
    );
    assert!(
        cdda_sim::multiple_work::can_work_at(&mut world, vlad, "ACT_MULTIPLE_MOP", (3, 0, 0)),
        "**and the spot itself is workable, because its real gate is not ported yet**"
    );
}

/// **Two people do not butcher the same body.** The reference skips a corpse that already carries `activity_var` (`activity_item_handling.cpp:1213`): a body somebody is working on is
/// not a work spot for anybody else, and the second person looks elsewhere or ends.
///
/// The port does not add a flag to the corpse - the store already says who is working on what, in the activity's `subject` - so this is derived, and it cannot go stale.
#[test]
fn two_people_do_not_take_the_same_corpse() {
    let mut world = World::new();
    let first = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (2, 2, 0));
    cdda_sim::npcs::make_npc(&mut world, first, "vlad");
    give_a_knife(&mut world, first);
    let second = cdda_sim::creature::spawn_creature(&mut world, "Igor", (2, 2, 0));
    cdda_sim::npcs::make_npc(&mut world, second, "igor");
    give_a_knife(&mut world, second);
    let corpse = world
        .spawn((
            cdda_sim::Position { x: 2, y: 2, z: 0 },
            cdda_sim::CorpseOf {
                kind: cdda_sim::MonsterKindId(1),
                died_on: 0,
                revives: false,
                monster_volume_ml: 62_500,
            },
        ))
        .id();
    cdda_sim::zones::place(
        &mut world,
        (2, 2, 0),
        (2, 2),
        "LOOT_CORPSE",
        "your_followers",
        "the corpse",
        "",
    );
    // **Nobody is working on it yet**, so it is a spot.
    assert!(
        cdda_sim::multiple_work::can_work_at(&mut world, first, "ACT_MULTIPLE_BUTCHER", (2, 2, 0)),
        "**an untouched corpse is work**"
    );
    // **The first person takes it.**
    world.entity_mut(first).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    assert_eq!(
        cdda_sim::multiple_work::work_a_turn(&mut world, first, "your_followers", (66, 66)),
        cdda_sim::multiple_work::TurnOfWork::Started,
        "**Vlad starts on it**"
    );
    // **Now it is Vlad's**: Igor sees no spot here, and his job ends because none is left.
    assert!(
        !cdda_sim::multiple_work::can_work_at(&mut world, first, "ACT_MULTIPLE_BUTCHER", (2, 2, 0)),
        "**a body somebody is butchering is not a work spot**"
    );
    // **But only that body is taken.** A second corpse beside it is still work for Igor, which is what makes the rule "this corpse", not "any corpse" - and what a check that
    // only asked whether *somebody* was butchering would get wrong.
    world.spawn((
        cdda_sim::Position { x: 3, y: 3, z: 0 },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 62_500,
        },
    ));
    cdda_sim::zones::place(
        &mut world,
        (3, 3, 0),
        (3, 3),
        "LOOT_CORPSE",
        "your_followers",
        "another body",
        "",
    );
    assert!(
        cdda_sim::multiple_work::can_work_at(&mut world, second, "ACT_MULTIPLE_BUTCHER", (3, 3, 0)),
        "**an untouched body is work even while another one is being cut**"
    );
    world.entity_mut(second).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    // **Igor's job is not ended**: the second body is a spot, so it walks towards it - the same reason the reference does not cancel a job while any spot remains.
    world.entity_mut(second).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    assert!(
        cdda_sim::multiple_work::next_work_square(&mut world, second, "your_followers").is_some(),
        "**Igor's job finds the other body**"
    );
    // **And when Vlad's cut is over, the body is work again** - the derivation is what makes that automatic.
    world
        .entity_mut(first)
        .remove::<cdda_sim::creature::Doing>();
    assert!(
        cdda_sim::multiple_work::can_work_at(&mut world, first, "ACT_MULTIPLE_BUTCHER", (2, 2, 0)),
        "**with nobody on it, the corpse is a spot again**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Doing>(first)
            .map(|doing| doing.subject),
        None,
        "**and the corpse was the subject of the cut that ended: {corpse:?}**"
    );
}

/// **A mining job skips ground that is not mineable.**
///
/// Reference: the mining branch of `can_do_activity_there` (`activity_item_handling.cpp:1160-1163`): a square without `TFLAG_MINEABLE` answers `NO_ZONE`, and the handler
/// passes over it. **The tool half is not answered here** - the branch also wants a digging tool, and the port has no item to ask - so this pins the terrain rule alone: the
/// pavement is worked, the grass beside it is not.
#[test]
fn a_mining_spot_whose_ground_is_not_mineable_is_skipped() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_grass".to_owned(), 1), ("t_concrete".to_owned(), 2)],
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [("t_concrete".to_owned(), vec!["MINEABLE".to_owned()])],
    );
    let grass = cdda_sim::terrain_types::code_of(&world, "t_grass").expect("grass is registered");
    let concrete =
        cdda_sim::terrain_types::code_of(&world, "t_concrete").expect("concrete is registered");
    cdda_sim::square::set_terrain(&mut world, (2, 0, 0), grass);
    cdda_sim::square::set_terrain(&mut world, (6, 0, 0), concrete);

    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_MINE".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    // The near zone is grass and the far one is pavement: the near square is nearer, and still not the one walked to.
    cdda_sim::zones::place(
        &mut world,
        (2, 0, 0),
        (2, 0),
        "MINING",
        "your_followers",
        "grass",
        "",
    );
    cdda_sim::zones::place(
        &mut world,
        (6, 0, 0),
        (6, 0),
        "MINING",
        "your_followers",
        "pavement",
        "",
    );
    // **No digging tool, no spot**, which is the branch's other half (`:1165-1171`).
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        None,
        "**mineable ground is not enough without a tool**"
    );
    give_tool(&mut world, vlad, "pickaxe", &["DIG_TOOL"], &[]);
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        Some((6, 0, 0)),
        "**the mineable square, not the nearest one**"
    );
}

/// **A fishing job skips dry ground.**
///
/// Reference: the fishing branch of `can_do_activity_there` (`activity_item_handling.cpp:1188-1191`): a square without `TFLAG_FISHABLE` answers `NO_ZONE`. The rod is the
/// other half of the branch and is not modelled, which is why this test is about the water only.
#[test]
fn a_fishing_spot_on_dry_ground_is_skipped() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_dirt".to_owned(), 1), ("t_water_sh".to_owned(), 2)],
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [("t_water_sh".to_owned(), vec!["FISHABLE".to_owned()])],
    );
    let dirt = cdda_sim::terrain_types::code_of(&world, "t_dirt").expect("dirt is registered");
    let water =
        cdda_sim::terrain_types::code_of(&world, "t_water_sh").expect("water is registered");
    cdda_sim::square::set_terrain(&mut world, (2, 0, 0), dirt);
    cdda_sim::square::set_terrain(&mut world, (6, 0, 0), water);

    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_FISH".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    cdda_sim::zones::place(
        &mut world,
        (2, 0, 0),
        (2, 0),
        "FISHING_SPOT",
        "your_followers",
        "shore",
        "",
    );
    cdda_sim::zones::place(
        &mut world,
        (6, 0, 0),
        (6, 0),
        "FISHING_SPOT",
        "your_followers",
        "water",
        "",
    );
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        None,
        "**water is not enough without a rod**"
    );
    give_tool(
        &mut world,
        vlad,
        "fishing_rod_basic",
        &[],
        &[("FISHING_ROD", 1)],
    );
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        Some((6, 0, 0)),
        "**the water, not the shore**"
    );
}

/// **A chopping job wants a tree, or a trunk, or a stump.**
///
/// Reference: the chopping branch of `can_do_activity_there` (`activity_item_handling.cpp:1198-1209`): `ter( src_loc ) == t_trunk || == t_stump || has_flag( TFLAG_TREE )`,
/// and anything else answers `NO_ZONE`. The axe is the branch's other half and is not modelled; the terrain rule stands on its own.
#[test]
fn a_chopping_job_only_works_a_tree() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_grass".to_owned(), 1),
            ("t_tree".to_owned(), 2),
            ("t_trunk".to_owned(), 3),
        ],
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [("t_tree".to_owned(), vec!["TREE".to_owned()])],
    );
    let grass = cdda_sim::terrain_types::code_of(&world, "t_grass").expect("grass is registered");
    let tree = cdda_sim::terrain_types::code_of(&world, "t_tree").expect("tree is registered");
    let trunk = cdda_sim::terrain_types::code_of(&world, "t_trunk").expect("trunk is registered");
    cdda_sim::square::set_terrain(&mut world, (2, 0, 0), grass);
    cdda_sim::square::set_terrain(&mut world, (6, 0, 0), tree);
    cdda_sim::square::set_terrain(&mut world, (8, 0, 0), trunk);

    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_CHOP_TREES".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    for (square, name) in [(2, "grass"), (6, "tree"), (8, "trunk")] {
        cdda_sim::zones::place(
            &mut world,
            (square, 0, 0),
            (square, 0),
            "CHOP_TREES",
            "your_followers",
            name,
            "",
        );
    }
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        None,
        "**a tree is not enough without an axe**"
    );
    give_tool(&mut world, vlad, "hand_axe", &[], &[("AXE", 1)]);
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        Some((6, 0, 0)),
        "**the tree, not the grass before it**"
    );
}

/// **A trunk is a chopping spot without the tree flag.**
///
/// Reference: `:1199` compares `ter( src_loc )` to `t_trunk` and `t_stump` **before** asking `TFLAG_TREE`, and neither id carries it. A rule that only asked the flag would find
/// nothing to do here and stop at the grass.
#[test]
fn a_trunk_is_a_chopping_spot_without_the_tree_flag() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_grass".to_owned(), 1), ("t_trunk".to_owned(), 2)],
    );
    let grass = cdda_sim::terrain_types::code_of(&world, "t_grass").expect("grass is registered");
    let trunk = cdda_sim::terrain_types::code_of(&world, "t_trunk").expect("trunk is registered");
    cdda_sim::square::set_terrain(&mut world, (2, 0, 0), grass);
    cdda_sim::square::set_terrain(&mut world, (6, 0, 0), trunk);
    let vlad = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    world.entity_mut(vlad).insert(cdda_sim::creature::Doing {
        activity: "ACT_MULTIPLE_CHOP_TREES".to_owned(),
        moves_left: 0,
        basis: cdda_sim::creature::Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    for (square, name) in [(2, "grass"), (6, "trunk")] {
        cdda_sim::zones::place(
            &mut world,
            (square, 0, 0),
            (square, 0),
            "CHOP_TREES",
            "your_followers",
            name,
            "",
        );
    }
    give_tool(&mut world, vlad, "hand_axe", &[], &[("AXE", 1)]);
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        Some((6, 0, 0)),
        "**the trunk, not the grass before it**"
    );
}

/// **A butcher with no blade finds no work.**
///
/// Reference: `activity_item_handling.cpp:1254` - a spot where the corpses are but `has_quality( qual_BUTCHER, 1 )` fails answers `NEEDS_BUTCHERING` and is **skipped**. So a
/// survivor carrying nothing walks past every body instead of starting a cut that could never finish. The knife is the difference and nothing else is: the same world with one
/// given to them finds the spot, which is the sibling test's job.
#[test]
fn a_butcher_without_a_knife_finds_no_work() {
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
    world.spawn((
        cdda_sim::Position { x: 3, y: 0, z: 0 },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 62_500,
        },
    ));
    cdda_sim::zones::place(
        &mut world,
        (3, 0, 0),
        (3, 0),
        "LOOT_CORPSE",
        "your_followers",
        "the corpse",
        "",
    );
    assert!(
        !cdda_sim::multiple_work::can_work_at(&mut world, vlad, "ACT_MULTIPLE_BUTCHER", (3, 0, 0)),
        "**bare hands are not a butchering tool**"
    );
    assert_eq!(
        cdda_sim::multiple_work::next_work_square(&mut world, vlad, "your_followers"),
        None,
        "**so the body is not a spot, and the job has nothing to walk to**"
    );
}
