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
