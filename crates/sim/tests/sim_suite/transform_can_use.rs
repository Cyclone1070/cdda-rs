//! Whether a `transform` action may be used at all, over all 459 of them and two standings each.
//!
//! `corpus/items/transform-can-use.json` is `--dump-transform-can-use`: the reference asks its own `iuse_transform::can_use` about every item in a **bare** standing and
//! a **wielded** one, and writes the verdict and the sentence it gave. That sentence is the part worth having: the conditions are asked in a fixed order, so the
//! answer says *which* one refused, not merely that something did.
//!
//! The messages carry the item's name formatted in, and `item::tname` is a later P3 piece, so the gate compares the **template**: the recorded sentence must begin with
//! what comes before the name and end with what comes after it. That is enough to tell the four refusals apart, because their prefixes are distinct and one of them
//! (`"Your internal reservoir is empty."`) has no name in it at all.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::UseAction;
use cdda_json::Value;
use cdda_sim::transform;
use std::collections::{BTreeMap, BTreeSet};

fn recorded() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/transform-can-use.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

fn loaded() -> BTreeMap<String, ItemType> {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect()
}

fn text(entry: &cdda_json::Object, key: &str) -> String {
    match entry.get(key) {
        Some(Value::Str(text)) => text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

fn int(entry: &cdda_json::Object, key: &str) -> i64 {
    match entry.get(key) {
        Some(Value::Int(number)) => *number,
        other => panic!("{key}: {other:?}"),
    }
}

fn boolean(entry: &cdda_json::Object, key: &str) -> bool {
    match entry.get(key) {
        Some(Value::Bool(flag)) => *flag,
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn every_transform_refuses_in_the_reference_s_own_order_and_words() {
    let Value::Object(root) = recorded() else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    assert_eq!(types.len(), 459, "every transform was recorded");
    let items = loaded();
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let qualities = cdda_content::qualities::load_paths(&paths).expect("the qualities load");

    let mut differing: Vec<String> = Vec::new();
    let mut refusals: BTreeSet<String> = BTreeSet::new();
    let mut allowed = 0;
    let mut quality_verdicts = 0;
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let id = text(entry, "id");
        let action = text(entry, "action");
        let Some(item) = items.get(&id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let Some(UseAction::Transform(actor)) = item.use_actions.get(&action) else {
            differing.push(format!("{id}: the port has no {action}"));
            continue;
        };
        let Some(Value::Array(states)) = entry.get("states") else {
            panic!("{id} has states")
        };
        for state in states {
            let Value::Object(state) = state else {
                panic!("a state")
            };
            let standing = transform::Standing {
                // The recording's two standings: a bare item is neither worn nor wielded, a wielded one is only wielded. Nothing in either has contents, and the
                // charges are the ones the item was built with.
                worn: false,
                wielded: text(state, "state") != "bare",
                empty: true,
                charges: i32::try_from(int(state, "charges")).expect("a charge count"),
            };
            // **The qualities' message is fully determined**, name and all, because it is built from the quality registry rather than from the item: the recorded
            // sentence is compared whole, not as a template.
            if !actor.qualities_needed.is_empty() {
                quality_verdicts += 1;
                if let Some(disagreement) =
                    quality_refusal(actor, &standing, &qualities, &text(state, "message"))
                {
                    differing.push(format!("{id}: {disagreement}"));
                }
                continue;
            }
            let expected_ok = boolean(state, "ok");
            let message = text(state, "message");
            // The standing carries no tools, so the character offers no quality at all.
            let ours = transform::can_use(actor, &standing, |_, _| false);
            match (&ours, expected_ok) {
                (Ok(()), true) => allowed += 1,
                (Ok(()), false) => differing.push(format!(
                    "{id} ({}): the port allows it, the reference says {message:?}",
                    text(state, "state")
                )),
                (Err(refusal), true) => differing.push(format!(
                    "{id} ({}): the port refuses with {refusal:?}, the reference allows it",
                    text(state, "state")
                )),
                (Err(refusal), false) => {
                    refusals.insert(format!("{refusal:?}"));
                    let template = refusal.template(actor);
                    // The name is formatted into the middle, so the recorded sentence is the template with something in place of the placeholder — or, for a
                    // template with none, exactly the template. **Two placeholder spellings**, because they come from two places: `can_use`'s own three sentences use
                    // `%1$s` and an actor's `need_charges_msg` is formatted with `%s`.
                    let (prefix, suffix) = match template.split_once("%1$s") {
                        Some(pair) => pair,
                        None => template.split_once("%s").unwrap_or((template, "")),
                    };
                    if !(message.starts_with(prefix) && message.ends_with(suffix)) {
                        differing.push(format!(
                            "{id} ({}): the port says {template:?}, the reference says {message:?}",
                            text(state, "state")
                        ));
                    }
                }
            }
        }
    }
    // **Non-vacuity**: the corpus has to reach the order, not just one branch. `NotWorn` before `NotWielded` is the ordering rule, and both have to appear.
    assert!(
        refusals.contains("NotWorn"),
        "the wearing rule has to fire: {refusals:?}"
    );
    assert!(
        refusals.contains("NotWielded"),
        "and so does the wielding rule: {refusals:?}"
    );
    assert!(allowed > 600, "most standings are allowed: {allowed}");
    assert_eq!(
        quality_verdicts, 9,
        "three types state qualities, in three standings each"
    );
    // **What this corpus cannot see, said out loud.** No transform states both `need_worn` and `need_wielding`, so no standing in this recording can tell the order
    // of those two rules apart; that ordering is pinned by a transcribed unit test beside the code instead (`src/iuse_actor.cpp:433-441`).
    let both = items
        .values()
        .flat_map(|item| item.use_actions.values())
        .filter(|action| match action {
            UseAction::Transform(actor) => actor.need_worn && actor.need_wielding,
            _ => false,
        })
        .count();
    assert_eq!(
        both, 0,
        "no actor in this content states both position rules"
    );
    assert!(
        differing.is_empty(),
        "{} verdicts disagree; the first ten are {:#?}",
        differing.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

/// The qualities' refusal, compared whole: the sentence names the quality **and its level**, and both come from the registry rather than from the item.
fn quality_refusal(
    actor: &cdda_content::use_actions::Transform,
    standing: &transform::Standing,
    qualities: &BTreeMap<String, cdda_content::qualities::Quality>,
    message: &str,
) -> Option<String> {
    let Err(transform::Refusal::MissingQualities(unmet)) =
        transform::can_use(actor, standing, |_, _| false)
    else {
        return Some("nothing offers the quality, so it must refuse".to_owned());
    };
    let ours = transform::quality_message(&unmet, |quality| {
        qualities
            .get(quality)
            .map_or_else(|| quality.to_owned(), |def| def.name.clone())
    });
    (ours != message).then(|| format!("the port says {ours:?}, the reference says {message:?}"))
}
