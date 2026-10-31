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
    // 751 after `time( '…' )` - the first math function whose argument is **text**, and the first the port had to parse rather than
    // compute. Nine entries, and two programs.
    // 757 after `rng`/`rand`: the first math functions that **change the store** while answering, so the dice are taken out for the
    // length of the evaluation and put back.
    // 792 after location variables: the plain form stores the actor's square, and the searching forms are still refused.
    // 805 after `open_dialogue`: the branch rule runs exactly, and the conversation itself is not opened (no interface to open it).
    assert_eq!(runnable, 805, "of which the port runs 805 entries");

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
            ("mapgen_update".to_owned(), 107),
            ("math".to_owned(), 98),
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

/// `time( '…' )` answers a duration in turns, which is the one math function whose argument is text.
///
/// Reference: `math_parser_diag.cpp`'s `time` answers the duration in turns, and the data uses it 19 times in effect-on-condition
/// maths. The port refused it twice over: the parser had no way to keep a quoted literal as an argument, and the evaluator had no
/// function to hand it to.
#[test]
fn a_time_argument_answers_the_duration_in_turns() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let statement = assignment("waiting = time('1 minutes')");
    assert_eq!(
        cdda_content::constructions::parse_duration("1 minutes"),
        60,
        "a minute is sixty turns"
    );
    assert_eq!(
        run_assignment(&mut world, survivor, None, &statement),
        Applied::Done
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "waiting"),
        Some(60.0)
    );

    // The literal is text, so a function expecting a number still refuses it rather than reading it as zero.
    let misleading = cdda_json::parse(br#"{"math": ["x = abs('1 minutes')"]}"#.as_slice())
        .expect("json");
    assert!(
        cdda_content::dialogue::read_effect(&misleading).is_some(),
        "it parses - `abs` takes one argument of any shape"
    );
    assert_eq!(
        cdda_sim::dialogue_effects::run_effects(
            &mut world,
            survivor,
            None,
            &[cdda_content::dialogue::read_effect(&misleading).expect("readable")]
        ),
        Applied::CannotAnswer,
        "and refuses to evaluate, because text is not a number"
    );
}

/// A `math` statement can draw, and the draw comes from the store's own generator.
///
/// Reference: `rng( lo, hi )` is uniform and inclusive (`src/rng.cpp`), and `rand( n )` is `rng( 0, round( n ) )` (`math_parser_func.h:57-60`).
/// They are the only two math functions that change the store while answering, so the dice are taken out for the length of the
/// evaluation and put back afterwards - the same shape every other rolling rule in the port uses.
#[test]
fn a_statement_can_roll_both_ends_inclusive() {
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(7));

    // `rng(1, 3)` must be able to reach both ends, and must never leave them.
    let mut seen: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for _ in 0..200 {
        assert_eq!(
            run_assignment(&mut world, survivor, None, &assignment("roll = rng(1, 3)")),
            Applied::Done
        );
        let value = dialogue_values::number_of(&world, survivor, "roll").expect("a number");
        #[allow(clippy::cast_possible_truncation)]
        let whole = value as i32;
        assert!((1..=3).contains(&whole), "rng(1, 3) gave {whole}");
        seen.insert(whole);
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        vec![1, 2, 3],
        "and two hundred rolls reached all three answers"
    );

    // `rand(0)` is the degenerate case the reference allows: `rng(0, 0)` is always zero.
    assert_eq!(
        run_assignment(&mut world, survivor, None, &assignment("zero = rand(0)")),
        Applied::Done
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "zero"),
        Some(0.0)
    );

    // The same seed replays the same sequence, which is what taking the dice out of the store buys.
    let mut first = Vec::new();
    let mut second = Vec::new();
    for into in [&mut first, &mut second] {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(99));
        for _ in 0..5 {
            run_assignment(&mut world, survivor, None, &assignment("r = rand(100)"));
            into.push(dialogue_values::number_of(&world, survivor, "r"));
        }
    }
    assert_eq!(first, second, "a seed replays the same five draws");
}

/// A `switch` runs the **last** case whose number the value reaches.
///
/// Reference: `f_switch` (`npctalk.cpp:6998-7020`): every case is tried in the data's order and each one the value reaches replaces the chosen
/// one, so the last match runs - not the first, and not the nearest.
#[test]
fn a_switch_runs_the_last_case_the_value_reaches() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"switch": ["rung"], "cases": [
                    {"case": 0, "effect": [{"u_message": "ground"}]},
                    {"case": 1, "effect": [{"u_message": "first"}]},
                    {"case": 2, "effect": [{"u_message": "second"}]}
                ]}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");

    let climb = |world: &mut World, rung: f64| {
        cdda_sim::dialogue_values::set(
            world,
            survivor,
            "rung",
            cdda_sim::dialogue_values::Value::Number(rung),
        );
        cdda_sim::messages::clear(world);
        let effects = std::slice::from_ref(&effect);
        assert_eq!(
            cdda_sim::dialogue_effects::run_effects(world, survivor, None, effects),
            Applied::Done
        );
        cdda_sim::messages::lines(world)
    };

    assert_eq!(climb(&mut world, 0.0), vec!["ground".to_owned()], "exactly zero reaches the first case");
    assert_eq!(
        climb(&mut world, 1.0),
        vec!["first".to_owned()],
        "one reaches the first two, and the last of them runs"
    );
    assert_eq!(climb(&mut world, 7.0), vec!["second".to_owned()], "seven reaches all three");
    assert_eq!(
        climb(&mut world, -1.0),
        Vec::<String>::new(),
        "and a value below every case runs nothing at all"
    );
}

/// A location variable holds the actor's square, and the searching forms are refused.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`). The plain form is the actor's own position; the searching forms walk the overmap and
/// the map for the nearest match (`target_params`, a terrain/furniture/monster list, radii), which nothing in the port can do yet.
#[test]
fn a_location_variable_holds_the_actors_square() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (7, 9, -1));
    let other = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (3, 4, 0));

    let plain = cdda_json::parse(
        br#"{"u_location_variable": {"u_val": "here"}}"#.as_slice(),
    )
    .expect("json");
    let effect = cdda_content::dialogue::read_effect(&plain).expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, Some(other), &effect),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::dialogue_values::value_of(&world, survivor, "here"),
        Some(cdda_sim::dialogue_values::Value::Position { x: 7, y: 9, z: -1 }),
        "the speaker's own square"
    );

    // With `use_beta`-style naming the listener's square is the one stored.
    let theirs = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"npc_location_variable": {"npc_val": "there"}}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, Some(other), &theirs);
    assert_eq!(
        cdda_sim::dialogue_values::value_of(&world, other, "there"),
        Some(cdda_sim::dialogue_values::Value::Position { x: 3, y: 4, z: 0 })
    );

    // The searching forms are refused rather than answered with the actor's own square.
    for text in [
        r#"{"u_location_variable": {"u_val": "x"}, "target_params": {"om_terrain": "field"}}"#,
        r#"{"u_location_variable": {"u_val": "x"}, "terrain": "t_grass"}"#,
        r#"{"u_location_variable": {"u_val": "x"}, "min_radius": 3}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_effect(&value).is_none(),
            "{text} must be refused"
        );
    }
}

/// `weighted_list_eocs` picks a program by weight, and a weight of zero is never picked.
///
/// Reference: `f_weighted_list_eocs` (`npctalk.cpp:6950-6975`), which builds a `weighted_int_list` and picks from it - uniform over the total
/// weight, so an entry's chance is its weight against the sum.
#[test]
fn a_weighted_list_picks_by_weight_and_never_picks_nothing() {
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(5));
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"weighted_list_eocs": [
                    [{"effect": [{"u_message": "one"}]}, 1],
                    [{"effect": [{"u_message": "two"}]}, 2],
                    [{"effect": [{"u_message": "never"}]}, 0]
                ]}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");

    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for _ in 0..300 {
        cdda_sim::messages::clear(&mut world);
        assert_eq!(
            cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
            Applied::Done
        );
        let lines = cdda_sim::messages::lines(&world);
        let [line] = lines.as_slice() else {
            panic!("one message per run, got {lines:?}");
        };
        *seen.entry(line.clone()).or_insert(0) += 1;
    }
    assert_eq!(seen.get("never"), None, "a weight of zero is never picked");
    let one = seen.get("one").copied().unwrap_or(0);
    let two = seen.get("two").copied().unwrap_or(0);
    assert_eq!(one + two, 300, "and one of the two weighted entries always runs");
    assert!(
        two > one,
        "the heavier entry came up more often: {two} against {one}"
    );

    // A list whose weights are all zero refuses rather than picking nothing.
    let empty = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"weighted_list_eocs": [[{"effect": [{"u_message": "x"}]}, 0]]}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &empty),
        Applied::CannotAnswer
    );

    // The same seed replays the same picks, which is what taking the dice out of the store buys.
    let replay = || {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(5));
        let mut picks = Vec::new();
        for _ in 0..10 {
            cdda_sim::messages::clear(&mut world);
            cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect);
            picks.push(cdda_sim::messages::lines(&world));
        }
        picks
    };
    assert_eq!(replay(), replay(), "a seed replays the same ten picks");
}

/// `open_dialogue` runs the branch that says whether a conversation could open, and only the avatar can open one.
///
/// Reference: `f_open_dialogue` (`npctalk.cpp:5476-5502`): a speaker that is not the avatar runs `false_eocs` and stops; otherwise the
/// conversation opens and `true_eocs` run. The port runs the branch rule exactly and does not open a conversation - there is no dialogue
/// interface to open (P10) - which is recorded in `docs/parity.md`.
#[test]
fn opening_a_conversation_runs_the_branch_that_says_whether_it_could() {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "a bystander", (0, 0, 0));
    let programs = cdda_content::dialogue::read_program(
        &cdda_json::parse(
            br#"{"type": "effect_on_condition", "id": "EOC_OPENED",
                 "effect": [{"u_message": "opened"}]}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    let refused = cdda_content::dialogue::read_program(
        &cdda_json::parse(
            br#"{"type": "effect_on_condition", "id": "EOC_REFUSED",
                 "effect": [{"u_message": "refused"}]}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    cdda_sim::eoc_registry::set_programs(&mut world, &[programs, refused]);

    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"open_dialogue": {"topic": "TALK_TEST",
                                   "true_eocs": ["EOC_OPENED"],
                                   "false_eocs": ["EOC_REFUSED"]}}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");

    // Nobody is the avatar, so the false branch runs.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, speaker, None, &effect),
        Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world), vec!["refused".to_owned()]);

    // With the speaker as the avatar, the true branch runs instead.
    cdda_sim::messages::clear(&mut world);
    cdda_sim::missions::set_avatar(&mut world, speaker);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, speaker, None, &effect),
        Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world), vec!["opened".to_owned()]);

    // A topic on its own reads too, and runs no branch at all.
    let plain = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"open_dialogue": "TALK_TOPIC"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    cdda_sim::messages::clear(&mut world);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, speaker, None, &plain),
        Applied::Done
    );
    assert!(cdda_sim::messages::lines(&world).is_empty());
}
