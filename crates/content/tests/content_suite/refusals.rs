//! Which entry of a program refuses, and what it looks like.
//!
//! The simulation's one-gap census blames a **member name**, and a member name is not the same thing as the entry that refuses: fixing `location_variable_adjust`'s
//! reader left its program refusing for another reason. This test asks the question the census cannot: for every program the port cannot read, **the first entry whose
//! reading fails, with its JSON**, grouped so the biggest cause is visible at a glance.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Every `.json` under the reference's data, by path.
fn json_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "json") {
                found.push(path);
            }
        }
    }
    found
}

/// The name of the member an effect entry is built from, which is how the data spells its rule.
/// **The member whose name no reader knows**, which is what a refusal actually says.
///
/// The first version named the first member of the object, and that label was wrong twice over: `{"args": .., "trigger_event": ..}` blamed `args` when the truth was that
/// `trigger_event` is not ported at all, and `{"//": .., "<unknown>": ..}` blamed the comment. A member is unknown when a one-member object holding it does not read, so the walk
/// asks each member on its own and names the first that fails - which is the name a round would have to port.
fn member_of(entry: &cdda_json::Value) -> String {
    let cdda_json::Value::Object(object) = entry else {
        return "<not an object>".to_owned();
    };
    let named: Vec<&(String, cdda_json::Value)> = object
        .members()
        .iter()
        .filter(|(name, _)| !name.starts_with("//"))
        .collect();
    let mut unknown: Vec<String> = Vec::new();
    for (name, value) in &named {
        let mut alone = cdda_json::Object::new();
        alone.insert(name.clone(), (*value).clone());
        if cdda_content::dialogue::read_effect(&cdda_json::Value::Object(alone)).is_none() {
            unknown.push(name.clone());
        }
    }
    // **Every** unknown member is named, joined by `+`. Naming only the first hid the real cause twice: `{"args": .., "trigger_event": ..}` reported `args`, and the effect the
    // round would have to write is `trigger_event`; a trade entry reported `count` when the effect is `u_sell_item`.
    if !unknown.is_empty() {
        return unknown.join("+");
    }
    named
        .first()
        .map_or_else(|| "<empty>".to_owned(), |(name, _)| name.clone())
}

/// Renders a value as JSON text, briefly.
fn brief(entry: &cdda_json::Value) -> String {
    let text = format!("{entry:?}");
    text.chars().take(220).collect()
}

/// The first entry that refuses, descending through the containers an effect list can carry.
///
/// A first pass blamed `else` seven times - but the data has **no** object with `else` and no `if`, so the label was the first member of an `if` object whose real
/// refusal sat inside it. The walk below follows `then`, `else`, `effect` and `cases` into their lists, so the answer is the entry whose own reading fails rather than
/// the container that holds it.
fn first_refusal(entry: &cdda_json::Value, depth: usize) -> Option<(String, String)> {
    if cdda_content::dialogue::read_effect(entry).is_some() || depth > 6 {
        return None;
    }
    if let cdda_json::Value::Object(object) = entry {
        for member in ["then", "else", "effect"] {
            if let Some(cdda_json::Value::Array(children)) = object.get(member) {
                for child in children {
                    if let Some(deeper) = first_refusal(child, depth + 1) {
                        return Some(deeper);
                    }
                    if cdda_content::dialogue::read_effect(child).is_none() {
                        return Some((member_of(child), brief(child)));
                    }
                }
            }
        }
        if let Some(cdda_json::Value::Array(cases)) = object.get("cases") {
            for case in cases {
                let cdda_json::Value::Object(given) = case else {
                    continue;
                };
                let Some(child) = given.get("effect") else {
                    continue;
                };
                // **The case's effects are a list**, and blaming the list itself said `<not an object>` for every `switch` refusal - a label that named the walker's own
                // step rather than the entry that refused. The walk goes into the elements, which is what the `then`/`else`/`effect` loop above already does.
                if let cdda_json::Value::Array(children) = child {
                    for element in children {
                        if let Some(deeper) = first_refusal(element, depth + 1) {
                            return Some(deeper);
                        }
                        if cdda_content::dialogue::read_effect(element).is_none() {
                            return Some((member_of(element), brief(element)));
                        }
                    }
                    continue;
                }
                if let Some(deeper) = first_refusal(child, depth + 1) {
                    return Some(deeper);
                }
            }
        }
    }
    Some((member_of(entry), brief(entry)))
}

#[test]
fn the_first_entry_that_refuses_says_which_rule_is_missing() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut examples: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut refused_programs = BTreeSet::new();
    for path in json_files(&data) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(cdda_json::Value::Array(entries)) = cdda_json::parse(text.as_bytes()) else {
            continue;
        };
        for entry in entries {
            let cdda_json::Value::Object(object) = &entry else {
                continue;
            };
            if object.get("type") != Some(&cdda_json::Value::Str("effect_on_condition".to_owned())) {
                continue;
            }
            let Some(cdda_json::Value::Array(effects)) = object.get("effect") else {
                continue;
            };
            for effect in effects {
                let Some((member, example)) = first_refusal(effect, 0) else {
                    continue;
                };
                let id = object
                    .get("id")
                    .and_then(|given| match given {
                        cdda_json::Value::Str(id) => Some(id.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("{path:?}"));
                let counted = examples.entry(member.clone()).or_insert((0, format!("{id}: {example}")));
                counted.0 += 1;
                refused_programs.insert(id);
                break;
            }
        }
    }
    let mut ranked: Vec<(String, usize, String)> = examples
        .into_iter()
        .map(|(member, (count, example))| (member, count, example))
        .collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let shown: Vec<String> = ranked
        .iter()
        .take(12)
        .map(|(member, count, example)| format!("{member} x{count}: {example}"))
        .collect();
    println!("{} programs refuse at their first unreadable entry", refused_programs.len());
    for line in &shown {
        println!("  {line}");
    }
    for line in shown {
        println!("REFUSED {line}");
    }
    // Pinned so a round that changes the frontier sees it here first: the total has fallen from 35 to **5**. Every bucket the round-976 roadmap listed is gone, and what is
    // left is one program per cause: `args`, `count`, `else`, `if`, `loc`. `false_eocs` closed at round 1484, when `search_data` learned the `condition` member. The seventh, `radius`, closed when `reveal_map` learned its `dbl_or_var` radius.
    assert_eq!(refused_programs.len(), 4, "four programs refuse at their first unreadable entry");
    assert_eq!(
        ranked
            .iter()
            .find(|(member, _, _)| member == "key")
            .map(|(_, count, _)| *count),
        None,
        "and no program names a place to revert any more: the store it needed landed in round 1054"
    );
}

/// The one `location_variable_adjust` the census still blames, read directly.
///
/// It sits in `traps.json` and adjusts a **context variable** by an expression of two random draws. The rounded braces are the reference's XOR, which its expression parser
/// reads as a power; the reading question is whether the port can carry the shape at all.
#[test]
fn a_context_variable_can_be_adjusted_by_an_expression() {
    let text = r#"{"location_variable_adjust": {"context_val": "trap_location"}, "//": "Adjust by between 2 and 12 in either direction", "x_adjust": {"math": ["((-1) ^ rand(1)) * (rand(10) + 2)"]}, "y_adjust": {"math": ["((-1) ^ rand(1)) * (rand(10) + 2)"]}}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    assert!(
        cdda_content::dialogue::read_effect(&value).is_some(),
        "the port reads this adjust: {value:?}"
    );
}
