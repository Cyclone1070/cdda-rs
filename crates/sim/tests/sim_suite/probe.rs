//! A probe, kept small: it says which step of reading a hand-written program fails.
#![allow(clippy::expect_used)]

/// The three steps of installing a program, asserted one at a time.
#[test]
fn a_hand_written_program_reads_step_by_step() {
    let effect = cdda_json::parse(br#"{"u_message": "marked"}"#.as_slice()).expect("json");
    assert!(
        cdda_content::dialogue::read_effect(&effect).is_some(),
        "an effect entry reads"
    );

    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_mark", "effect": [{"u_message": "marked"}]}"#.as_slice(),
    )
    .expect("json");
    assert!(
        cdda_content::dialogue::read_program(&program).is_some(),
        "the program reads"
    );

    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let read = cdda_content::dialogue::read_program(&program).expect("readable");
    cdda_sim::eoc_registry::set_programs(&mut world, &[read]);
    assert_eq!(
        cdda_sim::eoc_registry::run_program(&mut world, survivor, None, "EOC_mark", 0),
        cdda_sim::dialogue_effects::Applied::Done,
        "the program runs"
    );
    assert_eq!(cdda_sim::messages::lines(&world), vec!["marked".to_owned()]);
}

/// `u_run_npc_eocs` runs its program on each person the filters leave, and on nobody else.
///
/// Reference: `f_run_npc_eocs` (`npctalk.cpp:6552-6600`): people are narrowed by `unique_ids` when given, by `npc_range` around the actor when given, and
/// otherwise by the actor's own level.
#[test]
fn programs_run_on_the_people_the_filters_leave() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let near = cdda_sim::creature::spawn_creature(&mut world, "the shopkeeper", (2, 0, 0));
    let far = cdda_sim::creature::spawn_creature(&mut world, "the hermit", (20, 0, 0));
    let below = cdda_sim::creature::spawn_creature(&mut world, "the digger", (2, 0, -1));
    cdda_sim::npcs::make_npc(&mut world, near, "SHOPKEEPER");
    cdda_sim::npcs::make_npc(&mut world, far, "HERMIT");
    cdda_sim::npcs::make_npc(&mut world, below, "DIGGER");

    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_mark", "effect": [{"u_message": "marked"}]}"#.as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_program(&program).expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, std::slice::from_ref(&read));

    // The effect is run through a program, because that is the only caller that knows how deep a run is: `run_effect` on its own refuses a program call.
    let mark = read.clone();
    let runs = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let wrapper = format!(r#"{{"type": "effect_on_condition", "id": "EOC_runner", "effect": [{text}]}}"#);
        let value = cdda_json::parse(wrapper.as_bytes()).expect("json");
        let program = cdda_content::dialogue::read_program(&value).expect("the program reads");
        // Both the wrapper and the program it names have to be installed: the census's own rule is that a program naming a program it cannot find refuses.
        cdda_sim::eoc_registry::set_programs(world, &[program, mark.clone()]);
        cdda_sim::eoc_registry::run_program(world, survivor, None, "EOC_runner", 0)
    };

    // No range: everyone on the actor's own level, so the digger below is left out.
    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        runs(&mut world, r#"{"u_run_npc_eocs": ["EOC_mark"]}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        cdda_sim::messages::lines(&world).len(),
        2,
        "the shopkeeper and the hermit are on this level"
    );

    // A range of five leaves the hermit twenty squares out untouched.
    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        runs(&mut world, r#"{"u_run_npc_eocs": ["EOC_mark"], "npc_range": 5}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world).len(), 1);

    // A unique id narrows to one person; an id nobody has runs on nobody and still answers.
    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        runs(&mut world, r#"{"u_run_npc_eocs": ["EOC_mark"], "unique_ids": ["HERMIT"]}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world).len(), 1);

    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        runs(&mut world, r#"{"u_run_npc_eocs": ["EOC_mark"], "unique_ids": ["NOBODY"]}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert!(cdda_sim::messages::lines(&world).is_empty());

    // A program that is not installed refuses rather than passing silently.
    assert_eq!(
        runs(&mut world, r#"{"u_run_npc_eocs": ["EOC_not_installed"]}"#),
        cdda_sim::dialogue_effects::Applied::CannotAnswer
    );
}
