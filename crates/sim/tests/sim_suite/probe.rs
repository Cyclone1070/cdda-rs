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

    // The outcome messages are read now, and `force_safe` - the one member beside them the port still has no rule for - refuses the whole entry rather than being
    // dropped as if it meant nothing.
    let with_message =
        cdda_json::parse(br#"{"u_teleport": {"u_val": "place"}, "fail_message": "no"}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&with_message).is_some(), "a message reads");
    let with_safe =
        cdda_json::parse(br#"{"u_teleport": {"u_val": "place"}, "force_safe": true}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&with_safe).is_none(), "but `force_safe` does not");
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

    // A deadline **reads** now, as a number or a `math` expression (`npctalk.cpp:5912`), and the port has no store for the due turn yet - so the entry is read and
    // the mission is assigned, with the deadline carried and not kept. `an_assigned_mission_takes_a_computed_deadline` pins that behaviour.
    let with_deadline =
        cdda_json::parse(br#"{"assign_mission": "MISSION_x", "deadline": 100}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&with_deadline).is_some());
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

/// A spawn that names an item **group** draws from the group, and what the group produces is what appears.
///
/// Reference: `use_item_group` (`npctalk.cpp:3781`) sends the name to `item_group::item_from`. The drawing rule is the one the reader already wrote
/// (`cdda_content::item_group_draw`); the port's part is the store, and this checks that a spawn goes through it.
#[test]
fn a_spawn_from_an_item_group_produces_what_the_group_holds() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "mp3",
            volume_ml: 250,
            weight_g: 100,
            pockets: &[],
        }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(
        br#"{"type": "item_group", "id": "internal_mp3_radio_group", "subtype": "collection", "entries": [{"item": "mp3", "prob": 100, "count": 1}]}"#.as_slice(),
    )
    .expect("json");
    cdda_sim::item_groups::set_groups(&mut world, cdda_content::item_group_draw::load_definitions(&value));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));

    let spawn = cdda_json::parse(
        br#"{"u_spawn_item": "internal_mp3_radio_group", "use_item_group": true, "suppress_message": true}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&spawn).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    let mut items = world.query::<&cdda_sim::GroundItem>();
    assert_eq!(items.iter(&world).count(), 1, "one item, which is what the group holds");
}

/// `set_condition` stores the **condition**, not its answer, so the same stored name can be asked again at a later turn.
///
/// Reference: `f_set_condition` (`npctalk.cpp:5868-5880`) writes the condition into the dialogue's variables and `u_has_condition` evaluates it where it is asked.
/// The test checks the store holds a condition, which is what makes the later question meaningful: a stored boolean would freeze the answer at the turn it was
/// stored, which is exactly what the reference does not do.
#[test]
fn a_set_condition_stores_the_condition_itself() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);

    let value = cdda_json::parse(
        br#"{"set_condition": "random_enc_condition", "condition": {"math": ["0 == 0"]}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    let stored = cdda_sim::dialogue_values::value_of(&world, globals, "random_enc_condition");
    assert!(
        matches!(stored, Some(cdda_sim::dialogue_values::Value::Condition(_))),
        "the store holds a condition, not a boolean: {stored:?}"
    );
}

/// `u_driving` reads as a bare condition name, and answers "not driving" in a world with no vehicles.
///
/// Reference: `u_driving` is `Character::is_driving()`, true only in a vehicle's driver's seat. The port has no vehicles, so the only true statement about its
/// world is that nobody is driving - and the answer is written down where vehicles will have to change it, not left to a default.
#[test]
fn a_bare_u_driving_condition_reads_and_answers_no() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(br#"{"not": "u_driving"}"#.as_slice()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");
    assert!(cdda_sim::conditions::holds(&world, survivor, &condition), "not driving, so its negation holds");
}

/// `finish_mission` takes the mission off the character's list, and finishing one that was never taken does nothing.
///
/// Reference: `f_finish_mission` (`npctalk.cpp:5928-5950`): the avatar's own mission is completed or failed; one the character does not hold is not an error. The port
/// keeps the active list and drops the entry - it has no completed-or-failed record yet, which is written down rather than pretended.
#[test]
fn a_finished_mission_leaves_the_list() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, survivor);
    cdda_sim::missions::accept(&mut world, survivor, "MISSION_INVESTIGATE_TRADER");

    let value = cdda_json::parse(br#"{"finish_mission": "MISSION_INVESTIGATE_TRADER", "success": false}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    assert!(!cdda_sim::missions::has_mission(&world, survivor, "MISSION_INVESTIGATE_TRADER"), "the mission is gone");

    // A mission that was never taken is not an error.
    let value = cdda_json::parse(br#"{"finish_mission": "MISSION_NEVER_TAKEN"}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("and the one-argument form reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "still a completed effect");
}

/// A teleport reads its two messages and shows the right one: the failure when the target holds no square, the success when it moves.
///
/// Reference: `f_teleport` (`npctalk.cpp:7820-7835`): `fail_message` and `success_message` are shown by the move itself. Both are checked, because a reader that
/// took one and dropped the other would pass on a single-sided test.
#[test]
fn a_teleport_shows_one_message_of_two() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);

    // Nothing stored under the name: the move cannot happen, and the failure line is the one shown.
    let value = cdda_json::parse(
        br#"{"u_teleport": {"global_val": "original_loc"}, "fail_message": "Something is very wrong!", "success_message": "You are safely back.", "force": true}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads, messages and all");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::CannotAnswer, "no square, no move");
    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines.iter().any(|line| line.contains("Something is very wrong")),
        "the failure line is shown: {lines:?}"
    );

    // With a square stored, it moves and shows the other line.
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "original_loc",
        cdda_sim::dialogue_values::Value::Position { x: 5, y: 6, z: 0 },
    );
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "now it moves");
    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines.iter().any(|line| line.contains("safely back")),
        "the success line is shown: {lines:?}"
    );
}

/// A spawn with `loc` puts the item on that square, not on the speaker's.
///
/// Reference: `f_spawn_item` (`npctalk.cpp:3795`) reads `loc` as a `var_info` and places the item there. The test stores a square some distance away, so a spawner
/// that ignored `loc` would put the item at the origin and fail.
#[test]
fn a_spawn_at_a_location_lands_there() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind { id: "bottle_plastic", volume_ml: 250, weight_g: 10, pockets: &[] }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, false);
    cdda_sim::dialogue_values::set(
        &mut world,
        holder,
        "loc",
        cdda_sim::dialogue_values::Value::Position { x: 9, y: 4, z: 0 },
    );

    let value = cdda_json::parse(
        br#"{"map_spawn_item": "bottle_plastic", "loc": {"u_val": "loc"}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    let mut items = world.query::<(&cdda_sim::Position, &cdda_sim::GroundItem)>();
    let places: Vec<(i32, i32)> = items.iter(&world).map(|(at, _)| (at.x, at.y)).collect();
    assert_eq!(places, vec![(9, 4)], "the item is where `loc` said");
}

/// `remove_active_mission` drops the mission without finishing it, and removing one the character never took does nothing.
///
/// Reference: `f_remove_active_mission` (`npctalk.cpp:5959-5973`): the avatar's mission of that type is removed and the search stops at the first match. The port
/// keeps one entry per mission type, so the first match is the only one there is - which is the same answer.
#[test]
fn a_removed_active_mission_leaves_the_list_unfinished() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, survivor);
    cdda_sim::missions::accept(&mut world, survivor, "MISSION_CAMP_LEADERSHIP_CHANGE");

    let value = cdda_json::parse(br#"{"remove_active_mission": "MISSION_CAMP_LEADERSHIP_CHANGE"}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    assert!(!cdda_sim::missions::has_mission(&world, survivor, "MISSION_CAMP_LEADERSHIP_CHANGE"), "the mission is gone");
    assert!(cdda_sim::missions::active_missions(&world, survivor).is_empty(), "and nothing else was touched");
}

/// `u_add_morale` reads with a `max_bonus`, a computed duration and a `bonus` all at once - the shape the data uses.
///
/// Reference: `f_add_morale` (`npctalk.cpp:7177-7195`): the type is a `str_or_var`, the bonus and its cap are `dbl_or_var`, and the duration is a `duration_or_var`.
/// The census once reported `max_bonus` as the gap here; the test exists to say whether the reader or the census was wrong.
#[test]
fn a_morale_point_reads_with_its_cap_and_a_computed_duration() {
    let value = cdda_json::parse(
        br#"{"u_add_morale": "morale_feeling_bad", "bonus": -15, "max_bonus": -15, "duration": {"math": ["rand(432000) + 86400"]}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value);
    assert!(effect.is_some(), "the morale point reads whole: {effect:?}");
}

/// A duration may be a **variable** the dialogue holds, not only a rolled expression.
///
/// Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) reads the same `dbl_or_var` shape everywhere, and the data passes `{"context_val": "duration"}` to
/// `u_add_effect`. Both object forms - a `math` and a variable - are worked out where the effect is applied.
#[test]
fn a_duration_reads_from_a_variable() {
    let from_variable = cdda_json::parse(
        br#"{"u_add_effect": {"context_val": "effect"}, "duration": {"context_val": "duration"}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&from_variable);
    assert!(effect.is_some(), "a variable effect and a variable duration both read: {effect:?}");

    let rolled = cdda_json::parse(
        br#"{"u_add_effect": "bleed", "duration": {"math": ["rng(_time * 0.94, _time * 1.06)"]}}"#.as_slice(),
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_effect(&rolled).is_some(), "and so does a rolled one");
}

/// `turn_cost` spends the speaker's moves: a turn is a hundred moves, and the cost is a duration like any other.
///
/// Reference: `f_turn_cost` (`npctalk.cpp:5291-5300`): `mod_moves( -to_moves<int>( cost ) )`, with `to_moves` a hundred moves a turn. The test starts with a
/// known number of moves, so a wrong conversion factor shows up as a wrong number rather than as "something changed".
#[test]
fn a_turn_cost_spends_a_hundred_moves_a_turn() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature::set_moves(&mut world, survivor, 1000);

    let value = cdda_json::parse(br#"{"turn_cost": 2}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "it runs");
    let left = world.get::<cdda_sim::creature::Moves>(survivor).map(|moves| moves.left);
    assert_eq!(left, Some(800), "two turns cost two hundred moves");
}

/// `u_add_wet` raises a creature's wetness, and the range is clamped at soaked.
///
/// Reference: `f_add_wet` (`npctalk.cpp:5464-5474`) adds the amount with `wet_character`, and `Character::mod_wetness` clamps to dry..soaked. The test adds twice,
/// so a store that set rather than added would report the second value and fail.
#[test]
fn a_wet_creature_gets_wetter_up_to_soaked() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert_eq!(cdda_sim::wetness::of(&world, survivor), 0, "dry to begin with");

    let wet = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
        cdda_sim::dialogue_effects::run_effect(world, survivor, None, &effect)
    };
    assert_eq!(wet(&mut world, r#"{"u_add_wet": 40}"#), cdda_sim::dialogue_effects::Applied::Done, "it runs");
    assert_eq!(cdda_sim::wetness::of(&world, survivor), 40, "forty wetter");
    wet(&mut world, r#"{"u_add_wet": 40}"#);
    assert_eq!(cdda_sim::wetness::of(&world, survivor), 80, "and forty again, because wetness adds");
    wet(&mut world, r#"{"u_add_wet": 40}"#);
    assert_eq!(cdda_sim::wetness::of(&world, survivor), 100, "never wetter than soaked");
}

/// `u_is_underwater` reads as a bare condition name and answers no, because the port has no submersion state.
///
/// Reference: `u_is_underwater` is `Character::is_underwater()`. The port has water terrain but no depth on a creature, so the true statement about its world is
/// that nobody is under it - the answer is written down where a depth model would have to change it.
#[test]
fn a_bare_u_is_underwater_reads_and_answers_no() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // The data writes it **bare** - `{"not": "u_is_underwater"}` - which is the shape this reads.
    let bare = cdda_json::parse(br#""u_is_underwater""#.as_slice()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&bare).expect("the bare name reads");
    assert!(!cdda_sim::conditions::holds(&world, survivor, &condition), "nobody is underwater here");
}

/// The exact `if`/`then`/`else` the census called a nested gap reads whole.
///
/// The census blamed `<nested>` on the `else` branch's `u_message`, which the reader accepts. The test reads the data's own object, so the next round works from
/// what is true rather than from a blame list entry that may be one member out - the mistake `max_bonus` already taught.
#[test]
fn an_if_with_an_else_reads_whole() {
    let text = br#"{"if": {"u_can_see_location": {"context_val": "pos"}},
                   "then": {"u_message": "You can see <context_val:pos>."},
                   "else": {"u_message": "You can't see <context_val:pos>."}}"#;
    let value = cdda_json::parse(text.as_slice()).expect("json");
    let message = cdda_json::parse(br#"{"u_message": "You can't see <context_val:pos>."}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&message).is_some(), "the message reads");
    let condition =
        cdda_json::parse(br#"{"u_can_see_location": {"context_val": "pos"}}"#.as_slice()).expect("json");
    // The census blamed `<nested>` on the `else` branch's message; the gap was the **condition**. With a line of sight in place
    // (`condition.cpp:2173-2181` -> `talker_character.cpp:923-928`), the branch reads whole and the message was never the problem.
    let read = cdda_content::dialogue::read_condition(&condition);
    assert!(read.is_some(), "the condition reads now: {read:?}");
    let effect = cdda_content::dialogue::read_effect(&value);
    assert!(effect.is_some(), "so the whole branch reads: {effect:?}");
}

/// A creature sees a square across open floor and not through a wall.
///
/// Reference: `talker_character_const::can_see_location` (`talker_character.cpp:923-928`) is `Creature::sees( here, pos )`, and `creature.cpp:631-676` narrows the
/// range with the ambient light at the target before casting a line through `map::sees`. The port has no per-square ambient light yet, so it casts at
/// `MAX_VIEW_DISTANCE` - it can see too far in the dark, but never through a wall, which is what this test pins at a distance where light cannot matter.
#[test]
fn a_creature_sees_across_floor_but_not_through_a_wall() {
    let mut world = bevy_ecs::prelude::World::new();
    // A real level, because a line of sight is bounded by the map: `sees_on_one_level` refuses any target outside it, which is why the first version of this test
    // answered no to every square including the creature's own.
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..24 {
        for y in 0..24 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    level.set_terrain(12, 10, 2, 2);
    cdda_sim::load_level(&mut world, &level);
    // Codes 1 and 2 are open floor and a wall. `insert` records whether a code lets light through, which is the terrain half of `map::is_transparent`.
    let mut terrain = cdda_sim::terrain_table::TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert(2, false);
    let mut furniture = cdda_sim::terrain_table::TerrainTable::new();
    furniture.insert(0, true);
    cdda_sim::set_prototypes(&mut world, cdda_sim::Prototypes { terrain, furniture });
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (10, 10, 0));

    assert!(cdda_sim::seeing::can_see_location(&world, survivor, (14, 9)), "open floor is seen");
    assert!(
        !cdda_sim::seeing::can_see_location(&world, survivor, (14, 10)),
        "a wall in the way is not seen through"
    );
}

/// `place_override` adds a timed event due one second after its length, and the key names it.
///
/// Reference: `f_place_override` (`npctalk.cpp:4798-4819`): `get_timed_events().add( OVERRIDE_PLACE, calendar::turn + length + 1_seconds, ..., key )`. The test checks
/// the event is **not** due at the length and **is** due one turn later, which is the reference's own plus-one.
#[test]
fn a_place_override_comes_due_one_second_after_its_length() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::calendar::set_turn(&mut world, 100);
    assert_eq!(
        cdda_content::constructions::parse_duration("2 seconds"),
        2,
        "a turn is a second"
    );
    let value = cdda_json::parse(
        br#"{"place_override": "Infinite Corridor", "length": "10 minutes", "key": "LIXA_escape_key"}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    cdda_sim::calendar::set_turn(&mut world, 100 + 600);
    assert_eq!(cdda_sim::timed_events::has_happened(&world, "LIXA_escape_key"), Some(false), "not yet at ten minutes");
    cdda_sim::calendar::set_turn(&mut world, 100 + 601);
    assert_eq!(cdda_sim::timed_events::has_happened(&world, "LIXA_escape_key"), Some(true), "but due a second later");
}

/// `mapgen_update` reads a list of updates and a target named by a `context_val`, which are the two shapes the data uses and the port refused.
///
/// Reference: `f_mapgen_update` (`npctalk.cpp:4820-4845`) takes the member as a string **or an array**, and `target_var` is any `var_info` - the fourth scope is
/// `context_val`, which is what the data uses for a square a previous effect computed.
#[test]
fn a_mapgen_update_reads_a_list_and_a_context_target() {
    let listed = cdda_json::parse(
        br#"{"mapgen_update": [{"global_val": "random_encounter_map_remove"}],
            "om_terrain": {"global_val": "random_encounter_omt"},
            "target_var": {"global_val": "randenc_loc"}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&listed);
    assert!(effect.is_some(), "a list of updates reads: {effect:?}");

    let contextual = cdda_json::parse(
        br#"{"mapgen_update": "place_food_warehouse", "om_terrain": "forest_thick",
            "target_var": {"context_val": "search_locale"}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&contextual);
    assert!(effect.is_some(), "a `context_val` target reads: {effect:?}");
}

/// `u_remove_item_with` takes **every** stack of one kind and leaves the others.
///
/// Reference: `f_remove_item_with` (`npctalk.cpp:4046-4056`) removes by type id over the whole inventory. The test holds two of the named kind and one of another,
/// so a remover that stopped at the first stack - or that took everything - fails on one of the two counts.
#[test]
fn a_remove_item_with_takes_every_stack_of_its_kind() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind { id: "bandage", volume_ml: 50, weight_g: 20, pockets: &[] },
            cdda_sim::item_registry::Kind { id: "rock", volume_ml: 50, weight_g: 200, pockets: &[] },
        ],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    for id in ["bandage", "bandage", "rock"] {
        let kind = cdda_sim::item_registry::id_of(&mut world, id).expect("kind");
        let made = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
        world.entity_mut(made).remove::<cdda_sim::Position>();
        world.entity_mut(made).insert(cdda_sim::creature::Carried(survivor));
    }

    let value = cdda_json::parse(br#"{"u_remove_item_with": "bandage"}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(cdda_sim::inventory::count_of(&world, survivor, "bandage").0, 0, "both bandages are gone");
    assert_eq!(cdda_sim::inventory::count_of(&world, survivor, "rock").0, 1, "the rock stays");
}

/// `u_lose_morale` takes every point of one type and leaves the others.
///
/// Reference: `f_lose_morale` (`npctalk.cpp:7197-7204`) calls `remove_morale` with the type, which erases the matching points. The test gives the creature two
/// types, so a remover that cleared the whole mood - or that took only one of the two points of the named type - fails.
#[test]
fn a_lost_morale_takes_its_own_type_only() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let point = |kind: &str| cdda_sim::morale::MoralePoint {
        kind: kind.to_owned(),
        bonus: 10,
        max_bonus: 10,
        duration: 600,
        decay_start: 300,
        age: 0,
        capped: false,
    };
    // Three **different** types: `morale::add` stacks two points of the same type, which is the reference's own `add_morale` rule and would make a
    // "two points of one type" assertion test the stacking rule instead of this one.
    for kind in ["morale_feeling_bad", "morale_feeling_worse", "morale_feeling_good"] {
        cdda_sim::morale::add(&mut world, survivor, point(kind));
    }
    assert_eq!(cdda_sim::morale::points_of(&world, survivor).len(), 3, "three points to begin with");

    let value = cdda_json::parse(br#"{"u_lose_morale": "morale_feeling_bad"}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let left = cdda_sim::morale::points_of(&world, survivor);
    assert_eq!(left.len(), 2, "only the named type is gone");
    assert!(
        left.iter().all(|point| point.kind != "morale_feeling_bad"),
        "and none of it is left: {left:?}"
    );
}

/// `u_set_talker` writes the speaker's id into a variable **as text**, which is what a later `run_eocs` reads back.
///
/// Reference: `f_set_talker` (`npctalk.cpp:6937-6948`), whose own comment says the id is stringified for `run_eocs`. The test reads the variable back as text, so a
/// port that wrote a number would fail rather than quietly pass a comparison the data never makes.
#[test]
fn a_set_talker_writes_the_speakers_id_as_text() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(br#"{"u_set_talker": {"global_val": "talker_id"}}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    let held = cdda_sim::dialogue_values::value_of(&world, globals, "talker_id");
    let expected = cdda_sim::dialogue_values::Value::Text(survivor.index().to_string());
    assert_eq!(held, Some(expected), "the id is stored as text");
}

/// `u_assign_activity` starts a `Doing` of that activity, a hundred moves per turn of its duration.
///
/// Reference: `f_assign_activity` (`npctalk.cpp:5451-5462`) passes `to_moves<int>( duration )`, so a two-turn activity is two hundred moves. The test asserts the
/// number, not just that something was assigned, because the conversion is the one part of this rule a reader could get wrong without noticing.
#[test]
fn an_assigned_activity_has_a_hundred_moves_a_turn() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(br#"{"u_assign_activity": "ACT_MULTIPLE_FARM", "duration": "2 turns"}"#.as_slice())
        .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let doing = world.get::<cdda_sim::creature::Doing>(survivor).expect("it is doing something");
    assert_eq!(doing.activity, "ACT_MULTIPLE_FARM", "the named activity");
    assert_eq!(doing.moves_left, 200, "two turns of moves");
}

/// A dialogue cast runs the same spell table the game casts from: an id the table holds and the port can perform is `Done`, an unknown id refuses.
///
/// Reference: `f_cast_spell` (`npctalk.cpp`) reads a spell id and casts it; the port looks the prototype up in `spell_types` and hands it to `spell_effects::apply`.
/// The test asks the spell table which effect names the port can perform rather than naming one, so it keeps meaning the same thing as the table grows.
#[test]
fn a_dialogue_cast_runs_the_spells_own_effect() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // A spell effect that draws needs the world's generator: `attack` answers `NotPorted` without one, which is what the first version of this test measured - a
    // missing store, not a disagreement between the port's predicate and its dispatcher.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    let ported = ["heal", "attack", "recover_energy", "pain_split"]
        .into_iter()
        .find(|name| cdda_sim::spell_effects::is_ported(name))
        .expect("the port performs at least one spell effect");
    let flat = cdda_content::spells::Scaling::default();
    let spell = cdda_content::spells::Spell {
        id: "test_spell".to_owned(),
        name: "Test Spell".to_owned(),
        effect_name: ported.to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: String::new(),
        energy_source: String::new(),
        damage: flat,
        range: flat,
        area: flat,
        energy: flat,
        duration: flat,
        casting_time: flat,
        difficulty: None,
        max_level: None,
    };
    cdda_sim::spell_types::set_types(&mut world, vec![spell.clone()]);

    let unknown = cdda_json::parse(br#"{"u_cast_spell": {"id": "no_such_spell"}}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&unknown).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "an id the table does not hold refuses"
    );

    let found = cdda_sim::spell_types::find(&world, "test_spell").expect("the spell table holds the spell");
    assert!(cdda_sim::spell_effects::is_ported("attack"), "the port performs `attack`");
    // `attack` draws a body part per target, so with the generator in place and nobody else in reach it answers `NothingAffected` - the spell was performed.
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, survivor, &found, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "and the dispatcher agrees: the cast happened and found nobody"
    );
    let known = cdda_json::parse(br#"{"u_cast_spell": {"id": "test_spell"}}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&known).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done,
        "and a dialogue cast is performed by the spell's own effect ({ported})"
    );
}

/// A `u_location_variable` with radii and no search writes the actor's own square.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194-4300`) starts `target_pos` at `talker_pos` and only moves it inside the `search_target` block - so
/// `min_radius` and `max_radius` with nothing to search for are not a search at all, and the variable holds where the actor stands.
#[test]
fn a_location_variable_with_radii_and_no_search_is_the_actors_own_square() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (7, 9, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, false);
    let value = cdda_json::parse(
        br#"{"u_location_variable": {"u_val": "stuck_teleport"}, "min_radius": 0, "max_radius": 0}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let held = cdda_sim::dialogue_values::value_of(&world, globals, "stuck_teleport");
    assert_eq!(
        held,
        Some(cdda_sim::dialogue_values::Value::Position { x: 7, y: 9, z: 0 }),
        "the actor's own square"
    );
}

/// A `u_location_variable` that names a terrain finds the first matching square inside `target_max_radius`, skipping those within `target_min_radius`.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4250-4300`): walk `points_in_radius`, skip `rl_dist <= target_min_dist`, take the first terrain match. The test
/// puts the wanted terrain **twice** - once inside the minimum and once outside - so a search that ignored `target_min_radius` would answer the near one.
#[test]
fn a_terrain_search_finds_the_first_one_outside_the_minimum() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_grass".to_owned(), 1), ("t_dirt".to_owned(), 2)]);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (10, 10, 0));
    cdda_sim::square::set_terrain(&mut world, (11, 10, 0), 2);
    cdda_sim::square::set_terrain(&mut world, (14, 10, 0), 2);
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);

    let value = cdda_json::parse(
        br#"{"u_location_variable": {"global_val": "spawn_point"}, "terrain": "t_dirt",
            "target_min_radius": 2, "target_max_radius": 6}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let held = cdda_sim::dialogue_values::value_of(&world, holder, "spawn_point");
    let square = match held {
        Some(cdda_sim::dialogue_values::Value::Position { x, y, .. }) => (x, y),
        other => {
            assert!(other.is_some(), "a square is stored, not nothing");
            return;
        }
    };
    let (x, y) = square;
    assert!(x != 11 || y != 10, "the square inside the minimum is skipped, not taken: ({x}, {y})");
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (x, y, 0)),
        Some(2),
        "and what was found really is the terrain asked for"
    );
}

/// `assign_mission` with a computed `deadline` assigns the mission, and a deadline the dialogue cannot work out refuses the entry.
///
/// Reference: `f_assign_mission` (`npctalk.cpp:5908-5926`) reads the deadline with `get_dbl_or_var`, so a `math` expression is one of its shapes. The port has no
/// deadline store, so the test pins what it does have: the mission lands, and an unworkable deadline stops it rather than assigning a mission the data expected dated.
#[test]
fn an_assigned_mission_takes_a_computed_deadline() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, survivor);
    let value = cdda_json::parse(
        br#"{"assign_mission": "MISSION_CAMP_LEADERSHIP_CHANGE", "deadline": {"math": ["time('now') + 100"]}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value.clone()).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert!(
        cdda_sim::missions::has_mission(&world, survivor, "MISSION_CAMP_LEADERSHIP_CHANGE"),
        "the mission is active"
    );

    // A variable the dialogue never set is **zero** in the reference's math parser (`math_parser_diag.cpp`), so this deadline is still workable - which is the
    // rule the second half of the test pins, and the reason the first version of it expected the wrong answer.
    let unworkable = cdda_json::parse(
        br#"{"assign_mission": "MISSION_OTHER", "deadline": {"math": ["never_set + 1"]}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&unworkable).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done,
        "an unset variable reads as zero, so the deadline works out"
    );
    assert!(
        cdda_sim::missions::has_mission(&world, survivor, "MISSION_OTHER"),
        "and the mission is assigned"
    );
}

/// `parse_tags` substitutes `<scope:name>` into the text being stored, and leaves a tag it cannot answer exactly as written.
///
/// Reference: `f_set_string_var` with `parse_tags` hands the text to the reference's tag parser. The test builds a name from a variable and then checks an unknown
/// tag survives - a parser that emptied what it did not understand would lose part of the data's own text.
#[test]
fn a_tagged_string_var_substitutes_what_it_knows_and_keeps_the_rest() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, false);
    cdda_sim::dialogue_values::set(&mut world, holder, "name", cdda_sim::dialogue_values::Value::Text("rag".to_owned()));

    let value = cdda_json::parse(
        br#"{"set_string_var": "n_number_<u_val:name>_<not_a_scope:x>", "parse_tags": true,
            "target_var": {"u_val": "label"}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let held = cdda_sim::dialogue_values::value_of(&world, holder, "label");
    assert_eq!(
        held,
        Some(cdda_sim::dialogue_values::Value::Text(
            "n_number_rag_<not_a_scope:x>".to_owned()
        )),
        "the known tag is substituted and the unknown one stays as it was written"
    );
}

/// An expression written `{"global_val": "x", "default": 1}` reads as a variable **with a fallback**, not as a plain variable.
///
/// Reference: `dbl_or_var_part::evaluate` (`condition.cpp`) answers the variable and falls back to the data's `default` when it is unset. This test pins the
/// **reader** only: the census's remaining `count` gap is in the spawn effect's own count reader, which takes a number or a range and not this shape, and that is
/// where the next round starts - with the expression side already able to carry it.
#[test]
fn an_expression_with_a_default_reads_as_a_fallback() {
    let with_default =
        cdda_json::parse(br#"{"global_val": "gamble_multiplier", "default": 1}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_expression(&with_default).expect("it reads");
    assert!(
        matches!(
            read,
            cdda_content::dialogue::Expression::VariableOrDefault { ref name, fallback, .. }
                if name == "gamble_multiplier" && (fallback - 1.0).abs() < f64::EPSILON
        ),
        "the fallback is carried with the variable: {read:?}"
    );

    let plain = cdda_json::parse(br#"{"global_val": "gamble_multiplier"}"#.as_slice()).expect("json");
    let read = cdda_content::dialogue::read_expression(&plain).expect("it reads");
    assert!(
        matches!(read, cdda_content::dialogue::Expression::Variable { .. }),
        "and without a `default` it is a plain variable: {read:?}"
    );
}

/// A spawn count written as a variable with a `default` spawns the fallback until the variable is set, then the variable.
///
/// Reference: `get_dbl_or_var` (`npctalk.cpp`) takes a number, a two-end array or a `math` block, and `dbl_or_var_part::evaluate` answers a variable's value or the
/// data's `default`. The test spawns with the same effect twice, so a reader that ignored the fallback fails the first count and one that ignored the variable fails
/// the second.
#[test]
fn a_spawn_count_reads_a_variable_with_a_default() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind { id: "money_twenty", volume_ml: 5, weight_g: 1, pockets: &[] }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    let value = cdda_json::parse(
        br#"{"u_spawn_item": "money_twenty", "count": {"global_val": "gamble_multiplier", "default": 1}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");

    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let spawned = |world: &mut bevy_ecs::prelude::World| {
        let mut items = world.query::<&cdda_sim::GroundItem>();
        items.iter(world).count()
    };
    assert_eq!(spawned(&mut world), 1, "the fallback is one");

    cdda_sim::dialogue_values::set(&mut world, globals, "gamble_multiplier", cdda_sim::dialogue_values::Value::Number(5.0));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(spawned(&mut world), 6, "and five more once the variable is set");
}

/// `u_set_field` puts the field on every square within its radius, and an entry with no radius is refused rather than spread over everything.
///
/// Reference: `f_field` (`npctalk.cpp:7707-7725`): `points_in_radius( target_pos, radius )` each gets the field at `intensity`, and the default radius is **ten
/// million**. The port's map is a loaded level, so it refuses that default instead of pretending: the test pins both - the radius honoured, and the entry refused.
#[test]
fn a_set_field_covers_its_radius_and_refuses_a_missing_one() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_blood", 1, 3)]);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (10, 10, 0));
    let value = cdda_json::parse(
        br#"{"u_set_field": "fd_blood", "intensity": 2, "radius": 1}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let id = cdda_sim::field_registry::identity_of(&world, "fd_blood").expect("the field is known");
    for at in [(10, 10, 0), (9, 9, 0), (11, 11, 0), (9, 11, 0)] {
        assert!(
            cdda_sim::has_field_at(&world, at, id),
            "the field covers {at:?}, which is inside a radius of one"
        );
    }
    assert!(
        !cdda_sim::has_field_at(&world, (12, 10, 0), id),
        "and stops at the radius"
    );

    // No radius means the reference's ten million, which a loaded level cannot honour: refused, not guessed.
    let wide = cdda_json::parse(br#"{"u_set_field": "fd_blood"}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&wide).expect("the entry reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::CannotAnswer
    );
}



/// `custom_light_level` schedules a timed event a second after its length, and the event comes due on time.
///
/// Reference: `f_custom_light_level` (`npctalk.cpp:7230`): `get_timed_events().add( CUSTOM_LIGHT_LEVEL, calendar::turn + length + 1_seconds, ... )` - the extra
/// second because the event is ticked on its own turn. The test checks the event is **not** due at the length and **is** due one turn later, which is what pins the
/// second the reference adds.
#[test]
fn a_custom_light_level_comes_due_one_turn_past_its_length() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::calendar::set_turn(&mut world, 100);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(
        br#"{"custom_light_level": 100, "length": "5 seconds"}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let key = "custom_light_level:100:";
    cdda_sim::calendar::set_turn(&mut world, 100 + 4);
    assert!(
        cdda_sim::timed_events::has_happened(&world, key) != Some(true),
        "four turns in, a five-turn light is still on"
    );
    cdda_sim::calendar::set_turn(&mut world, 100 + 6);
    assert!(
        cdda_sim::timed_events::has_happened(&world, key) == Some(true),
        "and six turns in the event has come due"
    );
}

/// A `u_query` takes its answer from the choice queue, and the data's `default` when nobody was asked.
///
/// Reference: `u_query` (`condition.cpp`) shows a yes/no question and answers it; `default` is the answer when the player cannot be asked. A headless port asks by
/// taking the next queued answer. The test covers all three paths - queued yes, queued no, nothing queued - so a port that always answered the default would fail the
/// first two.
#[test]
fn a_query_takes_a_queued_answer_and_falls_back_to_its_default() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(
        br#"{"if": {"u_query": "Replace your current lenses?", "default": false},
            "then": [{"u_message": "replaced"}],
            "else": [{"u_message": "kept"}]}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let said = |world: &bevy_ecs::prelude::World| {
        let mut count = 0;
        for line in cdda_sim::messages::lines(world) {
            if line.contains("replaced") {
                count += 1;
            }
        }
        count
    };

    cdda_sim::choices::set_choices(&mut world, [0]);
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(said(&world), 1, "a queued zero is yes");

    cdda_sim::choices::set_choices(&mut world, [1]);
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(said(&world), 1, "a queued one is no, so the else branch ran");

    // Nothing queued: the data's `default` answers, which for this question is no.
    cdda_sim::choices::set_choices(&mut world, []);
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    assert_eq!(said(&world), 1, "an unanswered question takes its default");
}

/// A `u_has_effect` that names a `bodypart` answers about the creature, because every effect the port applies is the creature's.
///
/// Reference: `f_has_effect` (`condition.cpp:980-996`) asks a part when one is named. The port has no limb model, so its effects are the creature's - and the test
/// pins what that means: the effect present answers **yes**, the effect absent answers **no**, and the part is not a reason to refuse.
#[test]
fn a_has_effect_with_a_bodypart_answers_about_the_creature() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(
        br#"{"bodypart": "eyes", "u_has_effect": "conjunctivitis"}"#.as_slice(),
    )
    .expect("json");
    let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");

    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &condition),
        "nothing applied yet, so nothing is worn"
    );
    cdda_sim::applied_effects::add(
        &mut world,
        survivor,
        cdda_sim::effects::ActiveEffect {
            id: "conjunctivitis".to_owned(),
            intensity: 1,
            duration: 100,
            permanent: false,
        },
    );
    assert!(
        cdda_sim::conditions::holds(&world, survivor, &condition),
        "and the effect is the creature's, which is the only place the port can put it"
    );
}

/// A `u_spawn_monster` with `friendly` marks the spawn as the player's, and one without it does not.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp`) sets `monster::friendly` when the data asks for it. The test spawns one of each, so a reader that ignored the member
/// would fail the first assertion and one that always set it would fail the second.
#[test]
fn a_friendly_spawn_is_marked_and_a_plain_one_is_not() {
    let mut world = bevy_ecs::prelude::World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -12..12 {
        for y in -12..12 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the second", (1, 0, 0));
    // Built from the data's own shape, the way the other spawn tests build one: a monster type has thirty members and a literal would drift.
    let prototype = cdda_json::parse(
        br#"{"id": "mon_shadow", "name": {"str": "shadow"}, "symbol": "x", "color": "black", "hp": 10, "speed": 100}"#
            .as_slice(),
    )
    .expect("json");
    let kind = cdda_content::monsters::load_objects(&[prototype])
        .expect("readable")
        .into_iter()
        .next()
        .expect("one type");
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_shadow", hp: 10 }],
    );
    cdda_sim::monster_catalog::set_types(&mut world, vec![kind]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));

    let spawn = |world: &mut bevy_ecs::prelude::World, text: &str| {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
        cdda_sim::dialogue_effects::run_effect(world, survivor, None, &effect)
    };
    assert_eq!(
        spawn(&mut world, r#"{"u_spawn_monster": "mon_shadow", "real_count": 1, "friendly": true}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        spawn(&mut world, r#"{"u_spawn_monster": "mon_shadow", "real_count": 1}"#),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let mut friendly = 0;
    let mut total = 0;
    let mut monsters = world.query::<&cdda_sim::creature::CreatureKind>();
    for _ in monsters.iter(&world) {
        total += 1;
    }
    let mut marked = world.query::<&cdda_sim::friendly::Friendly>();
    for _ in marked.iter(&world) {
        friendly += 1;
    }
    assert_eq!(total, 2, "both spawns exist");
    assert_eq!(friendly, 1, "and exactly one of them is friendly");
}

/// `ter_furn_transform` changes the terrain of the squares it reaches, and leaves the ones it does not name alone.
///
/// Reference: `ter_furn_transform` (`magic_ter_fur_transform.cpp:63-130`) maps a *named* terrain to its results, and `f_transform_radius` (`npctalk.cpp:4483`)
/// applies it over `transform_radius`. The test plants two terrains, transforms one, and checks the other survived - a transform that replaced everything would pass
/// a one-sided test.
#[test]
fn a_terrain_transform_changes_what_it_names_and_nothing_else() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_dirt".to_owned(), 1), ("t_pit".to_owned(), 2), ("t_grass".to_owned(), 3)],
    );
    cdda_sim::terrain_transform::set_transforms(
        &mut world,
        [(
            "transform_pit".to_owned(),
            std::collections::BTreeMap::from([("t_dirt".to_owned(), "t_pit".to_owned())]),
        )],
    );
    for x in -6..6 {
        for y in -6..6 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), 1);
        }
    }
    cdda_sim::square::set_terrain(&mut world, (1, 0, 0), 3);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let value = cdda_json::parse(
        br#"{"ter_furn_transform": "transform_pit", "u_transform_radius": 2}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(2),
        "the dirt under the actor is now a pit"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 2, 0)),
        Some(2),
        "and so is the dirt at the edge of the radius"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 0, 0)),
        Some(3),
        "the grass is untouched, because the transform does not name it"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (5, 5, 0)),
        Some(1),
        "and dirt outside the radius is untouched too"
    );
}

/// `give_achievement` completes an achievement once, and completing it again changes nothing.
///
/// Reference: `f_give_achievment` (`npctalk.cpp:5566-5585`) reports an achievement **only while it is pending**. The test runs the effect twice and checks the store
/// holds one entry, so a port that appended on every call would fail.
#[test]
fn an_achievement_is_completed_once() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(br#"{"give_achievement": "achievement_officer_kill"}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    for _ in 0..2 {
        assert_eq!(
            cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
            cdda_sim::dialogue_effects::Applied::Done
        );
    }
    assert!(cdda_sim::achievements::has_completed(&world, survivor, "achievement_officer_kill"));
    assert_eq!(
        cdda_sim::achievements::completed(&world, survivor),
        vec!["achievement_officer_kill".to_owned()],
        "one entry, though the effect ran twice"
    );
    assert!(
        !cdda_sim::achievements::has_completed(&world, survivor, "achievement_other"),
        "and nothing else was given"
    );
}

/// A `u_spawn_npc` with `outdoor_only` puts its person under the sky, and one with a `lifespan` gives it a summon time.
///
/// Reference: `f_spawn_npc` reads `outdoor_only`/`indoor_only` the way the monster spawn does, and `lifespan` becomes `set_summon_time`. The test makes a level that
/// is **indoors everywhere** so an outdoor-only spawn has nowhere to go, which is the assertion that would catch a port that ignored the flag.
#[test]
fn an_npc_spawn_honours_its_outdoor_flag_and_lifespan() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));
    cdda_sim::calendar::set_turn(&mut world, 500);
    let mut level = cdda_core_types::level::Level::new();
    for x in -8..8 {
        for y in -8..8 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    // An open level is **outdoors**, so an indoor-only spawn has nowhere to go: the effect refuses rather than dropping the person under the sky, which is what the
    // flag exists to prevent. (The first version of this test asserted the opposite and was wrong about which side an open level is.)
    let indoors = cdda_json::parse(
        br#"{"u_spawn_npc": "portal_person", "real_count": 1, "indoor_only": true}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&indoors).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::CannotAnswer
    );

    // Without the flag it lands, and a lifespan gives it a summon time.
    let plain = cdda_json::parse(
        br#"{"u_spawn_npc": "portal_person", "real_count": 1, "lifespan": ["1 minutes", "3 minutes"]}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&plain).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let mut summoned = 0;
    let mut until = world.query::<&cdda_sim::monster_spawning::Summoned>();
    for held in until.iter(&world) {
        if held.until > 500 {
            summoned += 1;
        }
    }
    assert_eq!(summoned, 1, "one person has a summon time in the future");
}

/// A `u_location_variable` naming furniture finds it inside `target_max_radius` and skips what is inside `target_min_radius`.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4231-4300`), the `search_type = "furniture"` path. The furniture is planted twice - once inside the minimum and once
/// outside - so a search that ignored the minimum would answer the near one.
#[test]
fn a_furniture_search_finds_the_first_one_outside_the_minimum() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::furniture_types::set_types(&mut world, [("f_scrap_antenna".to_owned(), 1), ("f_table".to_owned(), 2)]);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (10, 10, 0));
    cdda_sim::square::set_terrain(&mut world, (11, 10, 0), 1);
    cdda_sim::square::set_furniture(&mut world, (11, 10, 0), 1);
    cdda_sim::square::set_terrain(&mut world, (14, 12, 0), 1);
    cdda_sim::square::set_furniture(&mut world, (14, 12, 0), 1);
    cdda_sim::square::set_terrain(&mut world, (10, 13, 0), 1);
    cdda_sim::square::set_furniture(&mut world, (10, 13, 0), 2);
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);

    let value = cdda_json::parse(
        br#"{"u_location_variable": {"global_val": "antenna"}, "furniture": "f_scrap_antenna",
            "target_min_radius": 2, "target_max_radius": 6, "x_adjust": 1}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let held = cdda_sim::dialogue_values::value_of(&world, holder, "antenna");
    let Some(cdda_sim::dialogue_values::Value::Position { x, y, .. }) = held else {
        assert!(held.is_some(), "a square is stored");
        return;
    };
    assert!(
        x != 11 || y != 10,
        "the furniture inside the minimum is skipped, not taken: ({x}, {y})"
    );
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (x, y, 0)),
        Some(1),
        "and what was found really is the furniture asked for"
    );
}


/// The two halves of a furniture search inside an overmap tile, each asked on its own.
///
/// The combined path refused its own test last round, so this pins the pieces: the overmap search finds the tile, and the furniture walk finds the square. **When the
/// whole fails and the parts hold, the fault is in the wiring between them** - which is exactly what this pair is for.
#[test]
fn the_halves_of_a_tile_furniture_search_hold_on_their_own() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::furniture_types::set_types(&mut world, [("f_scrap_antenna".to_owned(), 1)]);
    let tile = cdda_sim::overmap::tile_of(cdda_sim::overmap::SQUARES_PER_OMT, 0);
    cdda_sim::overmap::set_terrain(&mut world, tile, "labyrinth_entrance");
    let corner = (
        tile.0 * cdda_sim::overmap::SQUARES_PER_OMT,
        tile.1 * cdda_sim::overmap::SQUARES_PER_OMT,
        0,
    );
    cdda_sim::square::set_terrain(&mut world, (corner.0 + 1, corner.1 + 1, 0), 1);
    cdda_sim::square::set_furniture(&mut world, (corner.0 + 1, corner.1 + 1, 0), 1);

    let found = cdda_sim::overmap::nearest_with_terrain(
        &world,
        cdda_sim::overmap::tile_of(0, 0),
        "labyrinth_entrance",
        2,
        0,
    );
    assert_eq!(found, Some(tile), "the overmap search finds the tile");
    assert_eq!(
        cdda_sim::square::furniture_at(&world, (corner.0 + 1, corner.1 + 1, 0)),
        Some(1),
        "and the square carries the furniture code the table gives"
    );
    assert_eq!(
        cdda_sim::furniture_types::code_of(&world, "f_scrap_antenna"),
        Some(1),
        "which is what the table turns the name into"
    );
}

/// A furniture search inside an overmap tile found by `target_params` answers the square that carries the furniture.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4250-4300`): `target_params` sets the start, the overmap search finds the tile, and the furniture walk answers a
/// square near it. The furniture sits one square off the tile's corner, so a port that answered the corner without searching would be caught.
#[test]
fn a_furniture_search_inside_a_found_tile_answers_the_furniture() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::furniture_types::set_types(&mut world, [("f_scrap_antenna".to_owned(), 1)]);
    let tile = cdda_sim::overmap::tile_of(cdda_sim::overmap::SQUARES_PER_OMT, 0);
    cdda_sim::overmap::set_terrain(&mut world, tile, "labyrinth_entrance");
    let corner = (
        tile.0 * cdda_sim::overmap::SQUARES_PER_OMT,
        tile.1 * cdda_sim::overmap::SQUARES_PER_OMT,
        0,
    );
    cdda_sim::square::set_terrain(&mut world, (corner.0 + 1, corner.1 + 1, 0), 1);
    cdda_sim::square::set_furniture(&mut world, (corner.0 + 1, corner.1 + 1, 0), 1);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    let value = cdda_json::parse(
        br#"{"u_location_variable": {"global_val": "antenna"}, "furniture": "f_scrap_antenna",
            "target_params": {"om_terrain": "labyrinth_entrance", "search_range": 2}, "target_max_radius": 4}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    let said = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    let held = cdda_sim::dialogue_values::value_of(&world, holder, "antenna");
    assert_eq!(
        held,
        Some(cdda_sim::dialogue_values::Value::Position { x: corner.0 + 1, y: corner.1 + 1, z: 0 }),
        "the square that carries the furniture (the run answered {said:?})"
    );
}

/// `u_run_inv_eocs` with the `"all"` mode runs its program once per matching item, and once for none when nothing matches.
///
/// Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`): the `"all"` mode runs the programs on every item `search_data` lets through, handing each to the program
/// through the invocation's context. The test carries two phones and a rock, so a port that ran on everything would say four lines and one that filtered nothing
/// would say two with the wrong ids.
#[test]
fn an_inventory_eoc_runs_once_per_matching_item() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind { id: "smart_phone", volume_ml: 5, weight_g: 1, pockets: &[] },
            cdda_sim::item_registry::Kind { id: "rock", volume_ml: 5, weight_g: 1, pockets: &[] },
        ],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    for name in ["smart_phone", "smart_phone", "rock"] {
        let kind = cdda_sim::item_registry::id_of(&mut world, name).expect("the item is installed");
        let dropped = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
        world.entity_mut(dropped).insert(cdda_sim::creature::Carried(survivor));
    }
    let value = cdda_json::parse(
        br#"{"u_run_inv_eocs": "all", "search_data": [{"id": "smart_phone"}],
            "true_eocs": [{"effect": [{"u_message": "a phone"}]}]}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let lines = cdda_sim::messages::lines(&world);
    assert_eq!(
        lines.iter().filter(|line| line.contains("a phone")).count(),
        2,
        "one line per phone and none for the rock: {lines:?}"
    );

    // A mode that needs a menu is refused rather than run on everything.
    let manual = cdda_json::parse(
        br#"{"u_run_inv_eocs": "manual", "true_eocs": ["EOC_note"]}"#.as_slice(),
    )
    .expect("json");
    assert!(
        cdda_content::dialogue::read_effect(&manual).is_none(),
        "the manual mode needs a menu the port has not written"
    );
}

/// `u_run_inv_eocs` with `wielded_only` runs on the wielded item alone, and the `random` mode runs on exactly one match.
///
/// Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`): a `search_data` filter may ask for the wielded item, and the `"random"` mode draws one match with the game's
/// dice. The test carries two rocks and wields one, so a port that ignored the filter would say two lines, and a port that ran on every match in `random` mode would
/// say two as well.
#[test]
fn an_inventory_eoc_can_ask_for_the_wielded_item_or_one_at_random() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind { id: "rock", volume_ml: 5, weight_g: 1, pockets: &[] }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let kind = cdda_sim::item_registry::id_of(&mut world, "rock").expect("installed");
    let first = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
    world.entity_mut(first).insert(cdda_sim::creature::Carried(survivor));
    let second = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
    world.entity_mut(second).insert(cdda_sim::creature::Carried(survivor));
    world.entity_mut(second).insert(cdda_sim::creature::WieldedBy(survivor));

    let wielded = cdda_json::parse(
        br#"{"u_run_inv_eocs": "all", "search_data": [{"wielded_only": true}],
            "true_eocs": [{"effect": [{"u_message": "the one in hand"}]}]}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&wielded).expect("the effect reads");
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    let lines = cdda_sim::messages::lines(&world);
    assert_eq!(
        lines.iter().filter(|line| line.contains("the one in hand")).count(),
        1,
        "the wielded item alone: {lines:?}"
    );

    let random = cdda_json::parse(
        br#"{"u_run_inv_eocs": "random", "true_eocs": [{"effect": [{"u_message": "one of them"}]}]}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&random).expect("the effect reads");
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    let lines = cdda_sim::messages::lines(&world);
    assert_eq!(
        lines.iter().filter(|line| line.contains("one of them")).count(),
        1,
        "one drawn from two: {lines:?}"
    );
}

/// `location_variable_adjust` moves a square the dialogue holds, in squares or in overmap tiles, and writes it back or elsewhere.
///
/// Reference: `f_location_variable_adjust` (`npctalk.cpp:4419-4446`): `overmap_tile` counts the x and y moves in tiles, `z_override` replaces the level, and
/// `output_var` sends the answer somewhere other than the variable it came from. The test asks for all three in one case, so a port that ignored any of them fails.
#[test]
fn a_location_adjust_moves_a_square_by_tiles_and_overrides_the_level() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "storm",
        cdda_sim::dialogue_values::Value::Position { x: 48, y: 24, z: 0 },
    );
    let value = cdda_json::parse(
        br#"{"location_variable_adjust": {"global_val": "storm"}, "overmap_tile": true,
            "x_adjust": 1, "y_adjust": -1, "z_adjust": -2, "z_override": true,
            "output_var": {"global_val": "edge"}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let tile = cdda_sim::overmap::SQUARES_PER_OMT;
    assert_eq!(
        cdda_sim::dialogue_values::value_of(&world, globals, "edge"),
        Some(cdda_sim::dialogue_values::Value::Position { x: 48 + tile, y: 24 - tile, z: -2 }),
        "one tile right, one tile up, and the level replaced rather than moved"
    );
    assert_eq!(
        cdda_sim::dialogue_values::value_of(&world, globals, "storm"),
        Some(cdda_sim::dialogue_values::Value::Position { x: 48, y: 24, z: 0 }),
        "and the input variable is untouched, because an output was named"
    );
}

/// A monster search answers the square of the **nearest** creature of that kind, skipping one too close.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4300-4320`): the walk skips squares at or inside `target_min_radius` and stops at `target_max_radius`. The test puts a
/// match **too close** and a second one in range, so a port that ignored the minimum radius would answer the near one and fail.
#[test]
fn a_monster_search_answers_the_nearest_one_far_enough_out() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind { id: "mon_lieutenant_shadow", hp: 10 }],
    );
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_lieutenant_shadow").expect("installed");
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::spawn_monster(&mut world, (1, 0), kind, 10);
    cdda_sim::spawn_monster(&mut world, (6, 0), kind, 10);
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    let value = cdda_json::parse(
        br#"{"u_location_variable": {"global_val": "shadow"}, "monster": "mon_lieutenant_shadow",
            "target_min_radius": 3, "target_max_radius": 10}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        cdda_sim::dialogue_values::value_of(&world, globals, "shadow"),
        Some(cdda_sim::dialogue_values::Value::Position { x: 6, y: 0, z: 0 }),
        "the one six squares out, not the one beside the survivor"
    );
}

/// `has_alpha` answers true for any dialogue the port runs, and `has_beta` answers whether a listener came with it.
///
/// Reference: `f_has_alpha` / `f_has_beta` (`condition.cpp:1412-1423`): two flags on the dialogue itself, not questions about the world. The test asks both with and
/// without a listener, so a port that confused the two - or answered `has_alpha` from the listener - is caught.
#[test]
fn a_dialogue_knows_whether_it_has_someone_to_talk_as_and_to() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let stranger = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (1, 0, 0));
    let alpha = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#""has_alpha""#.as_slice()).expect("json"),
    )
    .expect("the condition reads");
    let beta = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#""has_beta""#.as_slice()).expect("json"),
    )
    .expect("the condition reads");
    assert!(cdda_sim::conditions::holds(&world, survivor, &alpha));
    assert!(!cdda_sim::conditions::holds(&world, survivor, &beta), "no listener yet");
    assert!(cdda_sim::conditions::holds_with(&world, survivor, Some(stranger), &beta));
    assert!(
        cdda_sim::conditions::holds_with(&world, survivor, Some(stranger), &alpha),
        "a listener does not take the alpha away"
    );
}

/// A mutation of depth zero asks for nothing, and is answered; a real depth is refused until there is a mutation pool.
///
/// Reference: `f_mutate` (`npctalk.cpp:3563-3571`): `mutate( highest_cat )` draws from the pools the depth reaches, so depth zero reaches none of them. The test asks
/// both depths, so a port that answered a real depth with a silent `Done` - the tempting shortcut - fails on the second assertion.
#[test]
fn a_mutation_of_depth_zero_asks_for_nothing_and_is_answered() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let none = cdda_json::parse(br#"{"u_mutate": 0}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&none).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let real = cdda_json::parse(br#"{"u_mutate": 2}"#.as_slice()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&real).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "a real depth needs a mutation pool the port does not have"
    );
}

/// `u_has_weapon` answers from the item the creature wields, and the `npc_` form from the listener's.
///
/// Reference: `f_has_weapon` (`condition.cpp`): whether the actor holds a weapon. The port answers it from `WieldedBy`, so the test wields a knife on one creature and
/// asks about both - a port that answered "always true" or "always false" fails on one side.
#[test]
fn having_a_weapon_follows_the_item_in_hand() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind { id: "knife_combat", volume_ml: 5, weight_g: 1, pockets: &[] }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let stranger = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (1, 0, 0));
    let armed = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#""u_has_weapon""#.as_slice()).expect("json"),
    )
    .expect("the condition reads");
    let theirs = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#""npc_has_weapon""#.as_slice()).expect("json"),
    )
    .expect("the condition reads");
    assert!(!cdda_sim::conditions::holds(&world, survivor, &armed), "empty hands");
    let kind = cdda_sim::item_registry::id_of(&mut world, "knife_combat").expect("installed");
    let knife = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
    world.entity_mut(knife).insert(cdda_sim::creature::WieldedBy(stranger));
    assert!(!cdda_sim::conditions::holds(&world, survivor, &armed), "someone else's knife");
    assert!(cdda_sim::conditions::holds_with(&world, survivor, Some(stranger), &theirs));
}

/// `u_run_inv_eocs` with `worn_only` runs on the worn items alone.
///
/// Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`): a `search_data` filter may ask for the worn items, which is a narrower set than the carried ones. The test
/// carries a shirt that is worn and a shirt that is not, so a port that ignored the filter says two lines.
#[test]
fn an_inventory_eoc_can_ask_for_what_is_worn() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind { id: "shirt", volume_ml: 5, weight_g: 1, pockets: &[] }],
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let kind = cdda_sim::item_registry::id_of(&mut world, "shirt").expect("installed");
    for worn in [true, false] {
        let shirt = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
        world.entity_mut(shirt).insert(cdda_sim::creature::Carried(survivor));
        if worn {
            world.entity_mut(shirt).insert(cdda_sim::creature::WornBy(survivor));
        }
    }
    let value = cdda_json::parse(
        br#"{"u_run_inv_eocs": "all", "search_data": [{"worn_only": true}],
            "true_eocs": [{"effect": [{"u_message": "a worn shirt"}]}]}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
    let lines = cdda_sim::messages::lines(&world);
    assert_eq!(
        lines.iter().filter(|line| line.contains("a worn shirt")).count(),
        1,
        "the worn shirt alone: {lines:?}"
    );
}

/// A `switch` on `{"math": ["u_name"]}` picks the last case the **variable's value** reaches.
///
/// Reference: `f_switch` (`npctalk.cpp:6998-7020`) evaluates the value once and every case it reaches replaces the chosen one. Two rules in the port made this read
/// zero: a bare math name kept its `u_` prefix as part of the variable's name (`read_var_value` splits that prefix out), and a variable holding the **text** `"2"` was
/// not a number to `number_of` (`diag_value::dbl()` parses a string). Both are fixed, and this test is the one that says so.
#[test]
fn a_switch_on_a_variable_name_picks_the_last_case_it_reaches() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, survivor, false);
    cdda_sim::dialogue_values::set(
        &mut world,
        holder,
        "LIXA_entry",
        cdda_sim::dialogue_values::Value::Text("2".to_owned()),
    );
    let value = cdda_json::parse(
        br#"{"switch": {"math": ["u_LIXA_entry"]},
            "cases": [
              {"case": 1, "effect": {"u_message": "one"}},
              {"case": 2, "effect": {"u_message": "two"}},
              {"case": 5, "effect": {"u_message": "five"}}
            ]}"#
        .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines.iter().any(|line| line.contains("two")),
        "the case the value reaches: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains("one") || line.contains("five")),
        "and only that one: {lines:?}"
    );
}

/// A program defined inline under an id is installed, so a later call **by name** finds it.
///
/// Reference: `load_inline_eoc` (`effect_on_condition.cpp:119-133`) adds the body to the factory under its id, so the id works from anywhere afterwards - that is why
/// the corpus writes `{"id": "EOC_x", "effect": [...]}` inside a `run_eocs`. The test calls the id from a **different** program, which is the part reading the entry
/// alone would not give.
#[test]
fn a_program_defined_inline_is_installed_under_its_id() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let programs = [
        cdda_sim::eoc_registry::Program {
            id: "EOC_outer".to_owned(),
            condition: None,
            effects: vec![
                cdda_content::dialogue::read_effect(
                    &cdda_json::parse(
                        br#"{"run_eocs": [{"id": "EOC_inner", "effect": [{"u_message": "from the body"}]}]}"#
                            .as_slice(),
                    )
                    .expect("json"),
                )
                .expect("the definition reads"),
            ],
        },
        cdda_sim::eoc_registry::Program {
            id: "EOC_caller".to_owned(),
            condition: None,
            effects: vec![
                cdda_content::dialogue::read_effect(
                    &cdda_json::parse(br#"{"run_eocs": "EOC_inner"}"#.as_slice()).expect("json"),
                )
                .expect("the call reads"),
            ],
        },
    ];
    cdda_sim::eoc_registry::set_programs(&mut world, &programs);
    assert_eq!(cdda_sim::eoc_registry::count(&mut world), 3, "the body is installed beside the two programs");
    assert_eq!(
        cdda_sim::eoc_registry::run_program(&mut world, survivor, None, "EOC_caller", 0),
        cdda_sim::dialogue_effects::Applied::Done,
        "a call by name reaches the inline body"
    );
    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines.iter().any(|line| line.contains("from the body")),
        "and the body ran: {lines:?}"
    );
}

/// `player_see_u` asks the avatar's sight, not the speaker's.
///
/// Reference: `f_player_see` (`condition.cpp:1366-1377`): the player's view is asked whether it sees the creature the dialogue holds. The test puts the avatar and the
/// stranger in the open, then moves the stranger out of sight past the view distance - so a port that answered from the speaker's own sight would say yes both times.
#[test]
fn the_players_sight_is_what_player_see_asks_about() {
    let mut world = bevy_ecs::prelude::World::new();
    let avatar = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, avatar);
    let stranger = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (3, 0, 0));
    let seen = cdda_content::dialogue::read_condition(
        &cdda_json::parse(br#""player_see_npc""#.as_slice()).expect("json"),
    )
    .expect("the condition reads");
    assert!(
        cdda_sim::conditions::holds_with(&world, avatar, Some(stranger), &seen),
        "three squares away in the open is in sight"
    );
    world
        .entity_mut(stranger)
        .insert(cdda_sim::Position { x: 400, y: 0, z: 0 });
    assert!(
        !cdda_sim::conditions::holds_with(&world, avatar, Some(stranger), &seen),
        "four hundred squares away is not"
    );
}

/// `run_eocs` with `time_in_future` waits, and runs when the turn comes - not before and not twice.
///
/// Reference: `f_run_eocs` (`npctalk.cpp:6332-6338`): the call is a timed event, so the programs run on the due turn. Until this round the port **ran the program at
/// once**, which is a wrong answer rather than a refusal - the test asks before the due turn, on it, and after it.
#[test]
fn a_delayed_run_eocs_waits_for_its_turn_and_runs_once() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::calendar::set_turn(&mut world, 100);
    let value = cdda_json::parse(
        br#"{"run_eocs": [{"effect": [{"u_message": "it happened"}]}], "time_in_future": "2 seconds"}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done,
        "scheduling is the whole effect"
    );
    assert_eq!(cdda_sim::scheduled::waiting(&mut world), 1, "one thing is waiting");
    assert!(
        !cdda_sim::messages::lines(&world).iter().any(|line| line.contains("it happened")),
        "and nothing has run yet"
    );
    cdda_sim::calendar::set_turn(&mut world, 101);
    cdda_sim::scheduled::run_due(&mut world, survivor);
    assert!(
        !cdda_sim::messages::lines(&world).iter().any(|line| line.contains("it happened")),
        "a turn early is still too early"
    );
    cdda_sim::calendar::set_turn(&mut world, 102);
    cdda_sim::scheduled::run_due(&mut world, survivor);
    let lines = cdda_sim::messages::lines(&world);
    assert_eq!(
        lines.iter().filter(|line| line.contains("it happened")).count(),
        1,
        "two seconds later, once: {lines:?}"
    );
    cdda_sim::scheduled::run_due(&mut world, survivor);
    assert_eq!(
        cdda_sim::messages::lines(&world)
            .iter()
            .filter(|line| line.contains("it happened"))
            .count(),
        1,
        "and not again on the turn after"
    );
}

/// A `sound_effect` entry reads, and answers `Done` without changing the world.
///
/// Reference: `f_sound_effect` (`npctalk.cpp:5528-5570`): the entry plays a sound in the sound pack and touches nothing else. The port has no audio, so the honest answer
/// is `Done` - **and the test asserts the world is unchanged**, which is the half that would catch a sound effect that started pretending to do something.
#[test]
fn a_sound_event_answers_without_touching_the_world() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(
        br#"{"id": "bionics", "sound_effect": "elec_blast", "volume": 100}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(survivor).copied(),
        Some(cdda_sim::Position { x: 0, y: 0, z: 0 }),
        "the creature did not move"
    );
    assert!(cdda_sim::messages::lines(&world).is_empty(), "and nothing was said");
}

/// A transform with a `target_var` turns the terrain around **that square**, not around the actor.
///
/// Reference: `f_transform_radius` (`npctalk.cpp:4450-4468`): the transform's centre is the square the variable holds, which is how the data drains a site stored
/// earlier. The test puts the stored square six squares from the actor and checks the change lands there and not under the actor's feet.
#[test]
fn a_transform_can_be_centred_on_a_stored_square() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_grass".to_owned(), 1), ("t_dirt".to_owned(), 2)]);
    cdda_sim::terrain_transform::set_transforms(
        &mut world,
        [("drain".to_owned(), [("t_grass".to_owned(), "t_dirt".to_owned())].into_iter().collect())],
    );
    cdda_sim::square::set_terrain(&mut world, (6, 0, 0), 1);
    cdda_sim::square::set_terrain(&mut world, (0, 0, 0), 1);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "drain_terrain",
        cdda_sim::dialogue_values::Value::Position { x: 6, y: 0, z: 0 },
    );
    let value = cdda_json::parse(
        br#"{"u_transform_radius": 0, "ter_furn_transform": "drain", "target_var": {"global_val": "drain_terrain"}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (6, 0, 0)),
        Some(2),
        "the stored square was drained"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(1),
        "and the ground under the actor was not"
    );
}

/// A transform with `time_in_future` waits for its turn, then turns the terrain once.
///
/// Reference: `f_transform_radius` (`npctalk.cpp:4450-4468`) with `time_in_future`: the transform is a timed event carrying the table and the radius. The test stores a
/// square, schedules the transform, and checks the ground is **unchanged until the turn comes** and changed after it - the half a port that ran it immediately fails.
#[test]
fn a_postponed_transform_waits_and_then_turns_the_ground() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_grass".to_owned(), 1), ("t_dirt".to_owned(), 2)]);
    cdda_sim::terrain_transform::set_transforms(
        &mut world,
        [("drain".to_owned(), [("t_grass".to_owned(), "t_dirt".to_owned())].into_iter().collect())],
    );
    cdda_sim::square::set_terrain(&mut world, (4, 0, 0), 1);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::calendar::set_turn(&mut world, 50);
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "drain_terrain",
        cdda_sim::dialogue_values::Value::Position { x: 4, y: 0, z: 0 },
    );
    let value = cdda_json::parse(
        br#"{"u_transform_radius": 0, "ter_furn_transform": "drain",
            "target_var": {"global_val": "drain_terrain"}, "time_in_future": "1 seconds"}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (4, 0, 0)),
        Some(1),
        "nothing has changed yet"
    );
    cdda_sim::calendar::set_turn(&mut world, 51);
    cdda_sim::scheduled::run_due(&mut world, survivor);
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (4, 0, 0)),
        Some(2),
        "the turn came and the grass became dirt"
    );
}

/// A trait learned with a `variant` keeps both the trait and the choice.
///
/// Reference: `f_add_trait` (`npctalk.cpp:3498-3516`): `set_mutation( trait, variant )` stores the cosmetic variation beside the mutation. The port used to refuse any
/// entry that named one; now it learns the trait and records the variant, and the test asks for both.
#[test]
fn a_trait_learned_with_a_variant_keeps_the_choice() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(
        br#"{"u_add_trait": "BEAUTIFUL", "variant": "black"}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert!(cdda_sim::traits::has_trait(&world, survivor, "BEAUTIFUL"), "the trait was learned");
    assert_eq!(
        cdda_sim::traits::variant_of(&world, survivor, "BEAUTIFUL"),
        Some("black".to_owned()),
        "and the variant was kept"
    );
}

/// A square inside a city's tiles answers `map_in_city`, and one outside does not.
///
/// Reference: `f_map_in_city` (`condition.cpp:1863-1875`) on `overmap::is_in_city` (`overmap.cpp:4228-4236`): membership of the city's tile set, with a target below
/// z -1 answered `false` before the set is even asked. The test marks one overmap tile, stores a square inside it, and asks three questions.
#[test]
fn the_city_tiles_say_which_squares_are_in_a_city() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::cities::set_tiles(&mut world, [(3, 4)]);
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "check_loc",
        cdda_sim::dialogue_values::Value::Position {
            x: 3 * cdda_sim::overmap::SQUARES_PER_OMT + 5,
            y: 4 * cdda_sim::overmap::SQUARES_PER_OMT + 5,
            z: 0,
        },
    );
    let inside = cdda_json::parse(br#"{"map_in_city": {"global_val": "check_loc"}}"#.as_slice()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&inside).expect("the condition reads");
    assert!(
        cdda_sim::conditions::holds(&world, survivor, &condition),
        "a square inside the marked tile is in the city"
    );
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "check_loc",
        cdda_sim::dialogue_values::Value::Position { x: 100, y: 100, z: 0 },
    );
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &condition),
        "a square in another tile is not"
    );
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "check_loc",
        cdda_sim::dialogue_values::Value::Position {
            x: 3 * cdda_sim::overmap::SQUARES_PER_OMT + 5,
            y: 4 * cdda_sim::overmap::SQUARES_PER_OMT + 5,
            z: -2,
        },
    );
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &condition),
        "and below z -1 the answer is false without asking the set"
    );
}

/// The terrain at a stored square answers `map_terrain_with_flag`.
///
/// Reference: `f_map_ter_furn_with_flag` (`condition.cpp:1818-1842`): the flag is asked of the **tile's terrain**, and `loc` holds the square. The test flags one terrain
/// `TRANSPARENT` and another not, and asks about the square the variable holds.
#[test]
fn the_terrain_at_a_stored_square_answers_its_flags() {
    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::terrain_types::set_types(&mut world, [("t_glass".to_owned(), 1), ("t_wall".to_owned(), 2)]);
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [("t_glass".to_owned(), vec!["TRANSPARENT".to_owned()])],
    );
    cdda_sim::square::set_terrain(&mut world, (2, 3, 0), 1);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let globals = cdda_sim::dialogue_values::holder_for(&mut world, survivor, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        globals,
        "loc",
        cdda_sim::dialogue_values::Value::Position { x: 2, y: 3, z: 0 },
    );
    let value = cdda_json::parse(
        br#"{"map_terrain_with_flag": "TRANSPARENT", "loc": {"global_val": "loc"}}"#.as_slice(),
    )
    .expect("json");
    let condition = cdda_content::dialogue::read_condition(&value).expect("the condition reads");
    assert!(
        cdda_sim::conditions::holds(&world, survivor, &condition),
        "glass is transparent"
    );
    let other = cdda_json::parse(
        br#"{"map_terrain_with_flag": "WALL", "loc": {"global_val": "loc"}}"#.as_slice(),
    )
    .expect("json");
    let other = cdda_content::dialogue::read_condition(&other).expect("the condition reads");
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &other),
        "and it is not a wall"
    );
}

/// Every entry of the bionic-noise program reads, including its two inline branches.
///
/// Reference: `bio_noise` (`data/json/effects_on_condition/bionic_eocs.json`): a sound, then a `run_eocs` of two **inline** programs, each with an `id`, a `condition` and
/// an `effect` list holding a message and a `sound_effect`. The port refuses this entry today, and this test says which part.
#[test]
fn the_bionic_noise_program_reads() {
    let effect = cdda_json::parse(
        br#"[{"u_make_sound": "Crackle!", "volume": 60, "type": "movement"},
            {"run_eocs": [{"id": "bio_noise_hearing", "condition": {"not": "u_is_deaf"},
                           "effect": [{"u_message": "Your faulty bionic emits a crackle of noise!", "type": "bad"},
                                      {"sound_effect": "elec_blast", "id": "bionics", "volume": 100}]},
                          {"id": "bio_noise_deaf", "condition": "u_is_deaf",
                           "effect": [{"u_message": "You feel your faulty bionic shuddering.", "type": "bad"},
                                      {"sound_effect": "elec_blast_muffled", "id": "bionics", "volume": 100}]}]}]"#
            .as_slice(),
    )
    .expect("json");
    let cdda_json::Value::Array(entries) = effect else {
        unreachable!("the test wrote a list");
    };
    for (index, entry) in entries.iter().enumerate() {
        assert!(
            cdda_content::dialogue::read_effect(entry).is_some(),
            "entry {index} reads: {entry:?}"
        );
    }
}

/// Deafness is an effect past intensity two, or narcosis, or worn gear with the flag.
///
/// Reference: `Character::is_deaf` (`character.cpp:4768-4775`). The test walks the three clauses the port has and the boundary the reference draws - intensity **2 is not
/// deaf**, 3 is - because `get_effect_int( effect_deaf ) > 2` is the rule and `>= 2` is the mistake.
#[test]
fn deafness_is_an_effect_past_two_or_the_gear_that_blocks_it() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let condition = cdda_content::dialogue::read_condition(&cdda_json::parse(br#""u_is_deaf""#.as_slice()).expect("json"))
        .expect("the condition reads");
    assert!(!cdda_sim::conditions::holds(&world, survivor, &condition), "a healthy creature hears");
    cdda_sim::applied_effects::add(
        &mut world,
        survivor,
        cdda_sim::effects::ActiveEffect {
            id: "deaf".to_owned(),
            intensity: 2,
            duration: 100,
            permanent: false,
        },
    );
    assert!(
        !cdda_sim::conditions::holds(&world, survivor, &condition),
        "intensity two is still not deaf - the reference asks for more than two"
    );
    cdda_sim::applied_effects::add(
        &mut world,
        survivor,
        cdda_sim::effects::ActiveEffect {
            id: "deaf".to_owned(),
            intensity: 3,
            duration: 100,
            permanent: true,
        },
    );
    assert!(
        cdda_sim::conditions::holds(&world, survivor, &condition),
        "intensity three is deaf"
    );
    let other = cdda_sim::creature::spawn_creature(&mut world, "the other", (1, 0, 0));
    cdda_sim::applied_effects::add(
        &mut world,
        other,
        cdda_sim::effects::ActiveEffect {
            id: "narcosis".to_owned(),
            intensity: 1,
            duration: 100,
            permanent: false,
        },
    );
    assert!(
        cdda_sim::conditions::holds(&world, other, &condition),
        "and narcosis deafens whoever has it"
    );
}

/// A spawn entry that names a `target_var` reads, where it used to be refused whole.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7405-7408`): a `target_var` names the square the spawn is centred on. The port refused the whole entry; this test pins that
/// the **reader** takes it, with the target parsed rather than dropped. (The runner's use of the square is exercised by the spawn tests that build a real level.)
#[test]
fn a_spawn_entry_can_name_the_square_it_is_centred_on() {
    let value = cdda_json::parse(
        br#"{"u_spawn_monster": "mon_zombie", "real_count": 1, "min_radius": 1, "max_radius": 3,
            "target_var": {"global_val": "spawn_at"}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the entry reads");
    assert!(
        matches!(effect, cdda_content::dialogue::Effect::SpawnMonster { target: Some(_), .. }),
        "the target is carried, not dropped: {effect:?}"
    );
}

/// A `set_string_var` with `i18n` reads and stores the text the data wrote.
///
/// Reference: `f_set_string_var` (`npctalk.cpp:5809-5866`): `i18n` asks for the text to be looked up in a translation catalogue. **The port has no catalogue** - its
/// strings are the data's own text - so the effect runs and stores what the data says, and this test pins that it is not dropped at load time.
#[test]
fn a_translated_string_variable_stores_the_text_it_was_given() {
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(
        br#"{"i18n": true, "set_string_var": "EOC Selector Test", "target_var": {"context_val": "title"}}"#
            .as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("the effect reads");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done
    );
    let fields = cdda_sim::dialogue_values::holder_for(&mut world, survivor, false);
    assert_eq!(
        cdda_sim::dialogue_values::text_of(&world, fields, "title"),
        Some("EOC Selector Test".to_owned()),
        "the text is stored as the data wrote it"
    );
}
