//! The accept-lists, asked of real containers and real payloads.
//!
//! `corpus/items/accepts.json` is `--dump-accepts`: twelve containers — chosen because their pockets ask something specific — against every twenty-fifth item
//! type, with `item_pocket::can_contain`'s own answer and its own message for each of the container's pockets, in order. 4,752 cases, and the messages say which
//! rule refused: 4,696 of the refusals are the accept-list's `"holster does not accept this item type or form factor"`, and the rest are the length, volume,
//! weight, ammunition and liquid rules.
//!
//! **What this file tests is the one rule, exactly.** The accept-list check sits inside `is_compatible`, which runs before every other check, so a payload that
//! hears any *other* message has already passed the list. That makes the equivalence tight in both directions: the port's list refuses a payload **if and only
//! if** the reference refused it with the accept-list message. Nothing here is a product of my reading: the message is the oracle.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::items::ItemType;
use cdda_json::Value;
use cdda_sim::{Pocket, accepts};
use std::collections::{BTreeMap, BTreeSet};

/// The message the reference gives a payload its accept-lists refuse.
const REFUSED: &str = "holster does not accept this item type or form factor";

/// One payload tried against one container, with the reference's verdict for each of the container's pockets.
struct Case {
    container: String,
    payload: String,
    /// For each pocket, in order: whether the reference refused it **for the accept-lists**.
    refused_by_list: Vec<bool>,
}

/// Every item the reference defines, from the reference checkout's own data.
fn items_loaded() -> Vec<ItemType> {
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    cdda_content::items::load_paths(&paths).expect("the items load")
}

fn dump() -> Vec<Case> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/accepts.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("the dump holds cases")
    };
    cases
        .iter()
        .map(|case| {
            let Value::Object(case) = case else {
                panic!("a case")
            };
            let text = |key: &str| match case.get(key) {
                Some(Value::Str(value)) => value.clone(),
                other => panic!("{key} is text: {other:?}"),
            };
            let Some(Value::Array(verdicts)) = case.get("verdicts") else {
                panic!("the verdicts")
            };
            Case {
                container: text("container"),
                payload: text("payload"),
                refused_by_list: verdicts
                    .iter()
                    .map(|verdict| {
                        let Value::Object(verdict) = verdict else {
                            panic!("a verdict")
                        };
                        matches!(verdict.get("why"), Some(Value::Str(why)) if why == REFUSED)
                    })
                    .collect(),
            }
        })
        .collect()
}

/// The base material the reference would pick: the one with the largest portion, the first alphabetically when they are all one
/// (`item::get_base_material`, `src/item.cpp:11002-11020`).
fn base_material(item: &ItemType) -> String {
    let mut best: Option<(&str, i32)> = None;
    for (material, portion) in &item.materials {
        match best {
            Some((_, best_portion)) if best_portion >= *portion => {}
            _ => best = Some((material, *portion)),
        }
    }
    best.map_or_else(String::new, |(material, _)| material.to_owned())
}

#[test]
fn the_port_refuses_exactly_the_payloads_the_reference_refuses_for_their_lists() {
    let cases = dump();
    assert_eq!(cases.len(), 4_752, "every recorded case");

    let types: BTreeMap<String, ItemType> = items_loaded()
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    let mut differing: Vec<String> = Vec::new();
    let mut refused = 0;
    for case in &cases {
        let container = types
            .get(&case.container)
            .unwrap_or_else(|| panic!("{} is a type", case.container));
        let payload = types
            .get(&case.payload)
            .unwrap_or_else(|| panic!("{} is a type", case.payload));
        let pockets: Vec<Pocket> = container
            .pockets
            .iter()
            .filter(|pocket| pocket.kind == "CONTAINER")
            .map(Pocket::from_spec)
            .collect();
        assert_eq!(
            pockets.len(),
            case.refused_by_list.len(),
            "{} pockets were asked",
            case.container
        );
        let flags: BTreeSet<String> = payload.flags.clone();
        let material = base_material(payload);
        for (index, pocket) in pockets.iter().enumerate() {
            let port_refuses = !accepts(pocket, &payload.id, &flags, &material);
            let reference_refuses = case.refused_by_list.get(index).copied().unwrap_or(false);
            refused += usize::from(reference_refuses);
            if port_refuses != reference_refuses {
                differing.push(format!(
                    "{} into {} (pocket {index}): the port says {}, the reference says {}",
                    case.payload,
                    case.container,
                    if port_refuses { "refused" } else { "accepted" },
                    if reference_refuses { "refused" } else { "accepted" }
                ));
            }
        }
    }
    assert!(
        refused > 4_000,
        "the corpus has to contain list refusals, or the rule is untested: it has {refused}"
    );
    assert!(
        differing.is_empty(),
        "{} of {} verdicts disagree; the first twenty are {:#?}",
        differing.len(),
        cases.len(),
        differing.get(..20).unwrap_or(&differing)
    );
}
