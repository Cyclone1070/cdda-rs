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
use cdda_sim::{ItemFacts, Phase, Pocket, TypeWeight, accepts, check};
use std::collections::{BTreeMap, BTreeSet};

/// The message the reference gives a payload its accept-lists refuse.
const REFUSED: &str = "holster does not accept this item type or form factor";

/// One payload tried against one container, with the reference's verdict for each of the container's pockets.
struct Case {
    container: String,
    payload: String,
    /// For each pocket, in order: the reference's message, or the empty string when it accepted.
    verdicts: Vec<String>,
}

/// Every material the reference defines, because whether a thing is soft is a question about its materials.
fn materials() -> BTreeMap<String, cdda_content::materials::Material> {
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    cdda_content::materials::load_paths(&paths)
        .expect("the materials load")
        .into_iter()
        .map(|material| (material.id.clone(), material))
        .collect()
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
                verdicts: verdicts
                    .iter()
                    .map(|verdict| {
                        let Value::Object(verdict) = verdict else {
                            panic!("a verdict")
                        };
                        match verdict.get("why") {
                            Some(Value::Str(why)) => why.clone(),
                            other => panic!("why is text: {other:?}"),
                        }
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

/// The port's own refusal for a pocket and a payload, as the message the reference would have given.
fn pocket_check(pocket: &Pocket, facts: &ItemFacts) -> String {
    match check(pocket, facts, pocket.capacity_ml) {
        Ok(()) => String::new(),
        Err(refusal) => refusal.message(),
    }
}

/// What the reference measured of each payload when it constructed it: its weight, volume, length and charges.
struct Measured {
    weight_grams: i64,
    volume_ml: i64,
    length_mm: i64,
    charges: i32,
}

fn measures() -> BTreeMap<String, Measured> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/measures.json");
    let bytes = std::fs::read(&path).expect("the measures");
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    let mut measured = BTreeMap::new();
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let whole = |key: &str| match entry.get(key) {
            Some(Value::Int(value)) => *value,
            other => panic!("{key} is a whole number: {other:?}"),
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("an id")
        };
        measured.insert(
            id.clone(),
            Measured {
                weight_grams: whole("weight"),
                volume_ml: whole("volume"),
                length_mm: whole("length"),
                charges: i32::try_from(whole("charges")).unwrap_or(0).max(1),
            },
        );
    }
    measured
}

/// The weights the prototypes state, in the units the sim asks for.
#[allow(dead_code)]
fn weights() -> BTreeMap<String, TypeWeight> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/prototypes/items.json");
    let bytes = std::fs::read(&path).expect("the prototypes");
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("prototypes") else {
        panic!("the dump holds prototypes")
    };
    let mut weights = BTreeMap::new();
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a prototype")
        };
        let whole = |key: &str| match entry.get(key) {
            Some(Value::Int(value)) => *value,
            other => panic!("{key} is a whole number: {other:?}"),
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("an id")
        };
        let no_drop = match entry.get("flags") {
            Some(Value::Array(flags)) => flags
                .iter()
                .any(|flag| matches!(flag, Value::Str(name) if name == "NO_DROP")),
            _ => false,
        };
        weights.insert(
            id.clone(),
            TypeWeight {
                milligrams: whole("weight_mg"),
                integral_milligrams: whole("integral_weight_mg"),
                count_by_charges: matches!(
                    entry.get("count_by_charges"),
                    Some(Value::Bool(true))
                ),
                no_drop,
            },
        );
    }
    weights
}

/// The payload's facts, as the reference saw them: the port's own loader for the identity, the recorded numbers for the measures.
fn facts_of(
    item: &ItemType,
    measures: &BTreeMap<String, Measured>,
    materials: &BTreeMap<String, cdda_content::materials::Material>,
) -> ItemFacts {
    let recorded = measures.get(&item.id);
    ItemFacts {
        volume_ml: recorded.map_or(0, |recorded| recorded.volume_ml),
        weight_grams: recorded.map_or(0, |recorded| recorded.weight_grams),
        phase: match item.phase() {
            "liquid" => Phase::Liquid,
            "gas" => Phase::Gas,
            _ => Phase::Solid,
        },
        id: item.id.clone(),
        flags: item.flags.clone(),
        base_material: base_material(item),
        ammo_types: item.ammo_types.clone(),
        is_ammo: item.subtypes.contains("AMMO"),
        charges: recorded.map_or(1, |recorded| recorded.charges),
        soft: item.is_soft(materials),
        length_mm: recorded.map_or(0, |recorded| recorded.length_mm),
    }
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
            case.verdicts.len(),
            "{} pockets were asked",
            case.container
        );
        let flags: BTreeSet<String> = payload.flags.clone();
        let material = base_material(payload);
        for (index, pocket) in pockets.iter().enumerate() {
            let port_refuses = !accepts(pocket, &payload.id, &flags, &material);
            let reference_refuses = matches!(case.verdicts.get(index), Some(why) if why == REFUSED);
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

#[test]
fn every_pocket_says_what_the_reference_says_about_a_real_payload() {
    let cases = dump();
    let types: BTreeMap<String, ItemType> = items_loaded()
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let measures = measures();
    let materials = materials();

    let mut differing: Vec<String> = Vec::new();
    let mut messages: BTreeMap<String, usize> = BTreeMap::new();
    for case in &cases {
        let container = types
            .get(&case.container)
            .unwrap_or_else(|| panic!("{} is a type", case.container));
        let payload = types
            .get(&case.payload)
            .unwrap_or_else(|| panic!("{} is a type", case.payload));
        let facts = facts_of(payload, &measures, &materials);
        let pockets: Vec<Pocket> = container
            .pockets
            .iter()
            .filter(|pocket| pocket.kind == "CONTAINER")
            .map(Pocket::from_spec)
            .collect();
        for (index, pocket) in pockets.iter().enumerate() {
            let port = pocket_check(pocket, &facts);
            let reference = case.verdicts.get(index).map_or("", String::as_str);
            *messages.entry(port.clone()).or_default() += 1;
            if port != reference {
                differing.push(format!(
                    "{} into {} (pocket {index}): the port says {port:?}, the reference says {reference:?}",
                    case.payload, case.container
                ));
            }
        }
    }
    assert!(
        messages.len() > 5,
        "the gate has to see more than one kind of answer, or it is only testing one rule: {messages:?}"
    );
    assert!(
        differing.is_empty(),
        "{} verdicts disagree; the first twenty are {:#?}",
        differing.len(),
        differing.get(..20).unwrap_or(&differing)
    );
}
