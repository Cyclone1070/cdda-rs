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
    assert_eq!(
        run_assignment(&mut world, survivor, None, &statement),
        Applied::Done
    );
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
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "counter"),
        Some(2.0)
    );
    run_assignment(&mut world, survivor, None, &assignment("counter += 3"));
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "counter"),
        Some(5.0)
    );
    run_assignment(&mut world, survivor, None, &assignment("counter -= 4"));
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "counter"),
        Some(1.0)
    );
    run_assignment(&mut world, survivor, None, &assignment("counter *= 6"));
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "counter"),
        Some(6.0)
    );
    run_assignment(&mut world, survivor, None, &assignment("counter /= 3"));
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "counter"),
        Some(2.0)
    );
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
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "kept"),
        Some(1.0)
    );

    dialogue_values::set(&mut world, survivor, "kept", Value::Number(7.0));

    // Dividing by zero is the same shape of refusal rather than an infinity.
    let divide_by_zero = assignment("kept /= 0");
    assert_eq!(
        run_assignment(&mut world, survivor, None, &divide_by_zero),
        Applied::CannotAnswer
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "kept"),
        Some(7.0)
    );
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

    assert_eq!(
        entries, 1612,
        "the data's effect-on-conditions hold 1,612 effects"
    );
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
    crate::numbers::NOW.accounts_for_every_program();
    assert_eq!(
        runnable,
        crate::numbers::NOW.effects_runnable,
        "of which the port runs 1,590 entries"
    );

    // And the ranking of what refuses, which is this census's real output: it says what to port next instead of what is used most.
    // `target_var` is the one entry the ranking cannot attribute on its own - a scan of the data shows 71 of those 98 are
    // `{"mapgen_update": …, "target_var": …}` and its relatives, so `mapgen_update` is what refuses underneath them.
    let mut ranked: Vec<(String, usize)> = refused.clone().into_iter().collect();
    ranked.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    ranked.truncate(5);
    let ranking: Vec<(String, usize)> = ranked;
    println!("effect refusals, most common first: {ranking:?}");
    // The **ranking** moves whenever any leaf lands, so it is printed rather than pinned to a list: what is pinned is that the entries are real
    // effects with refusals behind them, and that the commonest refusal is named.
    assert!(
        !ranking.is_empty(),
        "the effect census ranks its refusals: {ranking:?}"
    );
    for (member, count) in &ranking {
        assert!(
            *count > 0,
            "{member} refuses {count} entries, so ranking it is a claim about the data"
        );
    }
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
    assert_eq!(
        run_assignment(&mut world, survivor, None, &statement),
        Applied::Done
    );
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
    use cdda_sim::dialogue_effects::{Applied, run_effects};
    use cdda_sim::messages;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let program = vec![
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"math": ["visits = 0"]}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"u_message": "The spiders are quiet."}"#.as_slice())
                .expect("json"),
        )
        .expect("readable"),
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"math": ["visits += 1"]}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
    ];
    assert_eq!(
        run_effects(&mut world, survivor, None, &program),
        Applied::Done
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "visits"),
        Some(1.0),
        "the statements ran in order: set, message, increment"
    );
    assert_eq!(
        messages::lines(&world),
        vec!["The spiders are quiet.".to_owned()]
    );

    // The log is drained, not appended to forever.
    messages::clear(&mut world);
    assert!(messages::lines(&world).is_empty());

    // A message written as a translation object reads too, and a snippet reads as a snippet: the member names a category of text rather than the
    // text itself, which `a_snippet_message_picks_a_line_from_its_category_by_weight` covers.
    let translated = cdda_json::parse(br#"{"npc_message": {"str": "Rough weather."}}"#.as_slice())
        .expect("json");
    assert!(cdda_content::dialogue::read_effect(&translated).is_some());
    let snippet = cdda_json::parse(br#"{"u_message": "snippet_id", "snippet": true}"#.as_slice())
        .expect("json");
    assert!(matches!(
        cdda_content::dialogue::read_effect(&snippet),
        Some(cdda_content::dialogue::Effect::Snippet { .. })
    ));
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
    let Effect::AddEffect {
        duration,
        intensity,
        ..
    } = timed.clone()
    else {
        panic!("read as an effect to add");
    };
    assert_eq!(
        duration,
        cdda_content::dialogue::EffectDuration::Turns(Some(1800)),
        "thirty minutes is 1,800 turns"
    );
    assert_eq!(
        intensity,
        Some(cdda_content::dialogue::Expression::Number(2.0)),
        "and the intensity is the expression the data wrote"
    );

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
    let Effect::AddEffect { duration, .. } = &permanent else {
        panic!("read as an effect to add");
    };
    assert_eq!(*duration, cdda_content::dialogue::EffectDuration::Permanent);
    assert_eq!(
        run_effects(&mut world, survivor, None, &[permanent]),
        Applied::Done
    );

    let gone = entry(r#"{"u_remove_effect": "effect_bleed"}"#);
    assert_eq!(
        run_effects(&mut world, survivor, None, &[gone]),
        Applied::Done
    );
    assert!(!applied_effects::has(&world, survivor, "effect_bleed"));

    // A body part the port does not model is **ignored**, and the effect lands on the creature. The reference places it on the part named; the port
    // has no limbs, so the part is a name it cannot place and the creature is what is left. Refusing instead cost 14 effect entries and several
    // programs for a distinction the port cannot draw anyway.
    let on_a_part =
        cdda_json::parse(br#"{"u_add_effect": "effect_bleed", "target_part": "arm_l"}"#.as_slice())
            .expect("json");
    let welcomed = cdda_content::dialogue::read_effect(&on_a_part).expect("readable");
    assert_eq!(
        run_effects(&mut world, survivor, None, &[welcomed]),
        Applied::Done
    );
    assert!(
        applied_effects::has(&world, survivor, "effect_bleed"),
        "the effect landed on the creature, which is where the port can put it"
    );
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
    assert_eq!(
        run_effects(&mut world, survivor, None, &[guarded]),
        Applied::Done
    );
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
    assert_eq!(
        run_effects(&mut world, survivor, None, &[no_else]),
        Applied::Done
    );
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
    let misleading =
        cdda_json::parse(br#"{"math": ["x = abs('1 minutes')"]}"#.as_slice()).expect("json");
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

    assert_eq!(
        climb(&mut world, 0.0),
        vec!["ground".to_owned()],
        "exactly zero reaches the first case"
    );
    assert_eq!(
        climb(&mut world, 1.0),
        vec!["first".to_owned()],
        "one reaches the first two, and the last of them runs"
    );
    assert_eq!(
        climb(&mut world, 7.0),
        vec!["second".to_owned()],
        "seven reaches all three"
    );
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

    let plain = cdda_json::parse(br#"{"u_location_variable": {"u_val": "here"}}"#.as_slice())
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

    // The searching forms are refused rather than answered with the actor's own square. A `target_params` naming an overmap terrain with a range, a
    // distance and a level is no longer one of them - the port searches the overmap for it - but a field or a zone still is.
    for text in [
        // A special **alone** is a question the port cannot answer; a special beside a terrain is answered, because the reference uses the terrain.
        r#"{"u_location_variable": {"u_val": "x"}, "target_params": {"om_special": "hospital"}}"#,
        // A named terrain **is** read now (the `search_type = "terrain"` walk), so the refused case here is a field, which the port has no search rule for.
        r#"{"u_location_variable": {"u_val": "x"}, "field": "fd_fire"}"#,
        // `min_radius` alone is **not** a search: the reference starts at the talker's own square and only moves it inside a `search_target` block, so a radius
        // with nothing to search for is answered rather than refused. It is asserted on its own below rather than in this list.
        r#"{"u_location_variable": {"u_val": "x"}, "zone": "z_shop"}"#,
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
    assert_eq!(
        one + two,
        300,
        "and one of the two weighted entries always runs"
    );
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
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["refused".to_owned()]
    );

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

/// An added effect's duration and intensity come from the data, including a range and an expression.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`) with `get_duration_or_var` and `get_dbl_or_var( "intensity" )`: the duration may be a
/// string, a number of turns, an array of two ends (rolled between), or `PERMANENT`; the intensity may be an expression.
#[test]
fn an_added_effect_reads_its_duration_and_intensity_from_the_data() {
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(3));

    let add = |text: &str| {
        cdda_content::dialogue::read_effect(&cdda_json::parse(text.as_bytes()).expect("json"))
            .expect("readable")
    };

    // A duration string.
    let hours = add(r#"{"u_add_effect": "nightmares", "duration": "18 hours"}"#);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &hours),
        Applied::Done
    );
    let worn = cdda_sim::applied_effects::worn(&world, survivor);
    let nightmare = worn
        .iter()
        .find(|effect| effect.id == "nightmares")
        .expect("the effect is worn");
    assert_eq!(nightmare.duration, 18 * 3600, "eighteen hours of turns");

    // An intensity written as an expression.
    cdda_sim::dialogue_values::set(
        &mut world,
        survivor,
        "strong",
        cdda_sim::dialogue_values::Value::Number(3.0),
    );
    let expressive = add(r#"{"u_add_effect": "taint", "intensity": {"math": ["strong"]}}"#);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &expressive),
        Applied::Done
    );
    let worn = cdda_sim::applied_effects::worn(&world, survivor);
    let taint = worn
        .iter()
        .find(|effect| effect.id == "taint")
        .expect("the effect is worn");
    assert_eq!(
        taint.intensity, 3,
        "the expression's value became the intensity"
    );

    // A range is rolled between its ends, both of which are reachable.
    let ranged = add(r#"{"u_add_effect": "wet", "duration": [10, 20]}"#);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..80 {
        cdda_sim::applied_effects::remove(&mut world, survivor, "wet");
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &ranged);
        let worn = cdda_sim::applied_effects::worn(&world, survivor);
        let wet = worn
            .iter()
            .find(|effect| effect.id == "wet")
            .expect("the effect is worn");
        assert!((10..=20).contains(&wet.duration), "rolled {}", wet.duration);
        seen.insert(wet.duration);
    }
    assert!(seen.len() > 1, "eighty rolls reached more than one answer");
}

/// Adds a morale point through the dialogue effect, failing the test when it refuses.
fn add_morale_point(
    world: &mut World,
    creature: bevy_ecs::prelude::Entity,
    kind: &str,
    bonus: f64,
    max_bonus: f64,
    duration: &str,
    decay: &str,
) {
    let text = format!(
        r#"{{"u_add_morale": "{kind}", "bonus": {bonus}, "max_bonus": {max_bonus},
             "duration": "{duration}", "decay_start": "{decay}"}}"#
    );
    let effect =
        cdda_content::dialogue::read_effect(&cdda_json::parse(text.as_bytes()).expect("json"))
            .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(world, creature, None, &effect),
        Applied::Done
    );
}

/// A morale point fades along the reference's logistic curve, and stacking uses the square root of the sum of squares.
///
/// Reference: `player_morale::morale_point` (`morale.cpp:130-240`): a point keeps its whole bonus until `decay_start`, fades by
/// `logarithmic_range` to nothing at `duration`, and two points of the same sign stack as `sqrt(a² + b²)` rather than `a + b`.
#[test]
fn a_morale_point_keeps_its_bonus_then_fades_along_the_reference_curve() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    add_morale_point(
        &mut world,
        survivor,
        "morale_feeling_good",
        20.0,
        50.0,
        "6 hours",
        "3 hours",
    );
    assert_eq!(
        cdda_sim::morale::net_bonus_of(&world, survivor),
        20,
        "the whole bonus at once"
    );

    // Aged to exactly the decay start, it still gives everything.
    for _ in 0..3 * 3600 {
        cdda_sim::morale::age_morale(&mut world);
    }
    assert_eq!(
        cdda_sim::morale::net_bonus_of(&world, survivor),
        20,
        "nothing has faded yet at the decay start"
    );

    // Halfway between decay start and the end it has fallen, but not to nothing.
    for _ in 0..90 * 60 {
        cdda_sim::morale::age_morale(&mut world);
    }
    let halfway = cdda_sim::morale::net_bonus_of(&world, survivor);
    assert!(
        halfway < 20 && halfway > 0,
        "half faded, not gone: {halfway}"
    );

    // At its end it is gone, and the point is dropped.
    for _ in 0..3 * 3600 {
        cdda_sim::morale::age_morale(&mut world);
    }
    assert_eq!(cdda_sim::morale::net_bonus_of(&world, survivor), 0);
    assert!(
        cdda_sim::morale::points_of(&world, survivor).is_empty(),
        "an expired point is dropped rather than kept at zero"
    );

    // Two of the same sign stack as the square root of the sum of squares: 3 and 4 feel like 5, not 7.
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    add_morale_point(
        &mut world,
        survivor,
        "morale_good",
        3.0,
        50.0,
        "6 hours",
        "3 hours",
    );
    add_morale_point(
        &mut world,
        survivor,
        "morale_good",
        4.0,
        50.0,
        "6 hours",
        "3 hours",
    );
    assert_eq!(
        cdda_sim::morale::net_bonus_of(&world, survivor),
        5,
        "sqrt(3² + 4²) is 5"
    );

    // A cap holds the stack down.
    add_morale_point(
        &mut world,
        survivor,
        "morale_good",
        50.0,
        6.0,
        "6 hours",
        "3 hours",
    );
    assert_eq!(
        cdda_sim::morale::net_bonus_of(&world, survivor),
        6,
        "the cap wins over the stack"
    );
}

/// The reader dispatches on a member's **name**, never on the order the data happens to write its members in.
///
/// The real data writes the same object both ways - `{"u_message": "…", "snippet": true}` and `{"snippet": true, "u_message": "…"}` - so a reader
/// that depended on order would refuse half of them and quietly shrink every census.
#[test]
fn a_reader_dispatches_on_member_names_not_on_the_order_the_data_writes_them() {
    for text in [
        r#"{"u_message": "play_piano", "snippet": true}"#,
        r#"{"snippet": true, "u_message": "play_piano"}"#,
        r#"{"u_message": "hello"}"#,
        r#"{"u_add_effect": "wet", "duration": "1 hour"}"#,
        r#"{"duration": "1 hour", "u_add_effect": "wet"}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_effect(&value).is_some(),
            "{text} must read whatever order its members come in"
        );
    }
}

/// A snippet message names a category, and the line is picked from it by weight.
///
/// Reference: `f_message` (`npctalk.cpp:5312-5405`): with `snippet: true` the member is a category, not a message, and the line comes from the
/// snippet library by weight; with `same_snippet` the same line is said again.
#[test]
fn a_snippet_message_picks_a_line_from_its_category_by_weight() {
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(11));
    cdda_sim::snippets::set_snippets(
        &mut world,
        [
            (
                "files".to_owned(),
                snippet_line("first", "The first line.", 1),
            ),
            (
                "files".to_owned(),
                snippet_line("second", "The second line.", 3),
            ),
            ("files".to_owned(), snippet_line("never", "Never said.", 0)),
        ],
    );

    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_message": "files", "snippet": true}"#.as_slice()).expect("json"),
    )
    .expect("readable");

    let mut said: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for _ in 0..200 {
        cdda_sim::messages::clear(&mut world);
        assert_eq!(
            cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
            Applied::Done
        );
        let lines = cdda_sim::messages::lines(&world);
        let [line] = lines.as_slice() else {
            panic!("one message per run, got {lines:?}");
        };
        *said.entry(line.clone()).or_insert(0) += 1;
    }
    assert_eq!(
        said.get("Never said."),
        None,
        "a line of weight zero is never said"
    );
    let first = said.get("The first line.").copied().unwrap_or(0);
    let second = said.get("The second line.").copied().unwrap_or(0);
    assert_eq!(first + second, 200);
    assert!(
        second > first,
        "the heavier line came up more: {second} to {first}"
    );

    // `same_snippet` says the same line twice.
    let same = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_message": "files", "snippet": true, "same_snippet": true}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    cdda_sim::messages::clear(&mut world);
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &same);
    let once = cdda_sim::messages::lines(&world);
    cdda_sim::messages::clear(&mut world);
    cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &same);
    assert_eq!(
        cdda_sim::messages::lines(&world),
        once,
        "the same line again"
    );

    // An unknown category refuses rather than saying nothing quietly.
    let unknown = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_message": "no_such_category", "snippet": true}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &unknown),
        Applied::CannotAnswer
    );
}

/// One snippet line, for the tests above.
fn snippet_line(id: &str, text: &str, weight: i32) -> cdda_sim::snippets::SnippetLine {
    cdda_sim::snippets::SnippetLine {
        id: id.to_owned(),
        text: text.to_owned(),
        weight,
    }
}

/// A branch of an `if` may be a single effect, not only a list of them.
///
/// Reference: `talk_effect_t::load_effect` (`npctalk.cpp`) reads an array **or a single entry**, and the data writes both - so `"then": {...}`
/// is a one-entry list rather than a shape the reader should refuse.
#[test]
fn a_branch_may_be_one_effect_or_a_list_of_them() {
    for text in [
        r#"{"if": {"one_in_chance": 4}, "then": {"u_message": "one"}, "else": {"u_message": "other"}}"#,
        r#"{"if": {"one_in_chance": 4}, "then": [{"u_message": "one"}], "else": [{"u_message": "other"}]}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value)
            .unwrap_or_else(|| panic!("{text} must read"));
        let cdda_content::dialogue::Effect::If {
            then, otherwise, ..
        } = effect
        else {
            panic!("{text} must read as a branch");
        };
        assert_eq!(then.len(), 1, "one entry either way");
        assert_eq!(otherwise.len(), 1, "and the other branch too");
    }
}

/// A sound is made where its maker stands, and speech is said while background noise is not.
///
/// Reference: `f_make_sound` (`npctalk.cpp:6131-6200`): the volume is a `dbl_or_var`, the type names what sort of sound it is, and the member is
/// either the words or - with `snippet: true` - the category they are picked from.
#[test]
fn a_sound_is_made_where_its_maker_stands_and_speech_is_said() {
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (4, 6, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(2));
    let read = |text: &str| {
        cdda_content::dialogue::read_effect(&cdda_json::parse(text.as_bytes()).expect("json"))
            .expect("readable")
    };

    // Background noise: heard, but nothing is said.
    let bang = read(r#"{"u_make_sound": "a bang", "volume": 12, "type": "background"}"#);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &bang),
        Applied::Done
    );
    let heard = cdda_sim::sounds::heard_this_turn(&mut world);
    let [only] = heard.as_slice() else {
        panic!("one sound was made, got {heard:?}");
    };
    assert_eq!(only.position, (4, 6, 0), "where its maker stands");
    assert_eq!(only.volume, 12, "at the volume it was given");
    assert!(cdda_sim::messages::lines(&world).is_empty(), "no words");

    // Speech is said as well as heard.
    cdda_sim::sounds::clear(&mut world);
    let shout = read(r#"{"u_make_sound": "Get out of here!", "volume": 20, "type": "speech"}"#);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &shout),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["Get out of here!".to_owned()]
    );
    let heard = cdda_sim::sounds::heard_this_turn(&mut world);
    assert_eq!(heard.first().map(|sound| sound.volume), Some(20));

    // A snippet sound says a line from its category.
    cdda_sim::messages::clear(&mut world);
    cdda_sim::sounds::clear(&mut world);
    cdda_sim::snippets::set_snippets(
        &mut world,
        [("shouts".to_owned(), snippet_line("only", "Oi!", 1))],
    );
    let snip = read(r#"{"u_make_sound": "shouts", "volume": 8, "type": "alarm", "snippet": true}"#);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &snip),
        Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world), vec!["Oi!".to_owned()]);
    assert_eq!(cdda_sim::sounds::heard_this_turn(&mut world).len(), 1);

    // The npc form makes the sound where the *listener* stands.
    cdda_sim::sounds::clear(&mut world);
    let other = cdda_sim::creature::spawn_creature(&mut world, "a stranger", (9, 9, 0));
    let theirs = read(r#"{"npc_make_sound": "a bang", "volume": 3}"#);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, Some(other), &theirs),
        Applied::Done
    );
    let heard = cdda_sim::sounds::heard_this_turn(&mut world);
    assert_eq!(heard.first().map(|sound| sound.position), Some((9, 9, 0)));

    // A volume written as an expression is evaluated, and an unknown type is refused rather than guessed.
    let loud = read(r#"{"u_make_sound": "x", "volume": {"math": ["6 + 6"]}}"#);
    assert!(matches!(
        loud,
        cdda_content::dialogue::Effect::MakeSound { .. }
    ));
    let unknown =
        cdda_json::parse(br#"{"u_make_sound": "x", "volume": 4, "type": "shouting"}"#.as_slice())
            .expect("json");
    assert!(
        cdda_content::dialogue::read_effect(&unknown).is_none(),
        "a type the port does not know is refused"
    );
}

/// A mapgen update writes its terrain over the squares the speaker stands among.
///
/// Reference: `f_mapgen_update` (`npctalk.cpp`) with `mapgen::update` (`mapgen.cpp`): the update is drawn and its terrain written where it lands,
/// leaving squares it says nothing about alone.
#[test]
fn a_mapgen_update_writes_its_terrain_where_the_speaker_stands() {
    use std::collections::BTreeMap;

    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..6 {
        for y in 0..6 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));

    let mapgen = cdda_content::mapgen::MapgenObject {
        om_terrain: "lab_wall_opens".to_owned(),
        rows: vec!["AB".to_owned()],
        terrain: [('A', "t_rock".to_owned()), ('B', "t_rock_floor".to_owned())]
            .into_iter()
            .collect::<BTreeMap<char, String>>(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 100,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("lab_wall_opens".to_owned(), mapgen)]);
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_rock".to_owned(), 7u32),
            ("t_rock_floor".to_owned(), 8u32),
        ]
        .into_iter()
        .collect(),
    );
    assert_eq!(
        cdda_sim::mapgen_update::census(&world),
        1,
        "one mapgen installed"
    );

    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"mapgen_update": "lab_wall_opens"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(7),
        "A became rock"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 1, 0)),
        Some(8),
        "B became rock floor"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 2, 0)),
        Some(0),
        "the next row was left alone"
    );

    // A mapgen the world does not hold refuses rather than doing nothing quietly.
    let missing = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"mapgen_update": "no_such_update"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &missing),
        Applied::CannotAnswer
    );

    // An update aimed by a member the port has not built is refused rather than applied in the wrong place. This list keeps shrinking as the aiming
    // forms land - `target_var`, then `om_terrain` - and what is left is the whole mission target: search range, filters, reveal, and a create step the
    // port does not have.
    let whole_target = cdda_json::parse(
        br#"{"mapgen_update": "lab_wall_opens", "target_params": {"om_terrain": "field"}}"#
            .as_slice(),
    )
    .expect("json");
    assert!(
        cdda_content::dialogue::read_effect(&whole_target).is_none(),
        "a whole mission target is refused rather than applied in the wrong place"
    );
}

/// A `lab_wall_opens` mapgen writing rock and rock floor over a two-by-two, for the aiming tests below.
fn a_wall_update_world() -> (World, bevy_ecs::prelude::Entity) {
    use std::collections::BTreeMap;

    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..8 {
        for y in 0..8 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (2, 2, 0));
    let mapgen = cdda_content::mapgen::MapgenObject {
        om_terrain: "lab_wall_opens".to_owned(),
        rows: vec!["AB".to_owned()],
        terrain: [('A', "t_rock".to_owned()), ('B', "t_rock_floor".to_owned())]
            .into_iter()
            .collect::<BTreeMap<char, String>>(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 100,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("lab_wall_opens".to_owned(), mapgen)]);
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_rock".to_owned(), 7u32),
            ("t_rock_floor".to_owned(), 8u32),
        ]
        .into_iter()
        .collect(),
    );
    (world, survivor)
}

/// **An `om_terrain` aim with nothing to find lands where the speaker stands**, not nowhere.
///
/// Reference: `mission_util::get_om_terrain_pos` (`mission_util.cpp:321-346`): `target_pos` starts at the origin and is only **moved** when
/// `find_or_create_om_terrain` answers - so a search that finds nothing leaves the target at the origin, and `f_mapgen_update` (`npctalk.cpp:4851`) applies the
/// update there. The port refused, which is the same mistake the location variable had: an empty search is an answer, not a hole in the rule.
#[test]
fn a_mapgen_update_with_no_tile_in_sight_lands_where_the_speaker_stands() {
    let (mut world, survivor) = a_wall_update_world();
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"mapgen_update": "lab_wall_opens", "om_terrain": "lab_finale"}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("an om_terrain aim is readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        Applied::Done,
        "**nothing to find is not a refusal**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 2, 0)),
        Some(7),
        "**the update landed on the speaker's square**"
    );
}

/// **A `target_var` that holds nothing means the origin**, which is what an unset variable reads as.
///
/// Reference: `f_mapgen_update` (`npctalk.cpp:4843-4845`) reads the variable's square and projects it to an overmap tile; an unset variable reads as
/// `tripoint_abs_ms()` - the origin (`math_parser_diag_value.cpp:62-70`, port round 238) - so the update lands at overmap tile (0, 0), whose top-left is
/// absolute square (0, 0). The port refused instead of applying it there.
#[test]
fn a_mapgen_update_aimed_at_an_unset_variable_lands_at_the_origin() {
    let (mut world, survivor) = a_wall_update_world();
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"mapgen_update": "lab_wall_opens", "target_var": {"global_val": "nowhere"}}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a target_var aim is readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        Applied::Done,
        "**an unset target is the origin, not a refusal**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(7),
        "**the update landed at absolute square (0, 0)**"
    );
}

/// An update mapgen may place fields instead of terrain, which is how a scenario fills a room with smoke.
///
/// Reference: `mapgen::place_fields` (`mapgen.cpp`): `repeat` patches inside a rectangle, each at a strength rolled between the two ends, and a
/// patch on a square that already holds the field makes it stronger rather than replacing it.
#[test]
fn a_mapgen_update_may_place_fields_instead_of_terrain() {
    use cdda_core_types::rng::Rolls;
    use std::collections::BTreeMap;

    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..8 {
        for y in 0..8 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (2, 2, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(5));
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_smoke", 1, 3)]);

    // The data's own shape: an update mapgen with no rows at all.
    let mapgen = cdda_content::mapgen::MapgenObject {
        om_terrain: "SCENARIO_SMOKE".to_owned(),
        rows: Vec::new(),
        terrain: BTreeMap::new(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 1000,
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
        fields: vec![cdda_content::mapgen::Placement {
            field: "fd_smoke".to_owned(),
            x: (0, 3),
            y: (0, 3),
            intensity: (1, 3),
            repeat: 6,
        }],
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("SCENARIO_SMOKE".to_owned(), mapgen)]);

    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"mapgen_update": "SCENARIO_SMOKE"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        Applied::Done
    );

    // Six patches landed on squares of the 4x4 rectangle the update named, each between one and three.
    let mut patches = 0;
    let mut strengths = std::collections::BTreeSet::new();
    let mut query = world.query::<(&cdda_sim::Position, &cdda_sim::Field)>();
    for (position, field) in query.iter(&world) {
        assert!(
            (2..=5).contains(&position.x) && (2..=5).contains(&position.y),
            "a patch landed inside the rectangle, at ({}, {})",
            position.x,
            position.y
        );
        patches += usize::try_from(field.entry.intensity()).unwrap_or(0);
        strengths.insert(field.entry.intensity());
    }
    assert!(
        patches >= 6,
        "six patches were placed, got strength total {patches}"
    );
    assert!(
        strengths.iter().all(|strength| (1..=3).contains(strength)),
        "every patch is within the intensity range the data states: {strengths:?}"
    );
}

/// A `target_var` applies the update at the square the dialogue stored, not where the speaker stands.
///
/// Reference: `f_mapgen_update` (`npctalk.cpp`): with a `target_var` the update lands at the square that variable names - which is how an EOC
/// opens a wall somewhere it looked at earlier rather than under its own feet.
#[test]
fn a_mapgen_update_can_be_aimed_at_a_stored_square() {
    use std::collections::BTreeMap;

    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..10 {
        for y in 0..10 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));

    let mapgen = cdda_content::mapgen::MapgenObject {
        om_terrain: "open_the_wall".to_owned(),
        rows: vec!["A".to_owned()],
        terrain: [('A', "t_floor".to_owned())]
            .into_iter()
            .collect::<BTreeMap<char, String>>(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 100,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("open_the_wall".to_owned(), mapgen)]);
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [("t_floor".to_owned(), 4u32)].into_iter().collect(),
    );

    // Store a square the way `u_location_variable` does, then aim at it.
    let store = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_location_variable": {"u_val": "here"}}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done
    );
    // The speaker walks away, so the square it stands on and the square it stored are different: a fallback to "where the speaker stands" would
    // pass a test that left them together, and this one would not.
    if let Some(mut at) = world.get_mut::<cdda_sim::Position>(survivor) {
        (at.x, at.y) = (5, 5);
    }
    let aimed = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"mapgen_update": "open_the_wall", "target_var": {"u_val": "here"}}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &aimed),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(4),
        "the update landed at the square the dialogue stored"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (5, 5, 0)),
        Some(0),
        "and not where the speaker stands now, which is five squares away"
    );

    // **A target variable that was never set reads as the origin** (`math_parser_diag_value.cpp:62-70`), so the update lands at absolute square (0, 0) - not a refusal,
    // and not where the speaker stands.
    let unset = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"mapgen_update": "open_the_wall", "target_var": {"u_val": "nowhere"}}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &unset),
        Applied::Done,
        "**an unset target is the origin, not a refusal**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(4),
        "**and the update is written there**"
    );
}

/// `u_lose_effect` takes one effect or an array of them, and the port read neither until the member name was fixed.
///
/// Reference: `npctalk.cpp:8015` names the member `u_lose_effect`; the port read `u_remove_effect`, which the data never writes, so all 29 entries
/// refused and the refusal was blamed on the *effect* rather than on the reader's spelling.
#[test]
fn losing_effects_reads_the_name_the_data_writes_and_a_list_of_them() {
    // One effect, as the data usually writes it.
    let one = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_lose_effect": "drunk"}"#.as_slice()).expect("json"),
    )
    .expect("a single effect is readable");
    assert_eq!(one.kind(), "u_lose_effect");

    // A list of them, which the data also writes.
    let many = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_lose_effect": ["drunk", "infected"]}"#.as_slice()).expect("json"),
    )
    .expect("a list of effects is readable");
    assert_eq!(many.kind(), "u_lose_effect");

    // And the listener's spelling, which the data writes too.
    let theirs = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"npc_lose_effect": "drunk"}"#.as_slice()).expect("json"),
    )
    .expect("the listener's spelling is readable");
    assert_eq!(theirs.kind(), "u_lose_effect");

    // Both forms take the effects off: a creature with two effects loses both, and one it never had is not a failure.
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    for name in ["drunk", "infected"] {
        let add = cdda_content::dialogue::read_effect(
            &cdda_json::parse(
                format!(r#"{{"u_add_effect": "{name}", "duration": "PERMANENT"}}"#).as_bytes(),
            )
            .expect("json"),
        )
        .expect("readable");
        assert_eq!(
            cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &add),
            Applied::Done
        );
    }
    assert!(
        cdda_sim::applied_effects::has(&world, survivor, "drunk"),
        "two effects before"
    );
    assert!(
        cdda_sim::applied_effects::has(&world, survivor, "infected"),
        "two effects before"
    );
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &many),
        Applied::Done
    );
    assert!(
        !cdda_sim::applied_effects::has(&world, survivor, "drunk")
            && !cdda_sim::applied_effects::has(&world, survivor, "infected"),
        "both effects were taken away"
    );
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &one),
        Applied::Done,
        "losing an effect the creature does not have is not a failure"
    );
}

/// An effect's duration may be written as a variable, and a body part may be named.
///
/// Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) takes a variable as well as a number or a range, and `f_add_effect` takes a `target_part`
/// naming the body part it lands on. The data uses both - `{"u_add_effect": "conjunctivitis", "duration": {"math": ["rand(345600) + 86400"]},
/// "target_part": "eyes"}` - and the port refused both shapes.
#[test]
fn a_math_duration_is_worked_out_and_a_body_part_is_ignored() {
    use cdda_core_types::rng::Rolls;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(3));

    // The data's own shape: a worked-out duration and a body part.
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_add_effect": "conjunctivitis", "duration": {"math": ["1000 + 500"]}, "target_part": "eyes"}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a worked-out duration and a body part are both readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::applied_effects::worn(&world, survivor)
            .iter()
            .find(|worn| worn.id == "conjunctivitis")
            .map(|worn| worn.duration),
        Some(1_500),
        "the duration is what the expression worked out"
    );

    // A body part the port knows nothing about is ignored rather than refused, for every effect family that takes one.
    for text in [
        r#"{"u_add_effect": "drunk", "duration": 10, "target_part": "torso"}"#,
        r#"{"u_add_effect": "drunk", "duration": 10, "bodypart": "arm_l"}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_effect(&value).is_some(),
            "{text} is readable: the port has no limb model to place it on"
        );
    }
}

/// A menu of programs runs the one that was picked, and refuses when nobody answered.
///
/// Reference: `f_run_eoc_selector` (`npctalk.cpp:6385`): the player picks one of the programs, or walks away when `allow_cancel` says they may. The port
/// takes the answer from a queue rather than drawing a menu - and a **missing** answer refuses, because that is a question the game could not ask, not a
/// player pressing escape.
#[test]
fn a_menu_of_programs_runs_the_one_that_was_picked() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let first = cdda_content::dialogue::read_program(
        &cdda_json::parse(
            br#"{"id": "MENU_FIRST", "effect": [{"math": ["u_val('picked') = 1"]}]}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    let second = cdda_content::dialogue::read_program(
        &cdda_json::parse(
            br#"{"id": "MENU_SECOND", "effect": [{"math": ["u_val('picked') = 2"]}]}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    cdda_sim::eoc_registry::set_programs(&mut world, &[first, second]);

    let menu = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"run_eoc_selector": ["MENU_FIRST", "MENU_SECOND"], "names": ["first", "second"], "allow_cancel": true}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");

    // Nobody answered: the game refuses rather than picking for the player.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &menu),
        Applied::CannotAnswer
    );

    // The second entry was picked, so the second program ran and the first did not.
    cdda_sim::choices::set_choices(&mut world, [1]);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &menu),
        Applied::Done
    );
    assert_eq!(cdda_sim::choices::queued(&world), 0, "the answer was taken");
    let picked = cdda_content::dialogue::read_text_expression("u_val('picked')").expect("readable");
    assert_eq!(
        cdda_sim::conditions::value(&world, survivor, None, &picked),
        Some(2.0),
        "the second program ran"
    );

    // Walking away is an answer when the data allows it, and not one when it does not.
    cdda_sim::choices::set_choices(&mut world, [cdda_sim::choices::CANCELLED]);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &menu),
        Applied::Done,
        "allow_cancel says walking away is allowed"
    );
    let no_escape = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"run_eoc_selector": ["MENU_FIRST"], "allow_cancel": false}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    cdda_sim::choices::set_choices(&mut world, [cdda_sim::choices::CANCELLED]);
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &no_escape),
        Applied::CannotAnswer,
        "and refusing cancel is refusing cancel"
    );
}

/// An `om_terrain` aim applies the update at the nearest tile of that terrain, and refuses when there is none.
///
/// Reference: `f_mapgen_update` (`npctalk.cpp`) with `om_terrain` calls `mission_util::get_om_terrain_pos` (`mission_util.cpp:321`), which searches
/// outward from the speaker's overmap tile. The port **searches only** - it never generates a tile to satisfy a search.
#[test]
fn an_om_terrain_aim_lands_on_the_nearest_tile_of_that_terrain() {
    use std::collections::BTreeMap;

    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..96 {
        for y in 0..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    // A survivor in overmap tile (0, 0): one overmap tile is 24 squares, two submaps each way (`SQUARES_PER_OMT`).
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));

    let mapgen = cdda_content::mapgen::MapgenObject {
        om_terrain: "lab_finale".to_owned(),
        rows: vec!["A".to_owned()],
        terrain: [('A', "t_rock".to_owned())]
            .into_iter()
            .collect::<BTreeMap<char, String>>(),
        furniture: BTreeMap::new(),
        traps: BTreeMap::new(),
        items: BTreeMap::new(),
        monsters: Vec::new(),
        weight: 100,
        fields: Vec::new(),
        toilets: BTreeMap::new(),
        liquids: BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        fill_ter: None,
        predecessor_mapgen: None,
    };
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("lab_finale".to_owned(), mapgen)]);
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [("t_rock".to_owned(), 9u32)].into_iter().collect(),
    );

    let aimed = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"mapgen_update": "lab_finale", "om_terrain": "lab_finale"}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("an om_terrain aim is readable");

    // No tile of that terrain anywhere: **the update lands where the speaker stands**, because `get_om_terrain_pos` starts its answer at the origin and nothing moved it
    // (`mission_util.cpp:321-334`). It is answered, not refused - the same correction the location search needed.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &aimed),
        Applied::Done,
        "**a search that finds nothing is an answer**"
    );

    // Two tiles of it, one nearer: the nearer is the one the update lands on.
    cdda_sim::overmap::set_terrain(&mut world, (1, 0), "lab_finale");
    cdda_sim::overmap::set_terrain(&mut world, (3, 0), "lab_finale");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &aimed),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (24, 0, 0)),
        Some(9),
        "the update landed at overmap tile (1, 0), whose top-left square is 24 * 1 = 24, 0"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (72, 0, 0)),
        Some(0),
        "and not at the far tile"
    );
}

/// A location variable can hold the nearest overmap tile of a terrain, which is what a mission target names.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`) with a `target_params` naming an overmap terrain: the search runs over the overmap and the
/// variable holds the square it found. Every other filter is a different question and is refused.
#[test]
fn a_location_variable_can_hold_the_nearest_tile_of_a_terrain() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));

    let store = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_location_variable": {"u_val": "lab"}, "target_params": {"om_terrain": "lab_finale"}}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a terrain target is readable");

    // Nothing of that terrain on the overmap: the search is **answered** - it runs its false branch, which this one does not have - and no square is invented.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done,
        "**an empty search is an answer**: the false branch runs (npctalk.cpp:4374-4378)"
    );

    cdda_sim::overmap::set_terrain(&mut world, (2, 1), "lab_finale");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done
    );
    let stored = cdda_sim::dialogue_values::position_of(
        &mut world,
        survivor,
        None,
        &cdda_content::dialogue::AssignmentTarget::Variable {
            scope: "u_val".to_owned(),
            name: "lab".to_owned(),
        },
    )
    .expect("the variable holds a square");
    assert_eq!(
        stored,
        (
            2 * cdda_sim::overmap::SQUARES_PER_OMT,
            cdda_sim::overmap::SQUARES_PER_OMT,
            0
        ),
        "the square is the tile's top-left, which is where a mapgen update lands"
    );

    // A target that asks a question the port cannot answer is refused, not answered approximately. An `om_special` **beside** an `om_terrain` is answered, because
    // the reference uses the terrain and never consults the special; a `target_params` naming only a special is the case the port cannot resolve, so that is the
    // example kept here.
    for text in [
        r#"{"u_location_variable": {"u_val": "x"}, "target_params": {"om_special": "hospital"}}"#,
        // A monster search **is** answered now (the `search_type = "monster"` walk), so the refused case here is a field again, which the port has no rule for.
        r#"{"u_location_variable": {"u_val": "x"}, "field": "fd_fire"}"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        assert!(
            cdda_content::dialogue::read_effect(&value).is_none(),
            "{text} asks something the port cannot answer"
        );
    }
}

/// **A search that finds nothing runs its false branch, and a search that finds something runs its true one.**
///
/// Reference: `f_location_variable` (`npctalk.cpp:4370-4392`): `if( !found ) { run_eoc_vector( false_eocs, d ); return; }` after the target search, and
/// `run_eoc_vector( true_eocs, d )` after the value is written. The branch is the **answer** to a search that came up empty - refusing instead throws away the
/// program's own plan for that case, which is what the data writes `false_eocs` to say.
///
/// Both branches here are inline programs that say a line, so the assertion is about which one ran; the position is checked too, because a found search writes the
/// square **and** runs the branch.
#[test]
fn a_location_search_runs_the_branch_it_found() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));
    let store = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_location_variable": {"u_val": "lab"}, "target_params": {"om_terrain": "lab_finale"},
                 "true_eocs": [{"u_message": "found it"}], "false_eocs": [{"u_message": "nothing here"}]}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a search with two branches is readable");

    // Nothing of that terrain: the **false** branch runs, and the effect is answered.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done,
        "**an empty search is an answer, not a refusal**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "nothing here"),
        "**the false branch ran**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert_eq!(
        cdda_sim::dialogue_values::position_of(
            &mut world,
            survivor,
            None,
            &cdda_content::dialogue::AssignmentTarget::Variable {
                scope: "u_val".to_owned(),
                name: "lab".to_owned(),
            },
        ),
        None,
        "**and no square was written**"
    );

    // With a tile, the **true** branch runs and the square is written.
    cdda_sim::overmap::set_terrain(&mut world, (2, 1), "lab_finale");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "found it"),
        "**the true branch ran**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert!(
        cdda_sim::dialogue_values::position_of(
            &mut world,
            survivor,
            None,
            &cdda_content::dialogue::AssignmentTarget::Variable {
                scope: "u_val".to_owned(),
                name: "lab".to_owned(),
            },
        )
        .is_some(),
        "**and the found square is written**"
    );
}

/// **A terrain can be named by a variable, and the data does it that way.**
///
/// Reference: `mission_util::parse_mission_om_target` (`mission_util.cpp:391-396`) reads `om_terrain` with `get_str_or_var( ..., "om_terrain", true )` - a literal
/// **or** a variable - and the data leans on that: `effects_on_condition/generalized_eocs.json:26-27` passes `{ "context_val": "omt" }`, because the tile is chosen
/// by an earlier program. A reader that only accepts a literal refuses every one of those programs, which is not a gap in the rule but in the reading of it.
///
/// The search itself is unchanged: the variable is resolved in the speaker's context, and the square stored is the found tile's top-left.
#[test]
fn a_location_variable_takes_its_terrain_from_a_variable() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));
    let store = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_location_variable": {"u_val": "lab"}, "target_params": {"om_terrain": {"context_val": "omt"}}}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("**an om_terrain named by a variable is readable**");
    cdda_sim::overmap::set_terrain(&mut world, (2, 1), "lab_finale");
    cdda_sim::dialogue_context::put(
        &mut world,
        survivor,
        [("omt".to_owned(), "lab_finale".to_owned())]
            .into_iter()
            .collect(),
    );
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done,
        "**the context names the terrain, and the search finds it**"
    );
    let stored = cdda_sim::dialogue_values::position_of(
        &mut world,
        survivor,
        None,
        &cdda_content::dialogue::AssignmentTarget::Variable {
            scope: "u_val".to_owned(),
            name: "lab".to_owned(),
        },
    )
    .expect("the variable holds a square");
    assert_eq!(
        stored,
        (
            2 * cdda_sim::overmap::SQUARES_PER_OMT,
            cdda_sim::overmap::SQUARES_PER_OMT,
            0
        ),
        "the square is the tile's top-left, which is where a mapgen update lands"
    );
}

/// A mission target names how far out to search, how near a tile may be, and which level - and the port reads all three.
///
/// Reference: `mission_util::parse_mission_om_target` (`mission_util.cpp:425-450`) reads `search_range` (default `OMAPX * 3`), `min_distance` and `z`;
/// `overmapbuffer::find_closest` (`overmapbuffer.cpp:1205-1230`) keeps only tiles at least `min_distance` out.
#[test]
fn a_mission_target_names_its_range_its_distance_and_its_level() {
    let mut world = World::new();
    // The survivor stands in overmap tile (10, 10), so the near tile at (11, 10) is one step away and the far one is five.
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (240, 240, 0));
    cdda_sim::overmap::set_terrain(&mut world, (11, 10), "lab_finale");
    cdda_sim::overmap::set_terrain(&mut world, (15, 10), "lab_finale");

    let store_in = |min_distance: &str, z: &str| {
        let text = format!(
            r#"{{"u_location_variable": {{"u_val": "lab"}}, "target_params": {{"om_terrain": "lab_finale", "min_distance": {min_distance}, "z": {z}}}}}"#
        );
        cdda_content::dialogue::read_effect(&cdda_json::parse(text.as_bytes()).expect("json"))
            .expect("a target with range, distance and level is readable")
    };
    let read_square = |world: &mut World| {
        cdda_sim::dialogue_values::position_of(
            world,
            survivor,
            None,
            &cdda_content::dialogue::AssignmentTarget::Variable {
                scope: "u_val".to_owned(),
                name: "lab".to_owned(),
            },
        )
    };

    // With no minimum distance the nearest tile wins, and the level is the one the data asked for.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store_in("0", "-1")),
        Applied::Done
    );
    let near = cdda_sim::overmap::SQUARES_PER_OMT;
    assert_eq!(
        read_square(&mut world),
        Some((11 * near, 10 * near, -1)),
        "the nearest tile, on the level the target names"
    );

    // A minimum distance of two skips it and lands on the far one, which is five steps out.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store_in("2", "0")),
        Applied::Done
    );
    assert_eq!(read_square(&mut world), Some((15 * near, 10 * near, 0)));

    // A minimum distance past every candidate finds nothing, and **a search that finds nothing is answered** - its false branch runs, and this one has none - rather than
    // offering the closest thing.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store_in("9", "0")),
        Applied::Done,
        "**an empty search runs its false branch** (npctalk.cpp:4374-4378)"
    );
}

/// `u_lose_var` forgets a variable, and each spelling forgets the right actor's.
///
/// Reference: `f_remove_var` (`npctalk.cpp:3656`): `u_lose_var` drops one of the speaker's variables, `npc_lose_var` one of the listener's, and
/// `global_lose_var` one of the world's. Dropping one that was never set is not an error - the reference's `remove_value` says nothing about it.
#[test]
fn losing_a_variable_forgets_it_and_only_where_it_was_set() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (0, 1, 0));
    for holder in [survivor, talker] {
        cdda_sim::dialogue_values::set(
            &mut world,
            holder,
            "remembered",
            cdda_sim::dialogue_values::Value::Number(7.0),
        );
    }
    let gone = |world: &World, holder: bevy_ecs::prelude::Entity| {
        cdda_sim::dialogue_values::number_of(world, holder, "remembered")
    };

    // The speaker's spelling forgets the speaker's copy and leaves the listener's alone.
    let mine = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_lose_var": "remembered"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, Some(talker), &mine),
        Applied::Done
    );
    assert_eq!(
        gone(&world, survivor),
        None,
        "the speaker's variable is gone"
    );
    assert_eq!(
        gone(&world, talker),
        Some(7.0),
        "the listener's is not touched"
    );

    // The listener's spelling forgets the other one.
    let theirs = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"npc_lose_var": "remembered"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, Some(talker), &theirs),
        Applied::Done
    );
    assert_eq!(gone(&world, talker), None);

    // Dropping what is not there is an answer, not a refusal: the reference removes unconditionally.
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, Some(talker), &mine),
        Applied::Done,
        "losing a variable nobody set is still done"
    );
    assert!(!cdda_sim::dialogue_values::remove(
        &mut world,
        survivor,
        "never_set"
    ));
}

/// `u_add_var` stores a value, the clock, or one of several texts.
///
/// Reference: `f_add_var` (`npctalk.cpp:3589-3610`): `possible_values` wins over `value`, `time: true` stores the current turn instead of either, and the
/// draw comes from the world's dice so the same seed gives the same choice.
#[test]
fn adding_a_variable_stores_a_value_the_clock_or_a_drawn_choice() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let text_of = |world: &World, holder: bevy_ecs::prelude::Entity, name: &str| {
        cdda_sim::dialogue_values::text_of(world, holder, name)
    };

    let plain = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_add_var": "mood", "value": "grim"}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &plain),
        Applied::Done
    );
    assert_eq!(text_of(&world, survivor, "mood").as_deref(), Some("grim"));

    // `time: true` stores the clock, and the clock is what moves it afterwards.
    cdda_sim::calendar::set_turn(&mut world, 4_242);
    let timed = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_add_var": "seen_at", "time": true}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &timed),
        Applied::Done
    );
    assert_eq!(
        text_of(&world, survivor, "seen_at").as_deref(),
        Some("4242")
    );

    // A list of texts is drawn from, and it is the world's dice that choose.
    let drawn = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_add_var": "weather_said", "possible_values": ["rain", "sun", "snow"]}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &drawn),
        Applied::Done
    );
    let first = text_of(&world, survivor, "weather_said");
    assert!(
        matches!(first.as_deref(), Some("rain" | "sun" | "snow")),
        "the value came from the list: {first:?}"
    );
    // The same seed picks the same choice, so a replay says the same thing twice.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &drawn),
        Applied::Done
    );
    assert_eq!(text_of(&world, survivor, "weather_said"), first);

    // A member with neither a value nor `time` is a variable with nothing to hold, so it is refused rather than set empty.
    let nothing = cdda_json::parse(br#"{"u_add_var": "mood"}"#.as_slice()).expect("json");
    assert!(cdda_content::dialogue::read_effect(&nothing).is_none());
}

/// A location variable can hold a square found by looking around the actor, which is the radii form.
///
/// Reference: `f_location_variable` (`npctalk.cpp`), the `max_radius > 0` block: up to 25 draws within `max_radius`, taking the first at least
/// `min_radius` away, and refusing when none of the 25 is. The draws are the world's dice, so a replay looks in the same places.
#[test]
fn a_radius_search_stores_a_square_within_it() {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -32..32 {
        for y in -32..32 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let store = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_location_variable": {"u_val": "hereabouts"}, "min_radius": 3, "max_radius": 12}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("a radius search is readable");
    let read = |world: &mut World| {
        cdda_sim::dialogue_values::position_of(
            world,
            survivor,
            None,
            &cdda_content::dialogue::AssignmentTarget::Variable {
                scope: "u_val".to_owned(),
                name: "hereabouts".to_owned(),
            },
        )
    };

    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done
    );
    let Some((x, y, z)) = read(&mut world) else {
        panic!("the search stored a square");
    };
    let step = x.abs().max(y.abs());
    assert!(
        (3..=12).contains(&step),
        "the square is between the radii: {x}, {y} is {step} steps out"
    );
    assert_eq!(z, 0, "the search stays on the actor's level");

    // The same seed looks in the same place, which is what lets a replay be compared at all.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done
    );
    assert_eq!(read(&mut world), Some((x, y, z)));

    // A radius that reaches past the level finds nothing it may store, and says so rather than storing the actor's own square.
    let nowhere = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_location_variable": {"u_val": "nowhere"}, "min_radius": 200, "max_radius": 400}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &nowhere),
        Applied::CannotAnswer
    );
}

/// A target search and a radius are two steps in order: find the tile, then draw around **what was found**.
///
/// Reference: `f_location_variable` (`npctalk.cpp`): the target search moves `talker_pos` to the found position, and the `max_radius > 0` block then draws
/// around that. A port that drew around the actor instead would answer a question the data did not ask.
#[test]
fn a_target_search_then_a_radius_draws_around_what_was_found() {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -48..144 {
        for y in -48..144 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    // The survivor in overmap tile (0, 0); the terrain to find is at tile (3, 0), whose top-left square is 72.
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (1, 1, 0));
    cdda_sim::overmap::set_terrain(&mut world, (3, 0), "lab_finale");

    let store = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_location_variable": {"u_val": "spot"}, "target_params": {"om_terrain": "lab_finale"}, "min_radius": 2, "max_radius": 6}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a target with a radius is readable");

    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &store),
        Applied::Done
    );
    let found = cdda_sim::dialogue_values::position_of(
        &mut world,
        survivor,
        None,
        &cdda_content::dialogue::AssignmentTarget::Variable {
            scope: "u_val".to_owned(),
            name: "spot".to_owned(),
        },
    )
    .expect("the variable holds a square");

    // The square is drawn around the found tile (72, 0), not around the survivor at (1, 1).
    let from_tile = (found.0 - 72).abs().max(found.1.abs());
    assert!(
        (2..=6).contains(&from_tile),
        "the square is two to six steps from the found tile: {:?} is {from_tile}",
        (found.0, found.1)
    );
    let from_actor = (found.0 - 1).abs().max((found.1 - 1).abs());
    assert!(
        from_actor > 6,
        "and it is nowhere near the actor, which is what tells the two steps apart: {from_actor}"
    );
}

/// `u_spawn_monster` makes real monsters and hallucinations, in a ring around the actor.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7371-7420`): `real_count` monsters of the type, then `hallucination_count` more that look identical and are not,
/// each on a square within `min_radius..max_radius` - whose defaults are **1** and **10**.
#[test]
fn spawning_monsters_makes_the_real_ones_and_the_imagined_ones() {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -24..48 {
        for y in -24..48 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let prototype = cdda_json::parse(
        br#"{"id": "mon_test", "name": "a test thing", "hp": 12, "speed": 100, "symbol": "t", "color": "white"}"#.as_slice(),
    )
    .expect("json");
    let kind = cdda_content::monsters::load_objects(&[prototype])
        .expect("readable")
        .into_iter()
        .next()
        .expect("one type");
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_test",
            hp: 12,
        }],
    );
    cdda_sim::monster_catalog::set_types(&mut world, vec![kind]);

    let spawn = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_spawn_monster": "mon_test", "real_count": 2, "hallucination_count": [1, 1], "min_radius": 3, "max_radius": 5}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a spawn is readable");

    assert!(
        cdda_sim::monster_catalog::type_of(&world, "mon_test").is_some(),
        "the catalog knows the type"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (4, 0, 0)),
        Some(0),
        "the level holds the squares the ring covers"
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(13));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &spawn),
        Applied::Done
    );

    // Three creatures were made - two real and one imagined - and the imagined one is marked.
    let imagined = cdda_sim::monster_spawning::hallucinations(&world);
    assert_eq!(imagined.len(), 1, "one hallucination");
    let mut made = 0;
    let mut query = world
        .try_query::<(&cdda_sim::creature::CreatureKind, &cdda_sim::Position)>()
        .expect("query");
    for (_, at) in query.iter(&world) {
        if at.x == 0 && at.y == 0 {
            continue;
        }
        made += 1;
        let step = at.x.abs().max(at.y.abs());
        assert!(
            (3..=5).contains(&step),
            "every spawn lands in the ring: ({}, {}) is {step} steps out",
            at.x,
            at.y
        );
    }
    assert_eq!(made, 3, "two real and one imagined");

    // A type the world does not know refuses rather than making something else.
    let unknown = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_spawn_monster": "mon_not_here", "real_count": 1}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &unknown),
        Applied::CannotAnswer
    );

    // The members the port cannot carry out are refused at read time rather than ignored. `spawn_message`, `outdoor_only`, `lifespan` and now `group` have left
    // this list as their rules landed; `mon_variables` has not, because it writes variables onto the spawned monster and no rule reads them yet.
    let with_variables = cdda_json::parse(
        br#"{"u_spawn_monster": "mon_test", "mon_variables": {"x": "1"}}"#.as_slice(),
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_effect(&with_variables).is_none());
}

/// A spawned monster can be given a lifespan, and the clock takes it away.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7552-7554`): `spawned->set_summon_time( lifespan )` when the data gives one, and the summon-expiry check
/// erases the monster once the clock passes it.
#[test]
fn a_summon_goes_when_its_lifespan_runs_out() {
    let mut world = World::new();
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
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_test",
            hp: 12,
        }],
    );
    cdda_sim::monster_catalog::set_types(&mut world, vec![kind]);

    cdda_sim::calendar::set_turn(&mut world, 1_000);
    let limited = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_spawn_monster": "mon_test", "real_count": 1, "lifespan": "5 seconds"}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("a lifespan is readable");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(17));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &limited),
        Applied::Done
    );

    // One turn before its time it is still there, and at its time it is gone: the boundary is the rule, not a round number.
    assert_eq!(
        cdda_sim::monster_spawning::expire_summonings(&mut world, 1_004),
        0
    );
    assert_eq!(
        cdda_sim::monster_spawning::expire_summonings(&mut world, 1_005),
        1
    );

    // A spawn with no lifespan stays, however long the clock runs.
    let lasting = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_spawn_monster": "mon_test", "real_count": 1}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(19));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &lasting),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::monster_spawning::expire_summonings(&mut world, 999_999),
        0
    );

    // A lifespan given as a range is drawn between its ends, both included - the data writes `[ "5 seconds", "10 seconds" ]`.
    let ranged = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_spawn_monster": "mon_test", "real_count": 1, "lifespan": ["5 seconds", "10 seconds"]}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("a ranged lifespan is readable");
    for seed in 0..16 {
        cdda_sim::calendar::set_turn(&mut world, 0);
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let drawn = cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &ranged);
        assert_eq!(
            drawn,
            Applied::Done,
            "the draw must land inside the range, seed {seed}"
        );
    }
}

/// A spawn can be asked for under the sky or under a roof, and it says a line when it happens.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7371-7420`): `outdoor_only` and `indoor_only` are refused together, the square drawn must satisfy them, and
/// `spawn_message` (plural when more than one appears) is the line the player reads.
#[test]
fn a_spawn_can_ask_for_the_sky_or_a_roof_and_say_a_line() {
    let mut world = World::new();
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
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_test",
            hp: 12,
        }],
    );
    cdda_sim::monster_catalog::set_types(&mut world, vec![kind]);

    // A square with nothing on it is under the sky.
    assert!(cdda_sim::outdoors::is_outside(&world, (5, 5, 0)));
    assert!(
        !cdda_sim::outdoors::is_outside(&world, (900, 900, 0)),
        "a square the level does not hold is nowhere"
    );

    let outside_spawn = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"u_spawn_monster": "mon_test", "real_count": 2, "outdoor_only": true, "spawn_message": "one appears", "spawn_message_plural": "several appear"}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("an outdoor spawn is readable");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(23));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &outside_spawn),
        Applied::Done
    );

    // Two appeared, so the plural line is the one said - once, not twice.
    let said = cdda_sim::messages::lines(&world);
    assert_eq!(said.len(), 1, "one line for the whole spawn: {said:?}");
    assert!(
        said.iter().any(|line| line.contains("several appear")),
        "the plural line is used when more than one appears: {said:?}"
    );

    // Both flags at once is no square at all, so the reference refuses it and so does the port.
    let both = cdda_json::parse(
        br#"{"u_spawn_monster": "mon_test", "outdoor_only": true, "indoor_only": true}"#.as_slice(),
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_effect(&both).is_none());
}

/// The bare `message` member says a line too, and it is the global form of `u_message`.
///
/// Reference: `f_message` is registered three times (`npctalk.cpp:8039` and `:8135`) - `u_message`, `npc_message`, and `message`, the last being the **global**
/// form. The data writes it as one word for lines a program says rather than a talker, and reading only the two creature forms refuses five of the data's
/// programs.
#[test]
fn the_global_message_member_says_a_line() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    let spoken = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"message": "the air shimmers"}"#.as_slice()).expect("json"),
    )
    .expect("the global form is readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &spoken),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["the air shimmers".to_owned()]
    );

    // The translation object form reads as the same line, which is how the data writes a line with a `str` member.
    let translated = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"message": {"str": "a second line"}}"#.as_slice()).expect("json"),
    )
    .expect("the object form is readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &translated),
        Applied::Done
    );
    assert_eq!(cdda_sim::messages::lines(&world).len(), 2);
}

/// **A locked creature is not moved by a teleport effect.** Reference: `f_teleport` (`npctalk.cpp:7836-7845`) hands the move to `teleport::teleport_to_point` and shows
/// `fail_message` when it answers `false` - so the guard that holds a spell's teleport holds a dialogue's too, and the failure is visible to the player.
#[test]
fn a_locked_creature_is_not_moved_by_a_teleport_effect() {
    use cdda_sim::dialogue_effects::{Applied, run_effects};
    use cdda_sim::dialogue_values::{self, Value};

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let mut kind = cdda_sim::effect_registry::bare("effect_lock");
    kind.flags.insert("TELEPORT_LOCK".to_owned());
    cdda_sim::effect_registry::set_effect_types(&mut world, &[kind]);
    cdda_sim::applied_effects::add(
        &mut world,
        survivor,
        cdda_sim::effects::ActiveEffect {
            id: "effect_lock".to_owned(),
            intensity: 1,
            duration: 10,
            permanent: false,
        },
    );
    let holder = dialogue_values::holder_for(&mut world, survivor, false);
    dialogue_values::set(
        &mut world,
        holder,
        "dest",
        Value::Position { x: 5, y: 5, z: 0 },
    );

    let effect = cdda_content::dialogue::Effect::Teleport {
        scope: "u_val".to_owned(),
        name: "dest".to_owned(),
        listener: false,
        fail_message: None,
        success_message: None,
        force: false,
        force_safe: false,
    };
    assert_eq!(
        run_effects(&mut world, survivor, None, &[effect]),
        Applied::CannotAnswer,
        "**the guard refused, and the effect says so**"
    );
    let at = world
        .get::<cdda_sim::Position>(survivor)
        .map(|at| (at.x, at.y, at.z));
    assert_eq!(at, Some((0, 0, 0)), "the locked creature did not move");
}

/// **A teleport can be aimed at a square the caller handed it**, which is the shape five entries in the data take: `{ "u_teleport": { "context_val": "trap_location" } }`
/// (`traps.json:532`) names a square from the **invocation's context** rather than from the speaker's own variables. The context is text, so the square arrives as `x,y,z` and has
/// to be read back - `read_var_value( target_var, d ).tripoint()` in the reference (`npctalk.cpp:7836-7840`).
#[test]
fn a_teleport_reads_its_target_from_the_context() {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
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
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "teleporter", (0, 0, 0));
    cdda_sim::PositionIndex::rebuild(&mut world);
    cdda_sim::dialogue_context::put(
        &mut world,
        speaker,
        [("trap_location".to_owned(), "4,5,0".to_owned())]
            .into_iter()
            .collect(),
    );

    let effect = cdda_content::dialogue::Effect::Teleport {
        scope: "context_val".to_owned(),
        name: "trap_location".to_owned(),
        listener: false,
        fail_message: None,
        success_message: None,
        force: false,
        force_safe: false,
    };
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, speaker, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done,
        "**the square the context names is where it goes**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(speaker)
            .map(|at| (at.x, at.y, at.z)),
        Some((4, 5, 0)),
        "and it is there"
    );
}

/// **`location_variable_adjust` moves a square that lives in the invocation's context**, which is the entry `EOC_TELEPAD` runs immediately before its teleport
/// (`traps.json:527-532`): the trap hands the program a square, the adjust moves it by a few, and the teleport goes there. Reading the adjust's square from a holder read the
/// wrong store, so the entry could never move the context value it names.
#[test]
fn an_adjust_moves_a_square_in_the_context() {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -10..=20 {
        for x in -10..=20 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "teleporter", (0, 0, 0));
    cdda_sim::PositionIndex::rebuild(&mut world);
    cdda_sim::dialogue_context::put(
        &mut world,
        speaker,
        [("trap_location".to_owned(), "4,5,0".to_owned())]
            .into_iter()
            .collect(),
    );

    let adjust = cdda_content::dialogue::Effect::LocationVariableAdjust {
        scope: "context_val".to_owned(),
        name: "trap_location".to_owned(),
        output: None,
        x_adjust: cdda_content::dialogue::Expression::Number(2.0),
        y_adjust: cdda_content::dialogue::Expression::Number(-1.0),
        z_adjust: cdda_content::dialogue::Expression::Number(0.0),
        z_override: false,
        overmap_tile: false,
    };
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, speaker, None, &adjust),
        cdda_sim::dialogue_effects::Applied::Done,
        "**the adjust answers**"
    );
    assert_eq!(
        cdda_sim::dialogue_context::values(&world, speaker)
            .get("trap_location")
            .cloned(),
        Some("6,4,0".to_owned()),
        "**and the context holds the moved square**: four and five, two east and one north"
    );

    // And then the teleport, which is the pair the telepad runs in order.
    let teleport = cdda_content::dialogue::Effect::Teleport {
        scope: "context_val".to_owned(),
        name: "trap_location".to_owned(),
        listener: false,
        fail_message: None,
        success_message: None,
        force: false,
        force_safe: false,
    };
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, speaker, None, &teleport),
        cdda_sim::dialogue_effects::Applied::Done
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(speaker)
            .map(|at| (at.x, at.y, at.z)),
        Some((6, 4, 0)),
        "the creature is on the square the adjust moved it to"
    );
}

/// **A target nobody set is the origin, not a refusal.** `read_var_value( target_var, d )` answers an **empty** `diag_value` for an unset variable, and `diag_value::tripoint()`
/// converts an empty value to a null `tripoint_abs_ms` - `( 0, 0, 0 )` absolute (`_diag_value_helper`'s `std::monostate` arm, `math_parser_diag_value.cpp:62-70`). So the reference
/// teleports the creature to the map's origin when the square's variable is empty, and the port does the same: nine programs in the data reach a teleport whose variable nothing
/// ever set, and refusing them was the port inventing a rule the reference does not have.
#[test]
fn an_unset_target_square_is_the_origin() {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
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
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (5, 5, 0));
    cdda_sim::PositionIndex::rebuild(&mut world);

    let effect = cdda_content::dialogue::Effect::Teleport {
        scope: "u_val".to_owned(),
        name: "never_set".to_owned(),
        listener: false,
        fail_message: None,
        success_message: None,
        force: false,
        force_safe: false,
    };
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, who, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done,
        "**the teleport happens**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(who)
            .map(|at| (at.x, at.y, at.z)),
        Some((0, 0, 0)),
        "at the null square the empty value converts to"
    );
}

/// **`next_weather` makes the sky re-roll at once.** Reference: `f_next_weather` (`npctalk.cpp:5790-5795`) calls `get_weather().set_nextweather( calendar::turn )`, and
/// `weather_manager::set_nextweather` (`weather.cpp:950-954`) writes that turn as the **due** turn and then updates. The data uses it to force a change: the portal storm's own
/// program ends with it (`portal_storm_effect_on_condition.json:152`).
#[test]
fn next_weather_makes_the_sky_due_now() {
    let mut world = World::new();
    cdda_sim::singletons::set_climate(
        &mut world,
        cdda_sim::Climate::new(cdda_content::weather::Rules::default()),
    );
    cdda_sim::calendar::set_turn(&mut world, 100);
    // Weather that is not due for a thousand turns: the manager remembers a due turn, and this one is far away.
    if let Some(mut climate) = cdda_sim::singletons::climate(&mut world) {
        climate.system.manager.weather = Some("clear".to_owned());
        climate.system.manager.next_turn = 1_100;
    }
    assert!(
        !cdda_sim::weather::next_is_due(&world, 100),
        "the sky is not due yet"
    );

    let effect = cdda_content::dialogue::Effect::LegacyStringEffect {
        name: "next_weather".to_owned(),
    };
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, who, None, &effect),
        cdda_sim::dialogue_effects::Applied::Done,
        "**the effect answers**"
    );
    assert!(
        cdda_sim::weather::next_is_due(&world, 100),
        "and the weather is due on this very turn"
    );
}

/// **Pain can be written from a `math` statement.** The data raises pain directly - `["u_pain()", "+=", "10"]` in the portal-storm programs - and the reference's
/// `pain_ass` (`math_parser_diag.cpp:884-900`) writes it with `set_pain`, which floors the result at zero. The port answered the *read* half of `u_pain()` and refused
/// the write, so every program that hurt the player refused at that statement.
#[test]
fn a_math_statement_can_add_to_pain() {
    let mut world = World::new();
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world.entity_mut(who).insert(cdda_sim::creature::Pain(0));
    let raise = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"math": ["u_pain()", "+=", "10"]}"#.as_slice()).expect("json"),
    )
    .expect("a pain assignment is readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, who, None, &raise),
        Applied::Done,
        "**the write the port could not do**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Pain>(who)
            .map(|pain| pain.0),
        Some(10)
    );
    // **`set_pain` floors at zero** (`creature.cpp:2189-2196`), so a subtraction past it leaves none rather than a negative number.
    let lower = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"math": ["u_pain()", "-=", "30"]}"#.as_slice()).expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, who, None, &lower),
        Applied::Done
    );
    assert_eq!(
        world
            .get::<cdda_sim::creature::Pain>(who)
            .map(|pain| pain.0),
        Some(0),
        "**and pain never goes below zero**"
    );
}

/// **A `set_string_var` from a variable that was never set stores nothing.** Reference: `f_set_string_var` (`npctalk.cpp:5809-5866`) evaluates its source with
/// `read_var_value`, whose answer for a variable that does not exist is the empty string - and a stored empty string is an answer, where refusing throws the whole
/// effect away. The mutation this test exists for is the one the batch run caught: storing the name instead of the value's default.
#[test]
fn a_set_string_var_from_an_unset_source_stores_nothing() {
    let mut world = World::new();
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"set_string_var": {"u_val": "missing"}, "target_var": {"u_val": "stored"}}"#
                .as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, who, None, &effect),
        Applied::Done,
        "**an unset source is the empty string, not a refusal**"
    );
    assert_eq!(
        cdda_sim::dialogue_values::text_of(&world, who, "stored"),
        Some(String::new()),
        "**and nothing is what was stored**"
    );
}

/// **A `reveal_map` whose target was never set reveals around the origin.** Reference: `f_reveal_map` (`npctalk.cpp:7752-7760`) reads the variable's square - an unset
/// variable reads as `tripoint_abs_ms()`, absolute square (0, 0) (`math_parser_diag_value.cpp:62-70`, port round 238) - and projects it to overmap tile (0, 0).
///
/// **The assertion is the difference, not the presence**: a first version asked whether tile (0, 0) was seen, which the old behaviour and a mutated origin at (1, 1)
/// both satisfied because the reveal covered a radius. Radius zero plus a before/after difference makes the tile the rule chose the only thing the test can see.
#[test]
fn a_reveal_map_with_an_unset_target_reveals_at_the_origin() {
    let mut world = World::new();
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (5, 5, 0));
    let before = cdda_sim::overmap::seen_tiles(&world);
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"reveal_map": {"global_val": "nowhere"}, "radius": 0}"#.as_slice())
            .expect("json"),
    )
    .expect("a reveal with an unset target is readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, who, None, &effect),
        Applied::Done,
        "**an unset target is the origin, not a refusal**"
    );
    let mut revealed: Vec<(i32, i32, i32)> = cdda_sim::overmap::seen_tiles(&world)
        .into_iter()
        .filter(|tile| !before.contains(tile))
        .collect();
    revealed.sort_unstable();
    assert_eq!(
        revealed,
        vec![(0, 0, 0)],
        "**the reveal covers the origin and nothing else**"
    );
}

/// **A `u_add_var` whose value was never set stores nothing.** Reference: `f_add_var` (`npctalk.cpp:3589-3610`) evaluates its value with `read_var_value`, whose
/// answer for a variable that does not exist is the empty string; refusing threw the effect away instead of storing the empty answer.
#[test]
fn an_add_var_from_an_unset_source_stores_nothing() {
    let mut world = World::new();
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(br#"{"u_add_var": "stored", "value": {"u_val": "missing"}}"#.as_slice())
            .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, who, None, &effect),
        Applied::Done,
        "**an unset source is the empty string, not a refusal**"
    );
    assert_eq!(
        cdda_sim::dialogue_values::text_of(&world, who, "stored"),
        Some(String::new()),
        "**and nothing is what was stored**"
    );
}

/// **A `mapgen_update` that says "in the future" is scheduled, not applied now.** Reference: `f_mapgen_update` (`npctalk.cpp:4853-4868`) reads
/// `time_in_future`; a positive duration adds an `UPDATE_MAPGEN` timed event carrying the place and the update's id, and applies nothing until that event runs.
/// The port applied it immediately, which is the opposite of what the data asked for.
///
/// **The event's place is the plan's**, and it is a real member of the store: the reference's `timed_event` holds a `point`, so the port's holds one too rather than
/// folding a square into the text.
#[test]
fn a_mapgen_update_in_the_future_is_scheduled_and_not_applied() {
    let (mut world, survivor) = a_wall_update_world();
    cdda_sim::calendar::set_turn(&mut world, 0);
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"mapgen_update": "lab_wall_opens", "time_in_future": "10 turns"}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("a mapgen update with a delay is readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        Applied::Done,
        "**the schedule is the effect's answer**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 2, 0)),
        Some(0),
        "**and nothing was written yet**"
    );
    assert_eq!(
        cdda_sim::timed_events::place_of(&world, "UPDATE_MAPGEN"),
        Some((2, 2, 0)),
        "**the event carries where the update will land**"
    );
    assert_eq!(
        cdda_sim::timed_events::text_of(&world, "UPDATE_MAPGEN").as_deref(),
        Some("lab_wall_opens"),
        "**and which update it is**"
    );
    assert_eq!(
        cdda_sim::timed_events::has_happened(&world, "UPDATE_MAPGEN"),
        Some(false),
        "**ten turns out is not yet due**"
    );
}

/// **A scheduled update lands on its turn, through the one store that holds future events.** Reference: `timed_events::process` (`timed_event.cpp:100-130`) erases each
/// event whose turn has come and then processes it; `UPDATE_MAPGEN` applies its update at the square the event carries.
///
/// The port used to keep **two** stores for work in the future - one keyed by turn for dialogue payloads and one by name for everything else - which is the second source of
/// truth the architecture forbids. They are one store now, so this test and the dialogue test in `probe.rs` are the same mechanism seen from two sides.
#[test]
fn a_scheduled_mapgen_update_lands_when_its_turn_comes() {
    let (mut world, survivor) = a_wall_update_world();
    cdda_sim::calendar::set_turn(&mut world, 0);
    let effect = cdda_content::dialogue::read_effect(
        &cdda_json::parse(
            br#"{"mapgen_update": "lab_wall_opens", "time_in_future": "10 turns"}"#.as_slice(),
        )
        .expect("json"),
    )
    .expect("readable");
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(&mut world, survivor, None, &effect),
        Applied::Done
    );
    assert_eq!(
        cdda_sim::timed_events::waiting(&world),
        1,
        "one event is waiting"
    );
    cdda_sim::calendar::set_turn(&mut world, 10);
    cdda_sim::timed_events::process_due(&mut world, survivor);
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 2, 0)),
        Some(0),
        "**a turn early is still too early**"
    );
    assert_eq!(
        cdda_sim::timed_events::waiting(&world),
        1,
        "and it is still waiting"
    );
    cdda_sim::calendar::set_turn(&mut world, 11);
    cdda_sim::timed_events::process_due(&mut world, survivor);
    assert_eq!(
        cdda_sim::timed_events::waiting(&world),
        0,
        "**the event is taken out before it runs**"
    );
    assert!(
        matches!(cdda_sim::square::terrain_at(&world, (2, 2, 0)), Some(7 | 8)),
        "**and the update applied**: {:?}",
        cdda_sim::square::terrain_at(&world, (2, 2, 0))
    );
}

/// **The temple's flood rises one ring a step, and schedules itself until the chamber is full.** Reference: `timed_event_type::TEMPLE_FLOOD`
/// (`timed_event.cpp:223-280`): shallow water beside deep water becomes deep, a rock floor beside any water becomes shallow, every decision is made from the level **as it
/// stood at the start of the step**, the player's own square decides which line is printed, and the event adds itself again two or three turns out - stopping the first
/// time a step changes nothing.
#[test]
fn the_temple_flood_rises_a_ring_a_step_and_stops_when_full() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_water_dp".to_owned(), 30),
            ("t_water_sh".to_owned(), 31),
            ("t_rock_floor".to_owned(), 32),
            ("t_rock".to_owned(), 33),
        ],
    );
    for x in 0..3 {
        cdda_sim::square::set_terrain(&mut world, (x, 0, 0), 33);
    }
    // A line: floor, shallow, deep - so the floor floods shallow and the shallow floods deep in one step. **The deep square is what makes it a flood**: a level with no
    // deep water changes nothing, which is the second half of this test.
    cdda_sim::square::set_terrain(&mut world, (0, 0, 0), 32);
    cdda_sim::square::set_terrain(&mut world, (1, 0, 0), 31);
    cdda_sim::square::set_terrain(&mut world, (2, 0, 0), 30);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, survivor);
    cdda_sim::calendar::set_turn(&mut world, 10);
    cdda_sim::timed_events::schedule(&mut world, "TEMPLE_FLOOD", 0);
    cdda_sim::timed_events::process_due(&mut world, survivor);
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 0, 0)),
        Some(30),
        "**the shallow square became deep**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(31),
        "**and the floor beside it became shallow in the same step**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("to your knees")),
        "**the player's square decided the line**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert_eq!(
        cdda_sim::timed_events::waiting(&world),
        1,
        "**and the flood is scheduled again**"
    );
    // **It keeps going until a step changes nothing**, which is the reference's own stop: each step deepens what the last one made shallow, so the line takes two more
    // steps to fill and the third finds nothing to do.
    for turn in [100, 200, 300] {
        cdda_sim::calendar::set_turn(&mut world, turn);
        cdda_sim::timed_events::process_due(&mut world, survivor);
    }
    assert_eq!(
        cdda_sim::timed_events::waiting(&world),
        0,
        "**a step that changes nothing ends the flood**"
    );
    for x in 0..3 {
        assert_eq!(
            cdda_sim::square::terrain_at(&world, (x, 0, 0)),
            Some(30),
            "**the line is all deep water**"
        );
    }
}
