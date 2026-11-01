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

/// `u_teleport` moves a creature to the square its variable holds.
///
/// Reference: `f_teleport` (`npctalk.cpp:7816-7845`): the member is an object naming the variable, and the creature's position becomes the point it holds.
#[test]
fn a_teleport_moves_to_the_stored_square() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -8..24 {
        for y in -8..24 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let runs = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
        cdda_sim::dialogue_effects::run_effect(world, survivor, None, &effect)
    };

    // A variable holding a square is somewhere to go.
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, false);
    cdda_sim::dialogue_values::set(
        &mut world,
        holder,
        "place",
        cdda_sim::dialogue_values::Value::Position { x: 7, y: 3, z: 0 },
    );
    assert_eq!(
        runs(&mut world, r#"{"u_teleport": {"u_val": "place"}}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let at = world.get::<cdda_sim::Position>(survivor).copied().expect("a position");
    assert_eq!((at.x, at.y, at.z), (7, 3, 0), "the creature stands where the variable said");

    // A variable that holds no location is nowhere to go, so the effect refuses rather than leaving the creature put and claiming success.
    cdda_sim::dialogue_values::set(
        &mut world,
        holder,
        "a_number",
        cdda_sim::dialogue_values::Value::Number(3.0),
    );
    assert_eq!(
        runs(&mut world, r#"{"u_teleport": {"u_val": "a_number"}}"#),
        cdda_sim::dialogue_effects::Applied::CannotAnswer
    );
    assert_eq!(
        runs(&mut world, r#"{"u_teleport": {"u_val": "never_set"}}"#),
        cdda_sim::dialogue_effects::Applied::CannotAnswer
    );

    // The outcome messages are members the port has no rule for, so an entry that states one is refused rather than half-run.
    let with_message =
        cdda_json::parse(br#"{"u_teleport": {"u_val": "place"}, "fail_message": "no"}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&with_message).is_none());
}

/// `assign_mission` gives the mission to the player, whoever is speaking.
///
/// Reference: `f_assign_mission` (`npctalk.cpp:5908-5926`): the type is reserved and assigned to `get_avatar()`, and a `deadline` sets the due turn after
/// assignment. The port's mission store keeps the active ids, so the deadline is refused rather than dropped.
#[test]
fn a_mission_is_given_to_the_player() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the shopkeeper", (1, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, survivor);

    let runs = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
        cdda_sim::dialogue_effects::run_effect(world, talker, None, &effect)
    };

    assert_eq!(
        runs(&mut world, r#"{"assign_mission": "MISSION_shopkeeper_errand"}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert!(
        cdda_sim::missions::has_mission(&world, survivor, "MISSION_shopkeeper_errand"),
        "the player holds it, though the shopkeeper was the one speaking"
    );
    assert!(
        !cdda_sim::missions::has_mission(&world, talker, "MISSION_shopkeeper_errand"),
        "the speaker does not keep a mission given to the player"
    );

    // The same mission twice is one mission, because the store holds the type ids a character has taken.
    assert_eq!(
        runs(&mut world, r#"{"assign_mission": "MISSION_shopkeeper_errand"}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        cdda_sim::missions::active_missions(&world, survivor),
        vec!["MISSION_shopkeeper_errand".to_owned()]
    );

    // A deadline is a member the port cannot carry out, so an entry stating one is refused rather than assigned without it.
    let with_deadline =
        cdda_json::parse(br#"{"assign_mission": "MISSION_x", "deadline": 100}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&with_deadline).is_none());
}

/// A spawn count can be a `math` expression, which the game evaluates where it is used.
///
/// Reference: `get_dbl_or_var` (`npctalk.cpp`) takes a number, a two-end array, **or** a `math` block, and the spawn data uses the third for counts that depend
/// on the world - `rand(2) + 1` hallucinations, for instance.
#[test]
fn a_spawn_count_can_be_computed() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let kind = cdda_content::monsters::load_objects(&[cdda_json::parse(
        br#"{"id": "mon_test", "name": "a test thing", "hp": 12, "speed": 100, "symbol": "t", "color": "white"}"#.as_slice(),
    )
    .expect("json")])
    .expect("readable")
    .into_iter()
    .next()
    .expect("one type");
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_test", hp: 12 }],
    );
    cdda_sim::monster_catalog::set_types(&mut world, vec![kind]);

    let spawn = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_spawn_monster": "mon_test", "real_count": {"math": ["2 + 3"]}}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("a computed count is readable");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(31));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &spawn),
        cdda_sim::dialogue_effects::Applied::Done
    );

    // Five creatures exist because the expression said five, which a fixed count of zero would not produce.
    let mut made = 0;
    let mut query = world
        .try_query::<&cdda_sim::creature::CreatureKind>()
        .expect("query");
    for _ in query.iter(&world) {
        made += 1;
    }
    assert_eq!(made, 5, "the math count decided how many were made");
}

/// `group: true` makes the member name a monster group, and the game draws one of its monsters.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7390`) with `MonsterGroupManager::GetMonsterFromGroup`: the group's weights decide which monster appears, so the
/// same call can produce different creatures on different draws.
#[test]
fn a_spawn_can_draw_from_a_group() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let types = cdda_content::monsters::load_objects(&[
        cdda_json::parse(
            br#"{"id": "mon_alpha", "name": "an alpha", "hp": 5, "speed": 100, "symbol": "a", "color": "white"}"#
                .as_slice(),
        )
        .expect("json"),
        cdda_json::parse(
            br#"{"id": "mon_beta", "name": "a beta", "hp": 5, "speed": 100, "symbol": "b", "color": "white"}"#
                .as_slice(),
        )
        .expect("json"),
    ])
    .expect("readable");
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[
            cdda_sim::monster_registry::MonsterKind { id: "mon_alpha", hp: 5 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_beta", hp: 5 },
        ],
    );
    cdda_sim::monster_catalog::set_types(&mut world, types);

    // A group of two monsters, the second twice as likely as the first.
    let group = cdda_json::parse(
        br#"{"type": "monstergroup", "id": "GROUP_test", "monsters": [{"monster": "mon_alpha", "weight": 1}, {"monster": "mon_beta", "weight": 2}]}"#
            .as_slice(),
    )
    .expect("json");
    let groups = cdda_content::monster_groups::load_objects(&[group]);
    assert_eq!(groups.len(), 1, "the group reads");
    cdda_sim::monster_groups::set_groups(&mut world, groups);

    let spawn = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_spawn_monster": "GROUP_test", "group": true, "real_count": 4, "min_radius": 1, "max_radius": 4}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a group spawn is readable");

    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(41));

    // The table itself answers before the effect is asked to: a spawn that refuses says nothing about which half failed.
    let mut probe_dice = cdda_core_types::rng::Rolls::new(7);
    let drawn = cdda_sim::monster_groups::pick(&world, "GROUP_test", &mut probe_dice);
    assert!(
        matches!(drawn.as_deref(), Some("mon_alpha" | "mon_beta")),
        "the group table picks a monster, and picked {drawn:?}"
    );

    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &spawn),
        cdda_sim::dialogue_effects::Applied::Done
    );

    // Four creatures exist, every one of them from the group - a group with no table, or a type that was not installed, would have refused instead.
    let mut made = 0;
    let mut query = world
        .try_query::<&cdda_sim::creature::CreatureKind>()
        .expect("query");
    for _ in query.iter(&world) {
        made += 1;
    }
    assert_eq!(made, 4, "four monsters were drawn from the group");

    // A group the world does not know refuses rather than making something else.
    let unknown = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_spawn_monster": "GROUP_missing", "group": true, "real_count": 1}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &unknown),
        cdda_sim::dialogue_effects::Applied::CannotAnswer
    );
}

/// Every monster the data defines loads, including the ones that copy from another monster in the same file.
///
/// Four spawns in the census refused with `unknown type`, and the catalogue is built by the same loader, so this says whether the story is "the file was never
/// read" or "the copy-from chain was not resolved" - two very different bugs.
#[test]
fn a_monster_that_copies_from_another_loads() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let monsters = cdda_content::monsters::load_paths(&[&data]).expect("the tree reads");
    let ids: Vec<&str> = monsters.iter().map(|kind| kind.id.as_str()).collect();
    assert!(ids.contains(&"mon_yrax_sphenocorona"), "the base loads");
    assert!(ids.contains(&"mon_golden_monolith"), "the monster that copies from it loads");
    assert!(ids.contains(&"mon_spider_cellar_giant"), "a base in another family loads");
    assert!(ids.len() > 1_000, "the whole catalogue is there: {}", ids.len());
}

/// In dialogue math, a variable nobody set is **zero**, so a program whose condition reads one can be evaluated.
///
/// Reference: `read_var_value` (`dialogue_helpers.cpp:11-16`) answers an empty `diag_value` for a variable that is not there, and `diag_value::dbl()`
/// (`math_parser_diag_value.cpp:168`) turns an empty value into zero - so `never_set + 1` is 1 and a comparison against zero is true.
#[test]
fn an_unset_variable_reads_as_zero() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let kind = cdda_content::monsters::load_objects(&[cdda_json::parse(
        br#"{"id": "mon_test", "name": "a test thing", "hp": 12, "speed": 100, "symbol": "t", "color": "white"}"#.as_slice(),
    )
    .expect("json")])
    .expect("readable")
    .into_iter()
    .next()
    .expect("one type");
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_test", hp: 12 }],
    );
    cdda_sim::monster_catalog::set_types(&mut world, vec![kind]);

    let spawn = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_spawn_monster": "mon_test", "real_count": {"math": ["never_set + 1"]}}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &spawn),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let mut made = 0;
    let mut query = world
        .try_query::<&cdda_sim::creature::CreatureKind>()
        .expect("query");
    for _ in query.iter(&world) {
        made += 1;
    }
    assert_eq!(made, 1, "an unset variable counted as zero, so one creature appeared");
}

/// `distance('u', 'npc')` answers how far apart two creatures are, counting a diagonal step as one square.
///
/// Reference: `distance_eval` (`math_parser_diag.cpp:185-201`) reads two points and answers `rl_dist`, which is `square_dist` unless the game's circular-distance
/// option is on: the **largest** axis distance, so a diagonal is one step and one storey up is one step too.
#[test]
fn two_creatures_are_as_far_apart_as_the_longest_axis() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let shopkeeper = cdda_sim::creature::spawn_creature(&mut world, "the shopkeeper", (3, 4, 0));

    let asks = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, Some(shopkeeper), &read)
    };

    // Three across and four down is four squares, not seven: the longest axis decides.
    assert!(asks(&world, "distance('u', 'npc') == 4"), "four squares apart");
    assert!(!asks(&world, "distance('u', 'npc') == 7"), "not seven");

    // A diagonal step is one square.
    if let Some(mut position) = world.get_mut::<cdda_sim::Position>(shopkeeper) {
        position.x = 1;
        position.y = 1;
    }
    assert!(asks(&world, "distance('u', 'npc') == 1"), "a diagonal is one step");

    // Asking for an actor that is not in the conversation refuses rather than answering zero.
    assert!(!asks(&world, "distance('u', 'npc') == 0"));
    let condition = cdda_json::parse(br#"{"math": ["distance('u', 'nobody') > 0"]}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_condition(&condition).expect("reads");
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, None, &read),
        "an unknown actor is a refusal, which answers false rather than a made-up distance"
    );
}

/// `u_pain()` answers how much pain the creature is in, and zero when nothing hurts.
///
/// Reference: `pain_eval` (`math_parser_diag.cpp:869-882`) answers the actor's `pain_cur`; `type: 'perceived'` asks for the pain as the character feels it, which
/// the port has no masking rule for, so that form is refused rather than answered with the raw number.
#[test]
fn pain_is_a_number_the_condition_can_read() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let asks = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    // A creature nobody has hurt is in no pain.
    assert!(asks(&world, "u_pain() == 0"), "no pain by default");

    world.entity_mut(survivor).insert(cdda_sim::creature::Pain(23));
    assert!(asks(&world, "u_pain() == 23"), "the number the store holds");

    // The perceived form is a rule the port does not have, so it refuses instead of answering the raw number under a different name.
    let perceived =
        cdda_json::parse(br#"{"math": ["u_pain('type': 'perceived') > 0"]}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_condition(&perceived).expect("the condition reads");
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, None, &read),
        "a refused function answers false rather than pretending"
    );
}

/// `u_monsters_nearby()` counts the monsters within a radius, which defaults to a thousand squares.
///
/// Reference: `_monsters_nearby_eval` (`math_parser_diag.cpp:800-840`): the radius comes from the `radius` keyword (default **1000**), an empty argument list
/// counts every monster, and a named type counts only that type. `_filter_monster` compares with `rl_dist`, so the radius is a Chebyshev one.
#[test]
fn monsters_are_counted_within_a_radius() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[
            cdda_sim::monster_registry::MonsterKind { id: "mon_zombie", hp: 40 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_dog", hp: 20 },
        ],
    );
    let zombie = cdda_sim::monster_registry::id_of(&mut world, "mon_zombie").expect("a kind");
    let dog = cdda_sim::monster_registry::id_of(&mut world, "mon_dog").expect("a kind");
    for (kind, square) in [(zombie, (2, 2, 0)), (zombie, (5, 0, 0)), (dog, (40, 0, 0))] {
        let fresh = cdda_sim::creature::spawn_creature(&mut world, "a monster", square);
        cdda_sim::creature::set_kind(&mut world, fresh, kind);
    }

    let asks = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    assert!(asks(&world, "u_monsters_nearby() == 3"), "every monster is inside the default radius");
    assert!(asks(&world, "u_monsters_nearby('mon_zombie') == 2"), "only the zombies");
    assert!(asks(&world, "u_monsters_nearby('radius': 3) == 1"), "the diagonal one counts as two squares out");
    assert!(asks(&world, "u_monsters_nearby('mon_dog', 'radius': 100) == 1"), "the dog is forty squares out");
    assert!(!asks(&world, "u_monsters_nearby('mon_dog') == 0"), "and it is still inside a thousand");

    // An attitude the port cannot mean is refused rather than answered with a number it does not have.
    let friendly =
        cdda_json::parse(br#"{"math": ["u_monsters_nearby('attitude': 'friendly') > 0"]}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_condition(&friendly).expect("reads");
    assert!(!cdda_sim::conditions::holds_with(&world, survivor, None, &read));
}

/// `u_run_monster_eocs` runs its program on the monsters the filters leave.
///
/// Reference: `f_run_monster_eocs` (`npctalk.cpp:6590-6700`): `mtype_ids` narrows by type, `monster_range` by Chebyshev distance from the actor, and the actor's
/// own level always has to match - a range narrows the square, it does not reach up or down a storey.
#[test]
fn programs_run_on_the_monsters_the_filters_leave() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[
            cdda_sim::monster_registry::MonsterKind { id: "mon_quad_leg", hp: 10 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_zombie", hp: 40 },
        ],
    );
    let leg = cdda_sim::monster_registry::id_of(&mut world, "mon_quad_leg").expect("a kind");
    let zombie = cdda_sim::monster_registry::id_of(&mut world, "mon_zombie").expect("a kind");
    for (kind, square) in [(leg, (1, 1, 0)), (leg, (20, 0, 0)), (zombie, (2, 0, 0))] {
        let fresh = cdda_sim::creature::spawn_creature(&mut world, "a monster", square);
        cdda_sim::creature::set_kind(&mut world, fresh, kind);
    }

    let mark = cdda_content::dialogue::read_program(
        &cdda_json::parse(
            br#"{"type": "effect_on_condition", "id": "EOC_mark", "effect": [{"u_message": "marked"}]}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, std::slice::from_ref(&mark));

    let runs = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let wrapper = format!(r#"{{"type": "effect_on_condition", "id": "EOC_runner", "effect": [{text}]}}"#);
        let value = cdda_json::parse(wrapper.as_bytes()).expect("json");
        let program = cdda_content::dialogue::read_program(&value).expect("the program reads");
        cdda_sim::eoc_registry::set_programs(world, &[program, mark.clone()]);
        cdda_sim::eoc_registry::run_program(world, survivor, None, "EOC_runner", 0)
    };

    // Only the legs, and only the one close enough.
    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        runs(
            &mut world,
            r#"{"u_run_monster_eocs": ["EOC_mark"], "mtype_ids": ["mon_quad_leg"], "monster_range": 2}"#
        ),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world).len(), 1, "one leg is within two squares");

    // Without a type filter, every monster on the level is marked.
    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        runs(&mut world, r#"{"u_run_monster_eocs": ["EOC_mark"]}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world).len(), 3, "three monsters share the level");
}

/// A parenthesised expression is a valid argument, so `max((ps_str/2), 1)` reads.
///
/// The spawn data writes exactly that as a count, and four programs refused on it: the count is fine, the expression inside it was not reading.
#[test]
fn a_parenthesised_argument_reads() {
    for text in [
        "max((ps_str/2), 1)",
        "1 + rand(4)",
        "rand(2) + 1",
        "max(ps_str/2, 1)",
        "(ps_str/2)",
    ] {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_condition(&condition).is_some(),
            "{text} reads"
        );
    }
}

/// A spawn that names no monster copies one that is already there.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7433-7465`): with an empty member, every hostile creature within `target_range` is a candidate, the player's own
/// square is excluded, and one is drawn - which is how the data makes a hallucination that looks like something the character can already see.
#[test]
fn a_spawn_that_names_nothing_copies_a_nearby_monster() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_shadow", hp: 30 }],
    );
    let shadow = cdda_sim::monster_registry::id_of(&mut world, "mon_shadow").expect("a kind");
    let kinds = cdda_content::monsters::load_objects(&[cdda_json::parse(
        br#"{"id": "mon_shadow", "name": "a shadow", "hp": 30, "speed": 100, "symbol": "s", "color": "black"}"#
            .as_slice(),
    )
    .expect("json")])
    .expect("readable");
    cdda_sim::monster_catalog::set_types(&mut world, kinds);
    let existing = cdda_sim::creature::spawn_creature(&mut world, "a shadow", (4, 0, 0));
    cdda_sim::creature::set_kind(&mut world, existing, shadow);

    let spawn = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_spawn_monster": "", "hallucination_count": 2, "target_range": 10, "max_radius": 4}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("an empty monster with a target range reads");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &spawn),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let mut made = 0;
    let mut query = world
        .try_query::<&cdda_sim::creature::CreatureKind>()
        .expect("query");
    for _ in query.iter(&world) {
        made += 1;
    }
    assert_eq!(made, 3, "the shadow and two copies of it");

    // Nothing to copy is not a spawn: the reference runs the effect's `false_eocs`, which the port cannot read, so it refuses.
    let lonely = cdda_sim::creature::spawn_creature(&mut world, "someone else", (30, 30, 0));
    let none = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_spawn_monster": "", "hallucination_count": 1, "target_range": 2}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, lonely, None, &none),
        cdda_sim::dialogue_effects::Applied::CannotAnswer
    );
}

/// An effect entry that holds only a comment does nothing, rather than refusing the program that carries it.
///
/// Reference: a member named `//` - or any name starting with `//` - is a comment no loader reads (`json.h:1500`, `json.cpp:153`), so an entry with nothing else
/// loads as a function with nothing to do. The data uses exactly that to disable an entry by hand.
#[test]
fn a_comment_only_entry_does_nothing() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let commented = cdda_json::parse(
        br#"{"u_message": "said first", "//": "this one still says its line"}"#.as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_effect(&commented).expect("a comment beside a real effect is not a refusal");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &read),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world), vec!["said first".to_owned()]);

    // An entry with nothing but comments is a no-op, and the program around it still runs.
    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_commented", "effect": [{"//": "disabled for now"}, {"u_message": "after"}]}"#
            .as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_program(&program).expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, std::slice::from_ref(&read));
    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        cdda_sim::eoc_registry::run_program(&mut world, survivor, None, "EOC_commented", 0),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world), vec!["after".to_owned()]);
}

/// A variable or a `math` block written as an **object** reads as an expression.
///
/// Reference: `get_dbl_or_var` (`npctalk.cpp`) takes a plain number, a `{"math": [...]}` block, or a variable object such as `{"global_val": "name"}` wherever an
/// expression is expected - and the data writes all three in the same slot.
#[test]
fn an_expression_can_be_written_as_an_object() {
    for text in [
        r#"{"math": ["rng(1, 1)"]}"#,
        r#"{"global_val": "exodii_mission_wh_chance"}"#,
        r#"{"context_val": "chance"}"#,
        r#"{"u_val": "vitri_vitrified"}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_expression(&value).is_some(),
            "{text} reads as an expression"
        );
    }

    // The variable object is the variable, not a call: a global read answers what the world put there.
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "chance",
        cdda_sim::dialogue_values::Value::Number(0.25),
    );
    let expression = cdda_content::dialogue::read_expression(
        &cdda_json::parse(br#"{"global_val": "chance"}"#.as_slice()).expect("json"),
    )
    .expect("reads");
    assert_eq!(
        cdda_sim::conditions::value(&world, survivor, None, &expression),
        Some(0.25),
        "the global's number, read through an object"
    );
}

/// `u_characters_nearby()` counts the other people within a radius.
///
/// Reference: `_characters_nearby_eval` (`math_parser_diag.cpp:688-734`): the characters within `radius` (default **1000**), filtered by `attitude` (default `any`).
/// The port has no attitude between characters - no faction, no ally - so `any` is the only filter it can answer and the rest must refuse.
#[test]
fn people_are_counted_within_a_radius() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    for (name, square) in [("the shopkeeper", (3, 4, 0)), ("the hermit", (40, 0, 0))] {
        let person = cdda_sim::creature::spawn_creature(&mut world, name, square);
        cdda_sim::npcs::make_npc(&mut world, person, "ID");
    }

    let asks = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    assert!(asks(&world, "u_characters_nearby() == 2"), "both are inside the default radius");
    assert!(asks(&world, "u_characters_nearby('radius': 5) == 1"), "only the shopkeeper is close");
    assert!(asks(&world, "u_characters_nearby('attitude': 'any') == 2"), "`any` is the filter the port can answer");

    // An attitude the port cannot judge refuses rather than counting everyone.
    let allies =
        cdda_json::parse(br#"{"math": ["u_characters_nearby('attitude': 'allies') > 0"]}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_condition(&allies).expect("reads");
    assert!(!cdda_sim::conditions::holds_with(&world, survivor, None, &read));
}

/// `u_vitamin()` answers how much of one vitamin the body holds, and zero for one it has none of.
/// `u_vitamin()` answers how much of one vitamin the body holds, and zero for one it has none of.
///
/// Reference: `vitamin_eval` (`math_parser_diag.cpp:1584-1592`) answers `vitamin_get`, which is the stored amount or **zero** when the vitamin is not in the body
/// (`consumption.cpp:715-723`). Zero is a real answer here rather than a refusal: "none of it" is exactly what the data asks about.
#[test]
fn a_vitamin_level_reads_as_the_amount_stored() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert!(cdda_sim::body::with_body_mut(&mut world, survivor, |body| {
        body.vitamins.insert("mutagen".to_owned(), 450);
    }));

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    assert!(holds(&world, "u_vitamin('mutagen') >= 450"), "the stored amount comes back");
    assert!(holds(&world, "u_vitamin('mutagen') < 451"), "and not one more");
    assert!(
        holds(&world, "u_vitamin('dermatik_larva_size') == 0"),
        "a vitamin the body has none of reads zero"
    );
    assert!(
        holds(&world, "max(360, 2000 - u_vitamin('mutagen')) == 1550"),
        "and it is a number inside an expression"
    );
}

/// `u_health()` answers the health **stat**, which is not the creature's hit points.
///
/// Reference: `health_eval` (`math_parser_diag.cpp:150-155`) answers `get_health()`, a number from `-200` to `200` that the reference keeps apart from `get_hp()`.
/// A character at full hit points can be at `-200` health; confusing the two would make a wounded character look well and a well one look dead.
#[test]
fn health_is_a_stat_and_not_hit_points() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let read = |text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        cdda_content::dialogue::read_condition(&condition).expect("the condition reads")
    };
    assert!(
        cdda_sim::conditions::holds_with(&world, survivor, None, &read("u_health() == 0")),
        "a fresh character is neither healthy nor ill"
    );
    assert!(cdda_sim::body::with_body_mut(&mut world, survivor, |body| body.health = -150));
    assert!(
        cdda_sim::conditions::holds_with(&world, survivor, None, &read("u_health() == -150")),
        "the stat is read as it was set, negative and all"
    );
    // The hit points did not move with it: these are two different numbers.
    let hit_points = cdda_sim::creature::health_of(&world, survivor);
    assert_ne!(hit_points, Some(-150), "hit points are not the health stat");
}

/// `u_calories()` answers the stored energy in kilocalories, and refuses the `percent` form it has no rule for.
///
/// Reference: `calories_eval` (`math_parser_diag.cpp:1753`) answers `get_stored_kcal()`, which is the port's `stored_calories` - thousandths of a kilocalorie -
/// divided by a thousand. `format: percent` needs the character's healthy weight, which the port has no rule for, so it is refused rather than answered with a
/// number that would be right for exactly one body size.
#[test]
fn calories_are_read_in_kilocalories() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert!(cdda_sim::body::with_body_mut(&mut world, survivor, |body| {
        body.stored_calories = 41_500_000;
    }));
    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };
    assert!(holds(&world, "u_calories() == 41500"), "forty-one and a half thousand kilocalories");

    // `percent` is refused: the port cannot say what percentage of a healthy weight that is, because it has no rule for healthy weight.
    let percent = cdda_json::parse(br#"{"math": ["u_calories('format': 'percent') > 10"]}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_condition(&percent).expect("the condition reads");
    assert!(!cdda_sim::conditions::holds_with(&world, survivor, None, &read), "no answer, so no pass");
}

/// `u_learn_recipe` teaches the actor one recipe, `u_forget_recipe` takes it back, and `u_know_recipe` asks.
///
/// Reference: `f_learn_recipe` (`npctalk.cpp:5250-5258`) hands the id to `Character::learn_recipe`, which adds it to `learned_recipes`
/// (`src/character_crafting.cpp:32-40`); `knows_recipe` (`:28-31`) is a membership test on that set. Teaching twice leaves one copy, and forgetting one the
/// character never knew changes nothing.
#[test]
fn a_recipe_is_taught_and_known() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert!(!cdda_sim::recipes_known::knows(&world, survivor, "cake2"), "nobody knows it yet");

    let runs = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let wrapper = format!(r#"{{"type": "effect_on_condition", "id": "EOC_teach", "effect": [{text}]}}"#);
        let value = cdda_json::parse(wrapper.as_bytes()).expect("json");
        let program = cdda_content::dialogue::read_program(&value).expect("the program reads");
        cdda_sim::eoc_registry::set_programs(world, std::slice::from_ref(&program));
        cdda_sim::eoc_registry::run_program(world, survivor, None, "EOC_teach", 0)
    };

    assert_eq!(
        runs(&mut world, r#"{"u_learn_recipe": "cake2"}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert!(cdda_sim::recipes_known::knows(&world, survivor, "cake2"), "the recipe is known");
    assert_eq!(
        cdda_sim::recipes_known::known(&world, survivor),
        vec!["cake2".to_owned()],
        "and it is the only one"
    );

    // Teaching the same recipe again leaves one copy: the set has no duplicates.
    assert_eq!(
        runs(&mut world, r#"{"u_learn_recipe": "cake2"}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::recipes_known::known(&world, survivor).len(), 1, "still one copy");

    assert_eq!(
        runs(&mut world, r#"{"u_forget_recipe": "cake2"}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert!(!cdda_sim::recipes_known::knows(&world, survivor, "cake2"), "and now it is gone");
}

/// `u_field_strength()` reads a field's intensity on the actor's square, and zero where there is none.
///
/// Reference: `field_strength_eval` (`math_parser_diag.cpp:330-355`): `fp ? fp->get_field_intensity() : 0` - an empty square answers zero rather than refusing,
/// because "none of it" is a real answer. `u_skill()` with the optional scope prefix is read here too: the data writes all three spellings.
#[test]
fn a_field_strength_and_a_skill_level_are_numbers() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -8..24 {
        for y in -8..24 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_web", 7, 3)]);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::skills_of::set_level(&mut world, survivor, "computer", 4);

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    assert!(holds(&world, "u_field_strength('fd_web') == 0"), "an empty square is zero webs");

    let web = cdda_sim::field_registry::identity_of(&world, "fd_web").expect("the field is installed");
    cdda_sim::add_field_strength(&mut world, (0, 0, 0), web, 2);
    assert!(holds(&world, "u_field_strength('fd_web') == 2"), "two levels of web");
    assert!(holds(&world, "u_field_strength('fd_web') < 3"), "and not three");

    // The data writes `u_skill` with the optional scope prefix; the reader has to accept the spelling and answer the level.
    assert!(holds(&world, "u_skill('computer') >= 4"), "the level it was set to");
    assert!(holds(&world, "u_skill('computer') < 5"), "and not one more");
    assert!(holds(&world, "skill('computer') == 4"), "the bare spelling is the same question");
}

/// `u_has_flag()` answers one or zero, and reads the flag name from a variable when the data writes one.
///
/// Reference: `has_flag_eval` (`math_parser_diag.cpp:1707-1717`) answers `actor->has_flag( flag )` as a number, because the data **adds** these together:
/// `u_has_flag(_flag1) + u_has_flag(_flag2) + u_has_flag(_flag3)` counts how many of three flags the character carries.
#[test]
fn flags_count_as_numbers_and_may_be_named_by_a_variable() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature_flags::add_flag(&mut world, survivor, "WIELDED_ITEM");

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    assert!(holds(&world, "u_has_flag('WIELDED_ITEM') == 1"), "a flag it carries");
    assert!(holds(&world, "u_has_flag('MUTATION_THRESHOLD') == 0"), "a flag it does not");
    assert!(
        holds(&world, "u_has_flag('WIELDED_ITEM') + u_has_flag('MUTATION_THRESHOLD') == 1"),
        "adding them counts how many it has"
    );

    // The data writes the flag through a variable, which is why the argument has to be evaluated rather than matched as text.
    let scope = cdda_sim::dialogue_values::holder_for(&mut world, survivor, false);
    cdda_sim::dialogue_values::set(
        &mut world,
        scope,
        "_flag1",
        cdda_sim::dialogue_values::Value::Text("WIELDED_ITEM".to_owned()),
    );
    assert!(holds(&world, "u_has_flag(_flag1) == 1"), "the same flag, named by a variable");
}

/// `u_hp_max()` answers the creature's hit-point ceiling, which is what a health bar is drawn against.
///
/// Reference: `hp_max_eval` (`math_parser_diag.cpp:1714-1719`) answers `get_hp_max( bodypart )`. The port has no per-part hit points, so it answers the whole
/// creature's ceiling - the same line `u_hp` draws - and the test pins that the ceiling does not follow the current hit points down.
#[test]
fn the_hit_point_ceiling_is_a_separate_number() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature::set_health(&mut world, survivor, 40);
    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };
    assert!(holds(&world, "u_hp_max() == 40"), "the ceiling it was given");

    // Losing hit points does not lower the ceiling: `hp` and `hp_max` are two numbers.
    if let Some(mut health) = world.get_mut::<cdda_sim::creature::Health>(survivor) {
        health.hp = 12;
    }
    assert!(holds(&world, "u_hp() == 12"), "what is left");
    assert!(holds(&world, "u_hp_max() == 40"), "and what it started with");
    assert!(holds(&world, "(u_hp() * 1.4) < u_hp_max()"), "the comparison the data writes");
}

/// An addiction has a level and a sated time, and both read as zero for one the character does not have.
///
/// Reference: `class addiction` (`src/addiction.h:58-80`) carries `intensity` and `sated`; `addiction_level` (`suffer.cpp:2118-2125`) searches for the entry and
/// answers **zero** when there is none, and `get_addiction_turns` (`talker_character.cpp:1011-1019`) answers the entry's `sated` or zero. The data relies on that
/// zero: `rand(6) < u_addiction_intensity('mutagen')` is written with no guard.
#[test]
fn an_addiction_reads_its_level_and_its_sated_time() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    assert!(holds(&world, "u_addiction_intensity('mutagen') == 0"), "no addiction reads zero");
    assert!(holds(&world, "u_addiction_turns('mutagen') == 0"), "and no sated time either");

    cdda_sim::addictions::set_intensity(&mut world, survivor, "mutagen", 9);
    assert!(holds(&world, "u_addiction_intensity('mutagen') == 9"), "the level it was given");
    assert!(holds(&world, "u_addiction_intensity('sleeping pill') == 0"), "and another is still zero");
    assert!(
        holds(&world, "u_addiction_turns('mutagen') == 600"),
        "a new addiction starts sated for one hour, as the reference's default does"
    );

    // The level is clamped to the reference's ceiling rather than refused.
    cdda_sim::addictions::set_intensity(&mut world, survivor, "mutagen", 99);
    assert!(
        holds(&world, "u_addiction_intensity('mutagen') == 20"),
        "MAX_ADDICTION_LEVEL is twenty"
    );
    cdda_sim::addictions::set_turns(&mut world, survivor, "mutagen", 30);
    assert!(holds(&world, "u_addiction_turns('mutagen') == 30"), "and the sated time can be set");
}

/// A function the **data** defined is called from a condition, with its body evaluated and its parameters bound.
///
/// Reference: `type: "jmath_function"` (`math_parser_jmath.cpp`): the data writes a body over `_0`, `_1`, ... and the reference's math parser calls it from the same
/// table as its compiled functions. `addiction_rational` is the one the addiction programs use:
/// `"return": "(_0 / max(1, (_0 - (_1 * _2)))) - 1"`.
#[test]
fn a_data_defined_function_is_called_from_a_condition() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let installed = cdda_sim::math_functions::install_json(
        &mut world,
        br#"[{"type": "jmath_function", "id": "addiction_rational", "num_args": 3, "return": "(_0 / max(1, (_0 - (_1 * _2)))) - 1"}]"#,
    )
    .expect("the data reads");
    assert_eq!(installed, 1, "one function installed");

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let condition = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let read = cdda_content::dialogue::read_condition(&condition).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &read)
    };

    // intensity 0: (2000 / max(1, 2000 - 0)) - 1 = 0. So `rand(2000) <= 0` is true only for a roll of zero.
    assert!(holds(&world, "addiction_rational(2000, 20, 0) == 0"), "no addiction is a zero chance");
    // intensity 10: (2000 / max(1, 2000 - 200)) - 1 = 1/9, a number the port computes rather than guesses.
    assert!(
        holds(&world, "addiction_rational(2000, 20, 10) > 0.11") && holds(&world, "addiction_rational(2000, 20, 10) < 0.12"),
        "a tenth of the way in is about a ninth"
    );

    // A name no table knows still refuses rather than answering zero.
    let unknown = cdda_json::parse(br#"{"math": ["no_such_function(1) > 0"]}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_condition(&unknown).expect("the expression reads");
    assert!(!cdda_sim::conditions::holds_with(&world, survivor, None, &read), "no answer, so no pass");
}

/// A `math` statement can write to a **store** it can read, not only to a variable.
///
/// Reference: the math parser registers a setter beside every getter it has (`vitamin_ass`, `hp_ass`, `skill_ass` in `math_parser_diag.cpp`), which is why the
/// data writes `"u_vitamin('x') = 5"` and `"u_hp('torso') -= 3"`. The port's stores are components, so the write resolves the same way the read does.
#[test]
fn a_math_statement_writes_to_a_store() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature::set_health(&mut world, survivor, 20);

    let run = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(format!(r#"{{"math": ["{text}"]}}"#).as_bytes()).expect("json");
        let Some(cdda_content::dialogue::Effect::Math(statements)) = cdda_content::dialogue::read_effect(&value) else {
            return cdda_sim::dialogue_effects::Applied::CannotAnswer;
        };
        cdda_sim::dialogue_effects::run_math(world, survivor, None, &statements)
    };

    assert_eq!(run(&mut world, "u_vitamin('mutagen') = 5"), cdda_sim::dialogue_effects::Applied::Done);
    assert_eq!(cdda_sim::body::vitamin_of(&world, survivor, "mutagen"), Some(5), "the vitamin store took the write");

    assert_eq!(run(&mut world, "u_hp('torso') -= 3"), cdda_sim::dialogue_effects::Applied::Done);
    assert_eq!(cdda_sim::creature::health_of(&world, survivor), Some(17), "hit points subtract from what was left");

    assert_eq!(run(&mut world, "u_skill('fabrication') = 4"), cdda_sim::dialogue_effects::Applied::Done);
    assert_eq!(cdda_sim::skills_of::level_of(&world, survivor, "fabrication"), 4, "the skill store took the write");

    // A name with no store behind it has nowhere to write, and says so rather than creating a variable by accident.
    assert_eq!(
        run(&mut world, "u_temperature('body') = 1"),
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "a name with no setter refuses"
    );
}

/// `npc_has_effect` asks about the **other** creature, and the data asks it 263 times.
///
/// Reference: `f_has_effect` (`condition.cpp:980-996`) takes its actor from the dialogue's scope, and the data spells the two scopes `u_has_effect` and
/// `npc_has_effect`. A reader that knows only the first refuses a quarter of the data's effect guards rather than answering them about the wrong creature.
#[test]
fn an_effect_condition_can_ask_about_the_other_creature() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (1, 0, 0));
    cdda_sim::applied_effects::add(
        &mut world,
        talker,
        cdda_sim::effects::ActiveEffect {
            id: "effect_boom".to_owned(),
            intensity: 2,
            duration: 10,
            permanent: false,
        },
    );

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, Some(talker), &condition)
    };

    assert!(!holds(&world, r#"{"u_has_effect": "effect_boom"}"#), "the survivor does not wear it");
    assert!(holds(&world, r#"{"npc_has_effect": "effect_boom"}"#), "the talker does");
    assert!(
        holds(&world, r#"{"npc_has_effect": "effect_boom", "intensity": 2}"#),
        "at the intensity it was given"
    );
    assert!(
        !holds(&world, r#"{"npc_has_effect": "effect_boom", "intensity": 3}"#),
        "and not at one it was not"
    );
    // With nobody to ask about, a question about the other creature is refused rather than answered about the speaker.
    let alone_value = cdda_json::parse(br#"{"npc_has_effect": "effect_boom"}"#.as_slice()).expect("json");
    let alone_condition = cdda_content::dialogue::read_condition(&alone_value).expect("the condition reads");
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, None, &alone_condition),
        "no listener, no answer"
    );
}

/// The data's short listener spelling answers about the other creature, as the long one does.
///
/// Reference: `math_parser` gives a two-scope function an `u_` and an `n_` spelling, and the data uses `n_skill` **141 times** - more than either long spelling.
/// A port that knows only `npc_skill` refuses the majority of the data's skill questions rather than answering them about the wrong creature.
#[test]
fn the_short_listener_spelling_answers_about_the_other_creature() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (1, 0, 0));
    cdda_sim::skills_of::set_level(&mut world, talker, "fabrication", 6);

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, Some(talker), &condition)
    };

    assert!(holds(&world, r#"{"math": ["n_skill('fabrication') == 6"]}"#), "the short spelling asks about the talker");
    assert!(holds(&world, r#"{"math": ["n_skill('fabrication') > u_skill('fabrication')"]}"#), "and not about the survivor");
    assert!(holds(&world, r#"{"math": ["u_skill('fabrication') == 0"]}"#), "the survivor still knows nothing");
}

/// `weather('temperature')` answers the precise weather, in kelvin.
///
/// Reference: `weather_eval` (`math_parser_diag.cpp:1497-1527`) answers aspects of `get_weather().weather_precise`, and `temperature` is returned in kelvin by
/// `units::to_kelvin`. The port's climate keeps that point in its manager, which is why the reader asks the climate rather than a copy.
#[test]
fn the_weather_answers_its_precise_numbers() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let holder = cdda_sim::singletons::singleton::<cdda_sim::weather::Climate, cdda_sim::singletons::TheClimate>(&mut world);
    let mut climate = world.get_mut::<cdda_sim::weather::Climate>(holder).expect("the climate");
    climate.set_point_for_reading(cdda_content::weather::Point {
        turn: 0,
        temperature: 288.15,
        humidity: 40.0,
        pressure: 1013.0,
        windpower: 7,
        ..cdda_content::weather::Point::default()
    });

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &condition)
    };

    assert!(holds(&world, r#"{"math": ["weather('temperature') == 288.15"]}"#), "15 celsius, in kelvin");
    assert!(holds(&world, r#"{"math": ["weather('humidity') == 40"]}"#), "and the humidity");
    assert!(holds(&world, r#"{"math": ["weather('windpower') == 7"]}"#), "and the wind");
    // An aspect the port has no rule for refuses rather than answering zero.
    assert!(!holds(&world, r#"{"math": ["weather('precipitation') == 0"]}"#), "no rule, no answer");
}

/// A `math` statement split across array elements reads, and a postfix step on a variable reads too.
///
/// Reference: `eoc_math::from_json` (`condition.cpp:2494-2502`) concatenates the array's strings into one expression, so `["u_pain()", "-=", "5"]` is the
/// statement `u_pain() -= 5`. The data writes both that split form and the postfix `["u_val('focus')--"]`, and both must read.
#[test]
fn a_split_math_statement_reads() {
    for text in [
        r#"{"math": ["u_pain()", "-=", "5"]}"#,
        r#"{"math": ["u_val('focus')--"]}"#,
        r#"{"math": ["Nemesis_iteration", "+=", "1"]}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_effect(&value).is_some(),
            "{text} must read as one statement"
        );
    }
}

/// A bare `is_day` / `is_night` is a condition, and the two are not each other's negation.
///
/// Reference: `conditional_t` (`condition.cpp:2783-2810`) looks a bare name up among the argument-less parsers, and the data writes `{"not": "is_day"}`. The rules are
/// `calendar.cpp:363-371`: day starts at `sunrise_angle` (-1 degree) and night starts at `civil_dawn` (-6 degrees), so the sky between them calls itself neither.
#[test]
fn a_bare_day_or_night_condition_reads() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, None, &condition)
    };

    // Noon on the first day is certainly day, and midnight is certainly night.
    // One turn is one second, so noon of the first day is 43,200 turns in and midnight is zero.
    let noon = 43_200;
    let midnight = 0;
    cdda_sim::calendar::set_turn(&mut world, noon);
    assert!(holds(&world, r#""is_day""#), "noon is day");
    assert!(!holds(&world, r#"{"not": "is_day"}"#), "and not not-day");
    cdda_sim::calendar::set_turn(&mut world, midnight);
    assert!(holds(&world, r#""is_night""#), "midnight is night");
    assert!(holds(&world, r#"{"not": "is_day"}"#), "midnight is not day");
}

/// A spell's level and experience read from the creature's own store, and a spell it does not know is a level of zero.
///
/// Reference: `spell_level_eval` / `spell_exp_eval` (`math_parser_diag.cpp:1036-1070`) answer `get_spell_level` and `get_spell_exp`, which are zero for a spell the
/// character has not learned - the data compares `u_spell_level('x') >= 3` with no guard, so "not learned" has to be a number.
#[test]
fn a_spell_level_reads_from_the_creatures_own_store() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (1, 0, 0));
    cdda_sim::known_spells::set_level(&mut world, talker, "spell_light", 4);
    cdda_sim::known_spells::set_experience(&mut world, talker, "spell_light", 275);

    let holds = |world: &bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");
        cdda_sim::conditions::holds_with(world, survivor, Some(talker), &condition)
    };

    assert!(holds(&world, r#"{"math": ["n_spell_level('spell_light') == 4"]}"#), "the talker knows it to four");
    assert!(holds(&world, r#"{"math": ["u_spell_level('spell_light') == 0"]}"#), "the survivor has not learned it");
    assert!(holds(&world, r#"{"math": ["n_spell_exp('spell_light') == 275"]}"#), "and its experience");
    // A step writes back to the same store: `u_spell_level('x') += 1` is how the data levels a spell.
    let value = cdda_json::parse(br#"{"math": ["spell_level('spell_light') += 1"]}"#.as_slice()).expect("json");
    let Some(cdda_content::dialogue::Effect::Math(statements)) = cdda_content::dialogue::read_effect(&value) else {
        return;
    };
    assert_eq!(
        cdda_sim::dialogue_effects::run_math(&mut world, talker, None, &statements),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::known_spells::level_of(&world, talker, "spell_light"), 5, "the store took the step");
}

/// A sound's kind is read from all twelve names the reference accepts, not the four the port knew.
///
/// Reference: `f_make_sound` (`npctalk.cpp:6145-6171`) maps `type` onto `sounds::sound_t`, and the data uses `activity` and `destructive_activity` as well as
/// `speech`, `alarm` and `order`. A name outside the list is an error in the reference, and the port refuses it for the same reason.
#[test]
fn a_sound_kind_reads_in_all_its_names() {
    let reads = |kind: &str| {
        let text = format!(r#"{{"u_make_sound": "a bang", "volume": 10, "type": "{kind}"}}"#);
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        cdda_content::dialogue::read_effect(&value).is_some()
    };
    for kind in [
        "background",
        "weather",
        "music",
        "movement",
        "speech",
        "electronic_speech",
        "activity",
        "destructive_activity",
        "alarm",
        "combat",
        "alert",
        "order",
    ] {
        assert!(reads(kind), "{kind} is a sound kind the reference knows");
    }
    assert!(!reads("a_kind_nobody_has"), "and a name outside the list is refused");
}

/// `u_spawn_npc` brings NPCs into being within the radii, with the traits the data lists.
///
/// Reference: `f_spawn_npc` (`npctalk.cpp:7575-7620`): a class, `real_count` and `hallucination_count`, `min_radius`/`max_radius` defaulting to one and ten, and a
/// `traits` array each one is given. The port's read is the effect; its run is a creature in the store with `Npc` on it.
#[test]
fn a_spawn_npc_effect_makes_npcs_with_their_traits() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // A floor for the spawner to put something on: the reference refuses a square with no map, and so does the port.
    for x in -12..=12 {
        for y in -12..=12 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), 1);
        }
    }

    let value = cdda_json::parse(
        br#"{"u_spawn_npc": "old_guard", "real_count": 2, "min_radius": 1, "max_radius": 3, "traits": ["GOODMEMORY"]}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let dice = cdda_core_types::rng::Rolls::new(0);
    cdda_sim::singletons::put_rolls(&mut world, dice);
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the spawn runs");

    let made = cdda_sim::npcs::npcs(&world);
    assert_eq!(made.len(), 2, "two of them, as `real_count` asked");
    for (entity, at) in made {
        assert!(cdda_sim::traits::has_trait(&world, entity, "GOODMEMORY"), "each is given the trait");
        let distance = (at.0.abs()).max(at.1.abs());
        assert!((1..=3).contains(&distance), "each is between the radii, not on top of the speaker");
    }
}

/// A sound effect reads from all the members the reference reads, and running it changes no game state.
///
/// Reference: `f_sound_effect` (`npctalk.cpp:5531-5560`): the member is the variant, `id` the sound, `volume` a loudness with `-1` meaning eighty, and
/// `outdoor_event` a flag. Only the player hears it, so nothing in the world moves.
#[test]
fn a_sound_effect_reads_and_changes_nothing() {
    let world_before = |text: &str| {
        let mut world = bevy_ecs::prelude::World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("the sound effect reads");
        let before = world.entities().len();
        let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
        (applied, world.entities().len() - before)
    };
    let (applied, entities) = world_before(
        r#"{"sound_effect": "elec_crackle", "id": "bionics", "volume": 100}"#,
    );
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    assert_eq!(entities, 0, "and makes nothing: not one entity more");

    // The reference's own default: no `volume` reads as `-1`, which it plays as eighty.
    let (applied, _) = world_before(r#"{"sound_effect": "elec_crackle", "id": "bionics"}"#);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "and reads with no volume");
}

/// `u_spawn_item` reads its count the way the reference does: **1** by default, **0** when the entry carries `charges`.
///
/// Reference: `f_spawn_item` (`npctalk.cpp:3783-3787`): `count` is `get_dbl_or_var( jo, "count", false, 1 )`, except when the entry has an integer `charges`, where
/// the same call passes a default of **0** - because a stack's size is then its charges. The second case is the one a careless read of the reference gets wrong,
/// so both are asserted here. Running it needs an item catalogue, which this empty world does not have, and the refusal is asserted too.
#[test]
fn a_spawn_item_effect_reads_its_count_the_reference_way() {
    let read = |text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        cdda_content::dialogue::read_effect(&value).expect("the effect reads")
    };
    let plain = format!("{:?}", read(r#"{"u_spawn_item": "chitin_piece"}"#));
    assert!(plain.contains("One(1)"), "a count nobody wrote is one: {plain}");
    let charged = format!("{:?}", read(r#"{"u_spawn_item": "chitin_piece", "charges": 3}"#));
    assert!(charged.contains("One(0)"), "and zero beside charges: {charged}");
    let counted = format!("{:?}", read(r#"{"u_spawn_item": "chitin_piece", "count": 5}"#));
    assert!(counted.contains("One(5)"), "five when the data says five: {counted}");

    // A world with no item catalogue cannot place a named item, and says so rather than dropping something nameless.
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &read(r#"{"u_spawn_item": "chitin_piece", "count": 5}"#));
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::CannotAnswer, "no catalogue, no item");
}

/// `u_consume_item` takes whole stacks out of what a creature carries, and refuses to take what is not there.
///
/// Reference: `f_consume_item` (`npctalk.cpp:3943-3968`): a count of stacks is taken with `use_amount`, charges with `use_charges`, and a creature that has neither
/// is told so rather than half-emptied. The port's `inventory::take` refuses a partial removal for the same reason.
#[test]
fn a_consume_item_effect_takes_whole_stacks() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "chitin_piece",
            volume_ml: 250,
            weight_g: 100,
            pockets: &[],
        }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let kind = cdda_sim::item_registry::id_of(&mut world, "chitin_piece").expect("the catalogue knows it");
    for _ in 0..3 {
        let item = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
        world.entity_mut(item).insert(cdda_sim::creature::Carried(survivor));
    }

    let value = cdda_json::parse(br#"{"u_consume_item": "chitin_piece", "count": 2}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    assert_eq!(cdda_sim::inventory::count_of(&world, survivor, "chitin_piece").0, 1, "two of three are gone");

    // Asking for more than is carried takes nothing at all, rather than half of what it asked for.
    let value = cdda_json::parse(br#"{"u_consume_item": "chitin_piece", "count": 5}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the effect still completes");
    assert_eq!(cdda_sim::inventory::count_of(&world, survivor, "chitin_piece").0, 1, "and the last one is still there");
}

/// `set_string_var` writes its text to `target_var`, and draws from a list when the data gives one.
///
/// Reference: `f_set_string_var` (`npctalk.cpp:5809-5866`): the member is a string, a **list to draw one from**, or a variable to copy; `target_var` names where the
/// text goes. Both forms are checked, because a reader that only handled a plain string would pass on the first and fail on the second.
#[test]
fn a_set_string_var_writes_its_text() {
    let written = |text: &str| {
        let mut world = bevy_ecs::prelude::World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
        // The world's own dialogue store: a global variable needs a holder, and the holder is an entity with a marker on it.
        let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
        let _ = holder;
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
        let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
        (applied, world, survivor)
    };
    let (applied, world, survivor) = written(
        r#"{"set_string_var": "calming_stone", "target_var": {"global_val": "item_type"}}"#,
    );
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    let held = cdda_sim::conditions::holder_for_scope(&world, survivor, None, "global_val")
        .and_then(|holder| cdda_sim::dialogue_values::text_of(&world, holder, "item_type"));
    assert_eq!(held.as_deref(), Some("calming_stone"), "and the world remembers what it was told");

    let (applied, world, survivor) = written(
        r#"{"set_string_var": ["unstable_rift", "calming_stone", "escape_stone"], "target_var": {"global_val": "item_type"}}"#,
    );
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the list form runs too");
    let stored = cdda_sim::conditions::holder_for_scope(&world, survivor, None, "global_val")
        .and_then(|holder| cdda_sim::dialogue_values::text_of(&world, holder, "item_type"));
    assert!(
        ["unstable_rift", "calming_stone", "escape_stone"].contains(&stored.as_deref().unwrap_or("")),
        "and it is one of the three: {stored:?}"
    );
}

/// `copy_var` moves a value from one variable to another, whichever scopes the two ends name.
///
/// Reference: `f_copy_var` (`npctalk.cpp:5797-5807`): both ends are `var_info`s and the value goes across untouched. The copy is checked through the store, and a
/// source that holds nothing copies nothing rather than writing an empty text.
#[test]
fn a_copy_var_moves_a_value_across() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    cdda_sim::dialogue_values::set(&mut world, globals, "from_here", cdda_sim::dialogue_values::Value::Number(7.0));

    let value = cdda_json::parse(
        br#"{"copy_var": {"global_val": "from_here"}, "target_var": {"u_val": "to_there"}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    let copied = cdda_sim::dialogue_values::value_of(&world, survivor, "to_there");
    assert_eq!(copied, Some(cdda_sim::dialogue_values::Value::Number(7.0)), "the value crossed");

    // A source that holds nothing copies an absence, and says so by holding nothing at the far end too.
    let value = cdda_json::parse(
        br#"{"copy_var": {"global_val": "never_set"}, "target_var": {"u_val": "still_empty"}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "and it is still a completed effect");
    assert_eq!(cdda_sim::dialogue_values::value_of(&world, survivor, "still_empty"), None, "with nothing written");
}

/// `alter_timed_events` schedules a named event, and the store answers "has it happened" from the clock.
///
/// Reference: `f_alter_timed_events` (`npctalk.cpp:4873-4882`) writes `get_timed_events().set_all( key, now + duration )`, and the dialogue condition built on it
/// compares that turn against the calendar. The port keeps the events in one component on one entity, so the answer is a comparison, not a bit somebody flipped.
#[test]
fn an_altered_timed_event_comes_due_with_the_clock() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // The world's clock has to exist before an event can be scheduled against it, which is the store's own rule: no clock, no answer.
    cdda_sim::calendar::set_turn(&mut world, 1000);

    let read_and_run = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
        cdda_sim::dialogue_effects::run_effect(world, survivor, None, &effect)
    };

    // An hour ahead: not due yet, due an hour later.
    let applied = read_and_run(&mut world, r#"{"alter_timed_events": "portal_storm", "time_in_future": "1 hour"}"#);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    assert_eq!(cdda_sim::timed_events::has_happened(&world, "portal_storm"), Some(false), "not yet: an hour is ahead");
    // An hour is **3600 turns**, because a turn is a second in the reference's clock - the number the store was showing me when I first read it as 600 and
    // blamed the calendar. Half an hour on, still ahead; a whole one, due.
    cdda_sim::calendar::set_turn(&mut world, 1000 + 1800);
    assert_eq!(cdda_sim::timed_events::has_happened(&world, "portal_storm"), Some(false), "half an hour: still ahead");
    cdda_sim::calendar::set_turn(&mut world, 1000 + 3600);
    assert_eq!(cdda_sim::timed_events::has_happened(&world, "portal_storm"), Some(true), "an hour on, it has happened");

    // No time ahead at all: due this very turn, which is the reference's zero default.
    let applied = read_and_run(&mut world, r#"{"alter_timed_events": "now_event"}"#);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "and the zero-duration form runs");
    assert_eq!(cdda_sim::timed_events::has_happened(&world, "now_event"), Some(true), "due at once");
    assert_eq!(
        cdda_sim::timed_events::has_happened(&world, "nobody_scheduled_this"),
        Some(false),
        "an event nobody scheduled has not happened"
    );
}

/// A spawn that asks for a hand rather than a square reads, and refuses to run while the port has no hand to put it in.
///
/// Reference: `f_spawn_item` (`npctalk.cpp:3799`) reads `force_equip` and wields what it spawned. The port reads the flag and refuses at run rather than dropping the
/// item on the floor where the data did not ask for it - a wrong world is worse than a stated gap.
#[test]
fn a_forced_equip_reads_and_refuses_to_run() {
    let value = cdda_json::parse(
        br#"{"u_spawn_item": "chitin_piece", "force_equip": true}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert!(format!("{effect:?}").contains("force_equip: true"), "and keeps the flag: {effect:?}");

    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::CannotAnswer, "no hand, no equip");
}
