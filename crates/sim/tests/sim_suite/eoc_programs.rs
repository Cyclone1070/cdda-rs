//! Running the data's effect-on-conditions: a program that names another program.
//!
//! Reference: `effect_on_conditions::load` fills the registry (`effect_on_condition.cpp:119-133`) and `dialogue::effect`'s
//! `run_eocs` runs a name from it (`npctalk.cpp`). This is the piece that makes the machine compose: until a program could
//! name another, every effect was a leaf.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::eoc_registry::{self, DEPTH_LIMIT};
use cdda_sim::dialogue_effects::Applied;
use cdda_sim::dialogue_values;
use cdda_sim::messages;

fn program(text: &str) -> cdda_content::dialogue::Program {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    cdda_content::dialogue::read_program(&value).expect("the port reads this program")
}

/// A world holding the two programs, a survivor, and nothing else.
fn world_with(text: &str) -> (World, Entity) {
    let mut world = World::new();
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let cdda_json::Value::Array(definitions) = &value else {
        panic!("a list of definitions");
    };
    let programs = definitions
        .iter()
        .map(|definition| cdda_content::dialogue::read_program(definition).expect("readable"))
        .collect::<Vec<_>>();
    eoc_registry::set_programs(&mut world, &programs);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    (world, survivor)
}

#[test]
fn a_program_runs_the_programs_it_names() {
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_OUTER",
             "effect": [{"u_message": "outer"}, "EOC_INNER"]},
            {"type": "effect_on_condition", "id": "EOC_INNER",
             "effect": [{"u_message": "inner"}, {"math": ["depth = 1"]}]}
        ]"#,
    );
    assert_eq!(eoc_registry::count(&mut world), 2, "both programs are installed");
    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_OUTER", 0),
        Applied::Done
    );
    assert_eq!(
        messages::lines(&world),
        vec!["outer".to_owned(), "inner".to_owned()],
        "and the named program ran where it was named"
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "depth"),
        Some(1.0),
        "the inner program changed the same survivor's values"
    );
}

#[test]
fn a_program_that_is_not_defined_is_refused_rather_than_silently_skipped() {
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_LONELY",
             "effect": [{"u_message": "before"}, "EOC_MISSING", {"u_message": "after"}]}
        ]"#,
    );
    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_LONELY", 0),
        Applied::CannotAnswer
    );
    assert_eq!(
        messages::lines(&world),
        vec!["before".to_owned()],
        "it stopped where the missing program would have run"
    );
}

#[test]
fn a_program_whose_condition_is_false_runs_nothing() {
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_GUARDED",
             "condition": {"math": ["armed", "==", 1]},
             "effect": [{"u_message": "armed"}]}
        ]"#,
    );
    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_GUARDED", 0),
        Applied::Done,
        "a false condition is a finished program, not a failure"
    );
    assert!(messages::lines(&world).is_empty());
}

#[test]
fn a_program_that_names_itself_is_stopped_at_the_depth_limit() {
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_LOOP", "effect": ["EOC_LOOP"]}
        ]"#,
    );
    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_LOOP", 0),
        Applied::CannotAnswer,
        "the port stops a loop instead of hanging a headless run"
    );
    assert_eq!(DEPTH_LIMIT, 16, "and the limit is a stated number");
}

#[test]
fn a_program_using_a_condition_the_port_cannot_read_is_refused_whole() {
    // `u_know_recipe` is not a condition the port answers, so the program is not installed at all - running the effects and
    // skipping the condition would be running the wrong program.
    let value = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_PARTIAL",
            "condition": "u_know_recipe",
            "effect": [{"u_message": "never"}]}"#,
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_program(&value).is_none());
    let _ = program;
}

/// The first member of a guard that the port does not read, for ranking the refusals.
///
/// Deliberately cheap: it names the **top-level** member of the guard, which is enough to choose the next round's work and
/// makes no claim about nested members. Asking one member at a time in isolation was tried and was wrong - a single-member
/// object is not the same question as the object it came from (an `and` whose members are each readable can still be refused),
/// so the port reports what it knows instead of a guess dressed as a diagnosis.
fn blame(condition: Option<&cdda_json::Value>) -> String {
    match condition {
        Some(cdda_json::Value::Object(object)) => {
            let Some((key, value)) = object.members().first() else {
                return "condition".to_owned();
            };
            // A joiner is not the thing that failed - the member inside it is, and reporting `and` for 157 programs would only
            // say "somewhere in a list". The items are already values, so no reconstruction is needed to ask about them.
            match key.as_str() {
                "and" | "or" => match value {
                    cdda_json::Value::Array(items) => items
                        .iter()
                        .find(|item| cdda_content::dialogue::read_condition(item).is_none())
                        .map_or_else(|| key.clone(), |item| blame(Some(item))),
                    _ => key.clone(),
                },
                "not" => blame(Some(value)),
                _ => key.clone(),
            }
        }
        Some(cdda_json::Value::Str(_)) => "<named>".to_owned(),
        _ => "condition".to_owned(),
    }
}

fn collect(
    value: &cdda_json::Value,
    defined: &mut usize,
    whole: &mut usize,
    refused: &mut std::collections::BTreeMap<String, usize>,
    programs: &mut Vec<cdda_content::dialogue::Program>,
) {
    match value {
        cdda_json::Value::Object(object) => {
            if matches!(object.get("type"), Some(cdda_json::Value::Str(kind)) if kind == "effect_on_condition")
                && object.get("id").is_some()
            {
                *defined += 1;
                if let Some(program) = cdda_content::dialogue::read_program(value) {
                    *whole += 1;
                    programs.push(program);
                } else {
                        // Say **which** half refused, so the census chooses the next round's work: a condition nobody can
                        // answer and an effect nobody can run are different problems.
                        let condition_ok = match object.get("condition") {
                            None => true,
                            Some(condition) => {
                                cdda_content::dialogue::read_condition(condition).is_some()
                            }
                        };
                        let blocked = if condition_ok {
                            "effect".to_owned()
                        } else {
                            blame(object.get("condition"))
                        };
                        *refused.entry(blocked).or_insert(0) += 1;
                }
            }
            for (_, member) in object.members() {
                collect(member, defined, whole, refused, programs);
            }
        }
        cdda_json::Value::Array(items) => {
            for item in items {
                collect(item, defined, whole, refused, programs);
            }
        }
        _ => {}
    }
}

/// How many of the data's 712 effect-on-conditions the port can run whole.
///
/// This is the machine's own census, and the one that matters most: a program is installed only when its **condition and every
/// effect** are readable, because running the effects of a program whose guard was dropped is running the wrong program. Before
/// this round no program could be installed at all - there was no registry to install into.
#[test]
fn the_port_installs_the_programs_it_can_run_whole_and_says_how_many() {
    use std::collections::BTreeMap;

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut defined = 0usize;
    let mut whole = 0usize;
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();

    let mut programs = Vec::new();
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
            collect(&value, &mut defined, &mut whole, &mut refused, &mut programs);
        }
    }

    println!("eoc programs: {defined} defined, {whole} runnable whole; refusals: {refused:?}");

    // The ranking is the round's deliverable: it says what to port next instead of what is most used. `math` guards are the
    // biggest single refusal (42 programs), then traits (36), then location (28) - none of which the earlier census could see,
    // because reporting the joiner (`and`, 157 programs) only said "somewhere in a list".
    //
    // Porting the trait guard moved three numbers at once, which is the census's own lesson: without it, 36 programs were blamed
    // on `u_has_trait`; with it, those programs are blamed on what refused them **next** - mostly their effects (374 -> 397) and
    // `math` guards (42 -> 45). A refusal that moves house is not the same as a program that starts running, and only one of the
    // thirty-six turned out to be otherwise complete.
    assert_eq!(
        whole, 170,
        "a hundred and seventy run whole: inline `run_eocs` added fifty-nine, `u_has_item` nine, the list guards two"
    );
    assert_eq!(refused.get("effect"), Some(&338), "and the effect side is still the bigger refusal");
    assert_eq!(refused.get("u_has_trait"), None, "traits are no longer blamed for anything");
    assert_eq!(refused.get("math"), Some(&46), "`math` guards are the biggest single one");
    assert_eq!(refused.get("u_has_item"), None, "carrying an item is no longer blamed for anything");
    // The chance guards now roll, so what is left of them is one program apiece - and the eight programs they freed are blamed on
    // what refused them next, which is the "moves house" pattern for the fifth time.
    assert_eq!(refused.get("one_in_chance"), Some(&1), "the dice no longer block the chance guards");
    assert_eq!(refused.get("x_in_y_chance"), Some(&1), "both of them, down to one each");
    assert_eq!(refused.get("u_at_om_location"), Some(&28), "with overmap location next");
    // 692, not the 712 effect-on-condition objects in the data: twenty carry no `id`, so nothing can name them and the port does
    // not install them. The count of definitions was itself worth learning - the 712 in the earlier census counted objects, the
    // machine composes by name.
    assert_eq!(defined, 692, "the data defines 692 nameable effect-on-conditions");
    assert_eq!(whole, 170, "and the port can run 170 of them whole");
}

/// A `run_eocs` entry can be a program written where it is used.
///
/// Reference: `load_eoc_vector_id_and_var` (`npctalk.cpp:512-541`) accepts a name or an object, and an object without an `id` is an
/// **inline** effect-on-condition (`load_inline_eoc`) run on the spot. The port runs it without installing it, because an inline
/// program has no id anything else could name.
#[test]
fn a_program_can_run_another_written_where_it_is_used() {
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_CALLER",
             "effect": [{"run_eocs": [{"effect": [{"u_message": "inline"}, {"math": ["count += 2"]}]}]}]}
        ]"#,
    );
    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_CALLER", 0),
        Applied::Done
    );
    assert_eq!(messages::lines(&world), vec!["inline".to_owned()]);
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "count"),
        Some(2.0),
        "and the inline program changed the caller's own values"
    );

    // An inline program with a condition of its own is respected.
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_GUARDED_CALL",
             "effect": [{"run_eocs": [{"condition": {"math": ["armed", "==", 1]},
                                      "effect": [{"u_message": "bang"}]}]}]}
        ]"#,
    );
    eoc_registry::run_program(&mut world, survivor, None, "EOC_GUARDED_CALL", 0);
    assert!(messages::lines(&world).is_empty(), "the guard held it back");

    // An entry carrying `variables` is refused: those values reach the program through a scope the port has not ported, and reading it
    // while dropping them would run a different program.
    let value = cdda_json::parse(
        br#"{"run_eocs": [{"id": "EOC_X", "variables": {"who": "u"}}]}"#.as_slice(),
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_effect(&value).is_none());
}
