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
        whole, 217,
        "two hundred and seventeen run whole: morale added fourteen, snippets eight"
    );
    assert_eq!(refused.get("effect"), Some(&344), "and the effect side is still the bigger refusal");
    assert_eq!(refused.get("u_has_trait"), None, "traits are no longer blamed for anything");
    assert_eq!(refused.get("math"), Some(&46), "`math` guards are the biggest single one");
    assert_eq!(refused.get("u_has_item"), None, "carrying an item is no longer blamed for anything");
    // The chance guards now roll, so what is left of them is one program apiece - and the eight programs they freed are blamed on
    // what refused them next, which is the "moves house" pattern for the fifth time.
    assert_eq!(refused.get("one_in_chance"), Some(&2), "the dice no longer block the chance guards");
    assert_eq!(refused.get("test_eoc"), None, "and `test_eoc` no longer blocks anything");
    assert_eq!(refused.get("is_season"), None, "the season is answered now that the world has a clock");
    assert_eq!(refused.get("expects_vars"), None, "and context values are handed over now");
    assert_eq!(refused.get("x_in_y_chance"), Some(&1), "both of them, down to one each");
    assert_eq!(refused.get("u_at_om_location"), None, "the overmap terrain answers now");
    // 692, not the 712 effect-on-condition objects in the data: twenty carry no `id`, so nothing can name them and the port does
    // not install them. The count of definitions was itself worth learning - the 712 in the earlier census counted objects, the
    // machine composes by name.
    assert_eq!(defined, 692, "the data defines 692 nameable effect-on-conditions");
    assert_eq!(whole, 217, "and the port can run 217 of them whole");
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

    // An entry carrying `variables` is read now that the port has a context scope: the values reach the program as its context, and
    // go when the call returns (see `a_call_hands_the_program_its_values_and_takes_them_back`).
    let value = cdda_json::parse(
        br#"{"run_eocs": [{"id": "EOC_X", "variables": {"who": "u"}}]}"#.as_slice(),
    )
    .expect("json");
    assert!(cdda_content::dialogue::read_effect(&value).is_some());
}

/// A call can hand a program context values, and the program can read them.
///
/// Reference: `f_run_eocs` (`npctalk.cpp:6332-6338`) evaluates each `variables` member into the invocation's context and lets it go when
/// the call returns; `context_val( '…' )` reads it and `expects_vars` (`condition.cpp:1111-1131`) asks whether it is there.
#[test]
fn a_call_hands_the_program_its_values_and_takes_them_back() {
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_WORKER",
             "condition": {"expects_vars": ["who"]},
             "effect": [{"math": ["seen = context_val('who')"]}]},
            {"type": "effect_on_condition", "id": "EOC_CALLER",
             "effect": [{"run_eocs": [{"id": "EOC_WORKER", "variables": {"who": "7"}}]}]}
        ]"#,
    );
    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_CALLER", 0),
        Applied::Done
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "seen"),
        Some(7.0),
        "the program read the value the call handed it"
    );
    assert!(
        cdda_sim::dialogue_context::values(&world, survivor).is_empty(),
        "and the context was gone when the call returned"
    );

    // Called with nothing, the same program's `expects_vars` holds it back.
    let (mut world, survivor) = world_with(
        r#"[
            {"type": "effect_on_condition", "id": "EOC_WORKER",
             "condition": {"expects_vars": ["who"]},
             "effect": [{"math": ["seen = context_val('who')"]}]},
            {"type": "effect_on_condition", "id": "EOC_RUDE",
             "effect": [{"run_eocs": ["EOC_WORKER"]}]}
        ]"#,
    );
    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_RUDE", 0),
        Applied::Done
    );
    assert_eq!(
        dialogue_values::number_of(&world, survivor, "seen"),
        None,
        "an unmet expectation stopped the program rather than reading a zero"
    );
}

/// How many readable programs actually **run** in a fresh world, which the readability census cannot say.
///
/// The census above counts a program as runnable whole when its condition and every effect can be *read*. That is a statement about the
/// data, not about behaviour: `u_at_om_location` reads now, but a world nothing has generated answers it false, and an effect can read and
/// still refuse at run time (`rng` with no dice, a context value nobody handed in). This test runs every readable program against a fresh
/// world and counts what came back, so the two numbers can never again be confused for each other.
#[test]
fn the_readable_programs_also_run_in_a_fresh_world() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut defined = 0usize;
    let mut readable: Vec<cdda_content::dialogue::Program> = Vec::new();

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
            for definition in definitions_in(&value) {
                if member(definition, "id").is_none() {
                    continue;
                }
                defined += 1;
                if let Some(program) = cdda_content::dialogue::read_program(definition) {
                    readable.push(program);
                }
            }
        }
    }

    // Every readable program is installed in the world each one runs in, because that is what a world is: the data's whole set of
    // programs, with any of them callable. Installing only the program under test made ninety of the ninety-eight refusals a `run_eocs`
    // call to a program that was simply not there - a fact about the census, not about the port.
    let mut ran = 0usize;
    let mut refused = 0usize;
    let mut runtime_blame: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for program in &readable {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
        cdda_sim::eoc_registry::set_programs(&mut world, &readable);
        if eoc_registry::run_program(&mut world, survivor, None, &program.id, 0) == Applied::Done {
            ran += 1;
            continue;
        }
        refused += 1;
        // Name the effect that refused **at run time**, by running them one at a time in their own fresh world. This is the same device
        // as the read-side blame, one level down: the read census says which member cannot be understood, this says which one cannot be
        // carried out.
        if let Some(culprit) = first_refusing_effect(program, &readable, 0) {
            *runtime_blame.entry(culprit).or_insert(0) += 1;
        }
    }

    let mut ranked: Vec<(String, usize)> = runtime_blame.into_iter().collect();
    ranked.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    ranked.truncate(6);
    println!(
        "of {defined} programs: {} readable, {ran} ran in a fresh world, {refused} refused at run time; \
         the effects that refused: {ranked:?}",
        readable.len()
    );

    assert_eq!(defined, 692, "the data defines 692 nameable effect-on-conditions");
    assert_eq!(readable.len(), 217, "the port reads 217 of them");
    assert_eq!(ran, 112, "a hundred and twelve of the readable programs also ran");
    assert_eq!(refused, 105, "and a hundred and five refused at run time");
    assert_eq!(ran + refused, readable.len(), "every readable program did one or the other");
}

/// The effect that refuses, following a `run_eocs` call into the program it names.
///
/// A program that calls another inherits the callee's refusal, so reporting `run_eocs` would say only "somewhere below" - the same trap as
/// reporting the joiner `and`. The search therefore descends into the callee and names the effect that actually refused.
fn first_refusing_effect(
    program: &cdda_content::dialogue::Program,
    installed: &[cdda_content::dialogue::Program],
    depth: usize,
) -> Option<String> {
    if depth >= 8 {
        return Some("run_eocs".to_owned());
    }
    for effect in &program.effects {
        let mut alone = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut alone, "the survivor", (0, 0, 0));
        cdda_sim::singletons::put_rolls(&mut alone, cdda_core_types::rng::Rolls::new(1));
        cdda_sim::eoc_registry::set_programs(&mut alone, installed);
        if cdda_sim::dialogue_effects::run_effect(&mut alone, survivor, None, effect)
            != Applied::CannotAnswer
        {
            continue;
        }
        return match effect {
            cdda_content::dialogue::Effect::RunEoc(calls) => calls
                .iter()
                .find_map(|call| match call {
                    cdda_content::dialogue::ProgramRef::Named { id, .. } => installed
                        .iter()
                        .find(|program| program.id == *id)
                        .and_then(|program| first_refusing_effect(program, installed, depth + 1)),
                    cdda_content::dialogue::ProgramRef::Inline { program, .. } => {
                        first_refusing_effect(program, installed, depth + 1)
                    }
                })
                .or_else(|| Some("run_eocs(calls a program that is not installed)".to_owned())),
            _ => Some(effect.kind().to_owned()),
        };
    }
    None
}

/// One member of a JSON object, or nothing when the value is not an object.
fn member<'a>(value: &'a cdda_json::Value, key: &str) -> Option<&'a cdda_json::Value> {
    match value {
        cdda_json::Value::Object(object) => object.get(key),
        _ => None,
    }
}

/// Every effect-on-condition object directly inside a JSON value, without descending into the ones it finds.
fn definitions_in(value: &cdda_json::Value) -> Vec<&cdda_json::Value> {
    let mut found = Vec::new();
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            cdda_json::Value::Array(items) => stack.extend(items.iter()),
            cdda_json::Value::Object(object) => {
                if matches!(
                    object.get("type"),
                    Some(cdda_json::Value::Str(kind)) if kind == "effect_on_condition"
                ) {
                    found.push(value);
                    continue;
                }
                for (_, member) in object.members() {
                    stack.push(member);
                }
            }
            _ => {}
        }
    }
    found
}

/// Which unreadable program blocks the most callers, so the next leaf is chosen by leverage rather than by frequency.
///
/// The runtime census showed ninety of ninety-seven refusals are a call to a program the port cannot read. A call graph makes that
/// actionable: for every readable program, the programs reachable from it that the port cannot read are the reason it will not run, and
/// counting how many readable programs reach each unreadable one says which single gap is worth closing first.
#[test]
fn the_call_graph_says_which_missing_program_blocks_the_most() {
    use std::collections::{BTreeMap, BTreeSet};

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut defined = 0usize;
    let mut readable: BTreeSet<String> = BTreeSet::new();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();

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
            for definition in definitions_in(&value) {
                let Some(cdda_json::Value::Str(id)) = member(definition, "id") else {
                    continue;
                };
                defined += 1;
                if cdda_content::dialogue::read_program(definition).is_some() {
                    readable.insert(id.clone());
                }
                edges.insert(id.clone(), programs_called(definition));
            }
        }
    }

    // For each readable program, which unreadable programs does it reach?
    let mut blocked_by: BTreeMap<String, usize> = BTreeMap::new();
    for root in &readable {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut queue: Vec<String> = edges.get(root).cloned().unwrap_or_default();
        while let Some(next) = queue.pop() {
            if !seen.insert(next.clone()) {
                continue;
            }
            if !readable.contains(&next) {
                *blocked_by.entry(next).or_insert(0) += 1;
                continue;
            }
            queue.extend(edges.get(&next).cloned().unwrap_or_default());
        }
    }

    let mut ranked: Vec<(String, usize)> = blocked_by.into_iter().collect();
    ranked.sort_by_key(|(name, count)| (std::cmp::Reverse(*count), name.clone()));
    ranked.truncate(8);
    println!(
        "of {defined} programs, {} readable; the unreadable programs that block the most readable callers: {ranked:?}",
        readable.len()
    );

    assert_eq!(defined, 692, "the data defines 692 nameable effect-on-conditions");
    assert_eq!(readable.len(), 217, "the port reads 217 of them");
    assert!(!ranked.is_empty(), "and something is blocking something");
}

/// Every program id a definition calls, through `run_eocs` and `test_eoc`, at any depth inside it.
fn programs_called(definition: &cdda_json::Value) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![definition];
    while let Some(value) = stack.pop() {
        match value {
            cdda_json::Value::Array(items) => stack.extend(items.iter()),
            cdda_json::Value::Object(object) => {
                for (key, member) in object.members() {
                    if key == "run_eocs" || key == "test_eoc" {
                        collect_called_ids(member, &mut found);
                    }
                    stack.push(member);
                }
            }
            _ => {}
        }
    }
    found
}

/// The ids a `run_eocs` or `test_eoc` member names, in any of its three shapes.
fn collect_called_ids(value: &cdda_json::Value, found: &mut Vec<String>) {
    match value {
        cdda_json::Value::Str(id) => found.push(id.clone()),
        cdda_json::Value::Array(items) => {
            for item in items {
                collect_called_ids(item, found);
            }
        }
        cdda_json::Value::Object(object) => {
            if let Some(cdda_json::Value::Str(id)) = object.get("id") {
                found.push(id.clone());
            }
            for (_, member) in object.members() {
                if member != value {
                    stack_member(member, found);
                }
            }
        }
        _ => {}
    }
}

/// Follows one member of a call object, looking for a further `run_eocs` or `test_eoc` inside it.
fn stack_member(value: &cdda_json::Value, found: &mut Vec<String>) {
    match value {
        cdda_json::Value::Array(items) => {
            for item in items {
                stack_member(item, found);
            }
        }
        cdda_json::Value::Object(object) => {
            for (key, member) in object.members() {
                if key == "run_eocs" || key == "test_eoc" {
                    collect_called_ids(member, found);
                } else {
                    stack_member(member, found);
                }
            }
        }
        _ => {}
    }
}

/// How far each unreadable program is from being complete, which is what decides whether porting a member buys anything.
///
/// The call graph said which programs are blocked; it could not say whether one more rule would finish them. An effect list refuses as a whole,
/// so a program with nine unreadable members gains nothing from the ninth being ported - while a program with **one** gap becomes runnable the day
/// that gap closes. This census counts the gaps and reports the members that are the *only* thing standing between the port and a program.
#[test]
fn the_programs_one_gap_away_say_what_to_port_next() {
    use std::collections::BTreeMap;

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut defined = 0usize;
    let mut one_gap: BTreeMap<String, usize> = BTreeMap::new();
    let mut gaps_total = 0usize;

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
            for definition in definitions_in(&value) {
                if member(definition, "id").is_none() {
                    continue;
                }
                defined += 1;
                if cdda_content::dialogue::read_program(definition).is_some() {
                    continue;
                }
                let gaps = unreadable_members(definition);
                gaps_total += gaps.len();
                if let [only] = gaps.as_slice() {
                    *one_gap.entry(only.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut ranked: Vec<(String, usize)> = one_gap.into_iter().collect();
    ranked.sort_by_key(|(name, count)| (std::cmp::Reverse(*count), name.clone()));
    ranked.truncate(8);
    let single = ranked.iter().map(|(_, count)| count).sum::<usize>();
    println!(
        "of {defined} programs, {gaps_total} unreadable members in total; {single} programs are one gap away, and the members that \
         would finish them: {ranked:?}"
    );

    assert_eq!(defined, 692, "the data defines 692 nameable effect-on-conditions");
    // 987 unreadable members across 511 programs, and 138 of those programs are **one member** from running.
    //
    // The ranking's tail still carries two modifier names - `duration` and `bonus` - because a refused entry reports its first key, and a
    // modifier leads the object it sits in. They are listed so the number can be read honestly rather than silently dropped: the effect
    // refusing underneath them is what a round would have to port.
    assert_eq!(gaps_total, 919, "the data's programs hold 919 unreadable leaves");
    assert_eq!(single, 140, "a hundred and forty programs are one leaf away");
    assert_eq!(
        ranked,
        vec![
            ("<nested>".to_owned(), 36),
            ("mapgen_update".to_owned(), 32),
            ("condition".to_owned(), 31),
            ("u_add_effect".to_owned(), 12),
            ("math".to_owned(), 10),
            ("hallucination_count".to_owned(), 7),
            ("max_radius".to_owned(), 6),
            ("u_lose_effect".to_owned(), 6),
        ],
        "the members that would finish the most programs, single gaps first"
    );
}

/// Every member of a program the port cannot read, named the way the data spells it.
///
/// A gap is counted per **leaf**, not per refused entry. An `open_dialogue` whose `true_eocs` cannot be read used to count as one gap and was
/// really two, which made the "one gap away" column an upper bound on what a round could win - and it was wrong three rounds running.
fn unreadable_members(definition: &cdda_json::Value) -> Vec<String> {
    let mut gaps = Vec::new();
    if let Some(condition) = member(definition, "condition")
        && cdda_content::dialogue::read_condition(condition).is_none()
    {
        gaps.push("condition".to_owned());
    }
    for slot in ["effect", "true_effect", "false_effect"] {
        let Some(effects) = member(definition, slot) else {
            continue;
        };
        let entries = match effects {
            cdda_json::Value::Array(items) => items.as_slice(),
            other => std::slice::from_ref(other),
        };
        for entry in entries {
            collect_leaves(entry, &mut gaps);
        }
    }
    gaps
}

/// Whether a key describes an effect rather than being one: such a member is never the reason an entry refuses.
fn is_descriptor(key: &str) -> bool {
    matches!(
        key,
        "duration" | "bonus" | "intensity" | "popup" | "sound" | "same_snippet" | "target_part" | "target_var"
    )
}

/// Whether a key names a container rather than a member the port could port.
///
/// A container's own name is reported as a residue (`<nested>`) so the table does not promise a rule that does not exist.
fn is_container_member(key: &str) -> bool {
    matches!(key, "then" | "else" | "cases" | "true_eocs" | "false_eocs")
}

/// Adds one gap name for every place inside an effect entry that the port cannot read.
fn collect_leaves(entry: &cdda_json::Value, gaps: &mut Vec<String>) {
    if cdda_content::dialogue::read_effect(entry).is_some() {
        return;
    }
    let cdda_json::Value::Object(object) = entry else {
        gaps.push("<named>".to_owned());
        return;
    };
    if matches!(object.get("snippet"), Some(cdda_json::Value::Bool(true))) {
        gaps.push("snippet".to_owned());
        return;
    }
    let Some((key, value)) = object
        .members()
        .iter()
        .find(|(key, _)| !is_descriptor(key))
    else {
        gaps.push("<named>".to_owned());
        return;
    };
    // The containers an effect can hold: descend and count what refuses inside them.
    let nested = match key.as_str() {
        "if" => {
            let mut found = Vec::new();
            for branch in ["then", "else"] {
                if let Some(cdda_json::Value::Array(entries)) = object.get(branch) {
                    for entry in entries {
                        collect_leaves(entry, &mut found);
                    }
                }
            }
            Some(found)
        }
        "switch" => {
            let mut found = Vec::new();
            if let Some(cdda_json::Value::Array(cases)) = object.get("cases") {
                for case in cases {
                    let cdda_json::Value::Object(case) = case else {
                        continue;
                    };
                    let effects = match case.get("effect") {
                        Some(cdda_json::Value::Array(items)) => items.as_slice(),
                        Some(other) => std::slice::from_ref(other),
                        None => &[],
                    };
                    for entry in effects {
                        collect_leaves(entry, &mut found);
                    }
                }
            }
            Some(found)
        }
        "weighted_list_eocs" => {
            let mut found = Vec::new();
            if let Some(cdda_json::Value::Array(entries)) = object.get(key) {
                for entry in entries {
                    let cdda_json::Value::Array(pair) = entry else {
                        continue;
                    };
                    if let Some(first) = pair.first() {
                        collect_leaves(first, &mut found);
                    }
                }
            }
            Some(found)
        }
        _ => None,
    };
    if let Some(found) = nested
        && !found.is_empty()
    {
        gaps.extend(found);
        return;
    }
    let _ = value;
    if is_container_member(key) {
        gaps.push("<nested>".to_owned());
    } else {
        gaps.push(key.clone());
    }
}




/// The snippet store keeps the data's categories and picks from a category by weight.
///
/// Reference: `snippet_library` (`text_snippets.cpp`): lines are grouped by `category`, each carries a weight, and a pick is uniform over the total.
#[test]
fn the_snippet_store_picks_a_line_by_weight_and_never_picks_a_weightless_one() {
    use cdda_core_types::rng::Rolls;

    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(7));
    cdda_sim::snippets::set_snippets(
        &mut world,
        [
            ("notes".to_owned(), line("a", "The first note.", 1)),
            ("notes".to_owned(), line("b", "The second note.", 3)),
            ("notes".to_owned(), line("c", "Never read.", 0)),
        ],
    );
    assert_eq!(cdda_sim::snippets::census(&world), (1, 3), "one category, three lines");

    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut dice = Rolls::new(7);
    for _ in 0..400 {
        let picked = cdda_sim::snippets::pick(&mut world, "notes", Some(&mut dice)).expect("a line");
        *seen.entry(picked.id).or_insert(0) += 1;
    }
    assert_eq!(seen.get("c"), None, "a line of weight zero is never picked");
    let a = seen.get("a").copied().unwrap_or(0);
    let b = seen.get("b").copied().unwrap_or(0);
    assert_eq!(a + b, 400);
    assert!(b > 2 * a, "the heavier line came up about three times as often: {b} to {a}");

    assert!(
        cdda_sim::snippets::pick(&mut world, "no_such_category", Some(&mut dice)).is_none(),
        "an unknown category picks nothing rather than something wrong"
    );
}

/// One snippet line, for the store's test.
fn line(id: &str, text: &str, weight: i32) -> cdda_sim::snippets::SnippetLine {
    cdda_sim::snippets::SnippetLine {
        id: id.to_owned(),
        text: text.to_owned(),
        weight,
    }
}
