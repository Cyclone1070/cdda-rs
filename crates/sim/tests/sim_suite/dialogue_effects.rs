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
                        let cdda_json::Value::Object(fields) = entry else {
                            *refused.entry("<named>".to_owned()).or_insert(0) += 1;
                            continue;
                        };
                        let ran = cdda_content::dialogue::read_effect(entry).is_some();
                        if ran {
                            *runnable += 1;
                        } else {
                            for (key, _) in fields.members() {
                                *refused.entry(key.clone()).or_insert(0) += 1;
                            }
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

    let mut top: Vec<(String, usize)> = refused.into_iter().collect();
    top.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    top.truncate(5);
    println!("eoc effects: {entries} entries, {runnable} runnable; most common refusals: {top:?}");

    assert_eq!(entries, 1612, "the data's effect-on-conditions hold 1,612 effects");
    // 179 of the 304 `math` entries. Reading both spellings of a destination (`"x = 1"` and `"u_val('x') = 1"`)
    // took it from 174 to 179, and the 125 still refused are of two shapes worth naming: 139 of the statements are pure
    // **expressions** with no assignment at all (`"u_val('stamina')"`, `"ps_str + 50"`), and 76 are bare operators that suggest a
    // statement split across several entries in one list (`["...", "*=", "..."]`). Neither is run: a list whose meaning the port
    // cannot read is refused whole, because "ignore the parts I do not understand" is how a program silently does half its job.
    // 399: 179 `math` statements, 184 messages, and 36 plain calls to another program now that `run_eocs` runs. The other 131
    // `run_eocs` entries are object forms with modifiers (`popup`, `variables`, `true_eocs`/`false_eocs`), which the port refuses
    // rather than reading only the part it recognises.
    assert_eq!(runnable, 399, "of which the port runs 399 entries");
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
