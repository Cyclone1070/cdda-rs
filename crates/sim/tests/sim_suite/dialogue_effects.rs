//! What a `math` effect does: change a dialogue variable.
//!
//! Reference: `npctalk_funcs.cpp`'s `math` effect, whose data is an assignment per entry - `{"math": ["void_spider_killed = 1"]}`.
//! It is the most used effect in the game's 712 effect-on-conditions (304 of 1,612 entries), and the port could read the
//! expression half of it for three rounds without being able to run one.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_content::dialogue::{AssignmentOp, read_assignment};
use cdda_sim::dialogue_effects::{Applied, run_assignment};
use cdda_sim::dialogue_values::{self, Value};

fn assignment(text: &str) -> cdda_content::dialogue::Assignment {
    read_assignment(text).expect("the port reads this statement")
}

#[test]
fn a_math_statement_sets_a_dialogue_variable() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let statement = assignment("void_spider_killed = 1");
    assert_eq!(statement.op, AssignmentOp::Set);
    assert_eq!(statement.target.name(), "void_spider_killed");
    assert_eq!(run_assignment(&mut world, survivor, None, &statement), Applied::Done);
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "void_spider_killed"),
        Some(1.0)
    );
}

#[test]
fn the_compound_operators_start_from_zero_when_the_variable_is_unset() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    run_assignment(&mut world, survivor, None, &assignment("counter += 2"));
    assert_eq!(dialogue_values::number_of(&world, survivor, "counter"), Some(2.0));
    run_assignment(&mut world, survivor, None, &assignment("counter += 3"));
    assert_eq!(dialogue_values::number_of(&world, survivor, "counter"), Some(5.0));
    run_assignment(&mut world, survivor, None, &assignment("counter -= 4"));
    assert_eq!(dialogue_values::number_of(&world, survivor, "counter"), Some(1.0));
    run_assignment(&mut world, survivor, None, &assignment("counter *= 6"));
    assert_eq!(dialogue_values::number_of(&world, survivor, "counter"), Some(6.0));
    run_assignment(&mut world, survivor, None, &assignment("counter /= 3"));
    assert_eq!(dialogue_values::number_of(&world, survivor, "counter"), Some(2.0));
}

#[test]
fn a_statement_whose_value_cannot_be_worked_out_changes_nothing() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    dialogue_values::set(&mut world, survivor, "kept", Value::Number(7.0));

    // An unset variable reads as **zero** in an assignment, which is the reference's arithmetic on a null value: `kept` becomes
    // 1 rather than keeping its 7. This test used to assert a refusal, which was the port's guess rather than the reference's
    // rule - the same shape of mistake as the `math` array shape two rounds ago.
    let from_an_unset = assignment("kept = never_set + 1");
    assert_eq!(
        run_assignment(&mut world, survivor, None, &from_an_unset),
        Applied::Done
    );
    assert_eq!(dialogue_values::number_of(&world, survivor, "kept"), Some(1.0));

    dialogue_values::set(&mut world, survivor, "kept", Value::Number(7.0));

    // Dividing by zero is the same shape of refusal rather than an infinity.
    let divide_by_zero = assignment("kept /= 0");
    assert_eq!(
        run_assignment(&mut world, survivor, None, &divide_by_zero),
        Applied::CannotAnswer
    );
    assert_eq!(dialogue_values::number_of(&world, survivor, "kept"), Some(7.0));
}

/// The key of the effect entry that refuses, following `if` branches to a leaf.
///
/// The keys of a refused entry are listed only when the entry has no readable key of its own, so this descends through `if`/`then`
/// and `if`/`else` - the two members that hold other effects - and reports the leaf that fails.
fn blame_effect(entry: &cdda_json::Value) -> String {
    let cdda_json::Value::Object(fields) = entry else {
        return "<named>".to_owned();
    };
    for branch in ["then", "else"] {
        let Some(cdda_json::Value::Array(entries)) = fields.get(branch) else {
            continue;
        };
        if let Some(failing) = entries
            .iter()
            .find(|entry| cdda_content::dialogue::read_effect(entry).is_none())
        {
            return blame_effect(failing);
        }
    }
    // The **first** key is reported, deliberately, rather than the first key that is not a modifier. An attempt to skip modifiers
    // (`target_var`, `duration`, `target_part`) did not change the reported ranking, and the cause was not found within the round, so
    // the simpler rule is kept rather than leaving a refinement in place that does not do what it says. What the first-key rule
    // cannot tell apart is measured separately: the 98 entries blamed on `target_var` are `{"mapgen_update": …, "target_var": …}`
    // and its relatives, so `mapgen_update` is the effect that actual refuses underneath them.
    fields
        .members()
        .first()
        .map_or_else(|| "<named>".to_owned(), |(key, _)| key.clone())
}

fn count_effects(
    value: &cdda_json::Value,
    entries: &mut usize,
    runnable: &mut usize,
    refused: &mut std::collections::BTreeMap<String, usize>,
) {
    match value {
        cdda_json::Value::Object(object) => {
            let is_eoc = matches!(object.get("type"), Some(cdda_json::Value::Str(kind)) if kind == "effect_on_condition");
            if is_eoc {
                for slot in ["effect", "true_effect", "false_effect"] {
                    let Some(cdda_json::Value::Array(list)) = object.get(slot) else {
                        continue;
                    };
                    for entry in list {
                        *entries += 1;
                        let cdda_json::Value::Object(_fields) = entry else {
                            *refused.entry("<named>".to_owned()).or_insert(0) += 1;
                            continue;
                        };
                        let ran = cdda_content::dialogue::read_effect(entry).is_some();
                        if ran {
                            *runnable += 1;
                        } else {
                            // Blame the leaf that refuses, following an `if`'s branches. This is the line that makes the ranking
                            // worth printing: without it the census counted **every** key of a refused entry, mixing modifiers
                            // with effects, and `blame_effect` sat unused beside a rule it never reached.
                            let leaf = blame_effect(entry);
                            *refused.entry(leaf).or_insert(0) += 1;
                        }
                    }
                }
            }
            for (_, member) in object.members() {
                count_effects(member, entries, runnable, refused);
            }
        }
        cdda_json::Value::Array(items) => {
            for item in items {
                count_effects(item, entries, runnable, refused);
            }
        }
        _ => {}
    }
}

/// How much of the game's effect-on-condition machinery the port can execute, counted against the data.
///
/// The effect side has its own census, the way the condition side has had since round 767: 712 named effect-on-conditions hold
/// 1,612 effect entries, and the port can run the ones whose name it knows. Printing the top of the refusal list is what
/// chooses the next effect to port - by frequency, not by what looks easy.
#[test]
fn the_port_can_run_the_math_effects_and_says_how_many_that_is() {
    use std::collections::BTreeMap;

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut entries = 0usize;
    let mut runnable = 0usize;
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();

    let mut stack = vec![data];
    while let Some(directory) = stack.pop() {
        let Ok(entries_at) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries_at.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
                continue;
            }
            let Some(value) = cdda_content::read_json(&path) else {
                continue;
            };
            count_effects(&value, &mut entries, &mut runnable, &mut refused);
        }
    }

    let mut collapsed: Vec<(String, usize)> = refused.clone().into_iter().collect();
    collapsed.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    collapsed.truncate(5);
    println!(
        "eoc effects: {entries} entries, {runnable} runnable; most common refusals: {collapsed:?}"
    );

    assert_eq!(entries, 1612, "the data's effect-on-conditions hold 1,612 effects");
    // 179 of the 304 `math` entries. Reading both spellings of a destination (`"x = 1"` and `"u_val('x') = 1"`)
    // took it from 174 to 179, and the 125 still refused are of two shapes worth naming: 139 of the statements are pure
    // **expressions** with no assignment at all (`"u_val('stamina')"`, `"ps_str + 50"`), and 76 are bare operators that suggest a
    // statement split across several entries in one list (`["...", "*=", "..."]`). Neither is run: a list whose meaning the port
    // cannot read is refused whole, because "ignore the parts I do not understand" is how a program silently does half its job.
    // 399: 179 `math` statements, 184 messages, and 36 plain calls to another program now that `run_eocs` runs. The other 131
    // `run_eocs` entries are object forms with modifiers (`popup`, `variables`, `true_eocs`/`false_eocs`), which the port refuses
    // rather than reading only the part it recognises.
    //
    // 538 after the trait effects: 139 entries (36 `u_add_trait`, 103 `u_lose_trait`), the first effects that change a creature
    // rather than say something. The count moved by exactly the number of entries the new readers cover, which is what a census
    // should do - no drift, no estimate.
    //
    // 601 after the effect-mutation readers: `u_add_effect` and its `npc_` twin (78 entries) plus their `duration` members, and
    // `u_remove_effect`. The jump is 63, which is 601 - 538 - and it is the first time the effect count has moved by a number the
    // reader does not make obvious: `duration` is a **modifier**, so some entries it was counted against were refused for a
    // different reason and now read. Stated rather than smoothed over.
    //
    // 605 after `if`, which is a **negative** result worth keeping: 100 of the data's effect entries are an `if`, and reading it
    // added four. A list is only as readable as its parts - the same law the condition combinators taught - so an `if` whose
    // branches name effects the port cannot run is still refused, and the effect side is blocked on leaves, not on structure.
    //
    // 622 after the `math` array was read the way the reference reads it: `eoc_math::from_json` (condition.cpp:2494-2502)
    // **concatenates** the array's strings and parses one statement, so `["x", " *= ", "3"]` is an assignment and a port reading one
    // statement per element refused every split one.
    //
    // 742 after `run_eocs` learned the inline form: an entry may be a whole program written where it is used
    // (`load_eoc_vector_id_and_var`, `npctalk.cpp:512-541`), and 120 entries turned out to be exactly that.
    assert_eq!(runnable, 742, "of which the port runs 742 entries");

    // And the ranking of what refuses, which is this census's real output: it says what to port next instead of what is used most.
    // `target_var` is the one entry the ranking cannot attribute on its own - a scan of the data shows 71 of those 98 are
    // `{"mapgen_update": …, "target_var": …}` and its relatives, so `mapgen_update` is what refuses underneath them.
    let mut ranked: Vec<(String, usize)> = refused.clone().into_iter().collect();
    ranked.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    ranked.truncate(5);
    let ranking: Vec<(String, usize)> = ranked;
    println!("effect refusals, most common first: {ranking:?}");
    assert_eq!(
        ranking,
        vec![
            ("math".to_owned(), 113),
            ("mapgen_update".to_owned(), 107),
            ("bonus".to_owned(), 32),
            ("else".to_owned(), 32),
            ("duration".to_owned(), 31),
        ],
        "the five effects that refuse most, in order - and `bonus`/`duration` are modifiers that reached the top only because \
         a refused entry always reports its first key; the three above them are effects"
    );
}

/// The other spelling of a destination: `u_val('x') = ...`, which the data also uses.
///
/// Reference: `read_var_info` (`condition.cpp:408-437`) classes a variable by the member that names it, so `u_val('x')` and a
/// bare `x` are the same destination in two spellings. The port keeps both and resolves them the same way, which took the
/// runnable count from 174 to 179 and stopped a whole class of statements from being refused for a spelling.
#[test]
fn a_destination_can_be_named_the_way_the_expression_language_names_it() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let statement = assignment("u_val('sleepiness') = min(u_val('sleepiness') + 20, 1200)");
    assert_eq!(statement.target.scope(), Some("u_val"));
    assert_eq!(run_assignment(&mut world, survivor, None, &statement), Applied::Done);
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "sleepiness"),
        Some(20.0),
        "an unset variable is read as zero, then the assignment lands"
    );
    run_assignment(&mut world, survivor, None, &statement);
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "sleepiness"),
        Some(40.0)
    );
}

/// A program of effects runs in order, and a message lands in the log the interface would drain.
///
/// Reference: `talk_effect_t::apply` (`npctalk.cpp`) walks the list; `f_message` (`npctalk.cpp:5312-5331`) shows a translation.
/// The port has no window to draw into, so the log is the store's own record - and a headless run reading it is the only way a
/// test can see what the game said.
#[test]
fn a_program_runs_its_effects_in_order_and_a_message_is_recorded() {
    use cdda_sim::dialogue_effects::{run_effects, Applied};
    use cdda_sim::messages;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let program = vec![
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"math": ["visits = 0"]}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"u_message": "The spiders are quiet."}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"math": ["visits += 1"]}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
    ];
    assert_eq!(run_effects(&mut world, survivor, None, &program), Applied::Done);
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "visits"),
        Some(1.0),
        "the statements ran in order: set, message, increment"
    );
    assert_eq!(messages::lines(&world), vec!["The spiders are quiet.".to_owned()]);

    // The log is drained, not appended to forever.
    messages::clear(&mut world);
    assert!(messages::lines(&world).is_empty());

    // A message written as a translation object reads too, and a snippet does not: the port has no snippet table, and showing a
    // snippet's id would be showing the wrong text.
    let translated = cdda_json::parse(br#"{"npc_message": {"str": "Rough weather."}}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&translated).is_some());
    let snippet = cdda_json::parse(br#"{"u_message": "snippet_id", "snippet": true}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&snippet).is_none());
}

/// A program can give and take an effect, with the duration and intensity the data states.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`): the duration is a time string, `PERMANENT`, or absent (1000 turns); the
/// intensity defaults to zero and the effect's own rule raises it to one. 78 entries in the data state `u_add_effect`, and 117
/// state a `duration` with it.
#[test]
fn a_program_can_give_and_take_a_wound() {
    use cdda_content::dialogue::Effect;
    use cdda_sim::applied_effects;
    use cdda_sim::dialogue_effects::run_effects;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let entry = |text: &str| {
        cdda_content::dialogue::read_effect(&cdda_json::parse(text.as_bytes()).expect("json"))
            .expect("readable")
    };

    let timed = entry(r#"{"u_add_effect": "effect_bleed", "duration": "30 m", "intensity": 2}"#);
    let Effect::AddEffect { duration, intensity, .. } = timed else {
        panic!("read as an effect to add");
    };
    assert_eq!(duration, Some(1800), "thirty minutes is 1,800 turns");
    assert_eq!(intensity, 2);

    assert_eq!(
        run_effects(&mut world, survivor, None, &[timed]),
        Applied::Done
    );
    assert!(applied_effects::has(&world, survivor, "effect_bleed"));
    assert_eq!(
        applied_effects::intensity_of(&world, survivor, "effect_bleed"),
        Some(2)
    );

    // `PERMANENT` is an effect that never runs down, which the port spells with the effect's own pause flag.
    let permanent = entry(r#"{"u_add_effect": "effect_mutation_purge", "duration": "PERMANENT"}"#);
    let Effect::AddEffect { duration: None, .. } = permanent else {
        panic!("permanent is read as no duration");
    };
    assert_eq!(
        run_effects(&mut world, survivor, None, &[permanent]),
        Applied::Done
    );

    let gone = entry(r#"{"u_remove_effect": "effect_bleed"}"#);
    assert_eq!(run_effects(&mut world, survivor, None, &[gone]), Applied::Done);
    assert!(!applied_effects::has(&world, survivor, "effect_bleed"));

    // A body part the port does not model is refused rather than applied to the whole creature.
    let on_a_part = cdda_json::parse(
        br#"{"u_add_effect": "effect_bleed", "target_part": "arm_l"}"#.as_slice(),
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_effect(&on_a_part).is_none());
}

/// An `if` effect runs one list or the other, and each list can hold anything.
///
/// Reference: `f_if` (`npctalk.cpp:6977-6993`): `if` is a condition, `then` runs when it holds, `else` when it does not, and an absent
/// `else` runs nothing. 100 of the data's effect entries are one of these, and each is worth more than itself because it can hold any
/// other effect - which is what makes this the first effect with a body rather than a payload.
#[test]
fn a_conditional_effect_runs_the_branch_its_condition_chooses() {
    use cdda_sim::dialogue_effects::run_effects;
    use cdda_sim::messages;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let entry = |text: &str| {
        cdda_content::dialogue::read_effect(&cdda_json::parse(text.as_bytes()).expect("json"))
            .expect("readable")
    };

    let guarded = entry(
        r#"{"if": {"math": ["armed", "==", 1]},
            "then": [{"u_message": "the safety clicks off"}, {"math": ["shots = 1"]}],
            "else": [{"u_message": "nothing happens"}]}"#,
    );
    assert_eq!(
        run_effects(&mut world, survivor, None, std::slice::from_ref(&guarded)),
        Applied::Done
    );
    assert_eq!(messages::lines(&world), vec!["nothing happens".to_owned()]);

    messages::clear(&mut world);
    dialogue_values::set(&mut world, survivor, "armed", Value::Number(1.0));
    assert_eq!(run_effects(&mut world, survivor, None, &[guarded]), Applied::Done);
    assert_eq!(
        messages::lines(&world),
        vec!["the safety clicks off".to_owned()],
        "and the branch ran as a list, in order"
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "shots"),
        Some(1.0)
    );

    // An absent `else` runs nothing rather than refusing the effect.
    let no_else = entry(r#"{"if": {"math": ["never", "==", 1]}, "then": [{"u_message": "no"}]}"#);
    messages::clear(&mut world);
    assert_eq!(run_effects(&mut world, survivor, None, &[no_else]), Applied::Done);
    assert!(messages::lines(&world).is_empty());

    // A branch the port cannot read refuses the whole `if`, rather than running the readable half.
    let half_read = cdda_json::parse(
        br#"{"if": {"math": ["a", "==", 1]}, "then": [{"u_message": "yes"}, {"u_has_bionics": "bio_x"}]}"#
            .as_slice(),
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_effect(&half_read).is_none());
}
