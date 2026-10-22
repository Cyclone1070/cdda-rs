//! The id-set gate: every registry whose ids the shared rule collects exactly right.
//!
//! Almost every JSON type in the reference is a `generic_factory` of something, and all of them follow one registration rule: an entry registers under its
//! `id`, and an entry carrying an `abstract` key registers nothing at all - an abstract is a base other entries copy from, not a thing the game can name.
//! Two wrinkles:

//! - one registry's JSON type is not its registry name: `mutation` registers as `trait` (959 of them, and the reference's own dump calls the registry
//!   `trait`, which is why a rule that trusted the type name missed every one);
//! - a factory's **null prototype** is created by the code, not by the data, so `t_null` is the one terrain id that is in the reference's registry and not
//!   in the files.
//!
//! This test is the evidence for that rule over thirty-six registries and about thirteen thousand ids. It is deliberately separate from `digest.rs`, which
//! gates registries the port has an actual **loader** for: this one says the id set is right, not that anything consumes it yet. Where the two overlap they
//! must agree, and they do.

use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The registries this rule gets exactly right, with the count each of them holds. The counts are part of the claim: a registry that quietly lost or gained
/// entries changes a number here even if the digest check below is somehow skipped.
const GATED: [(&str, usize); 36] = [
    ("ITEM", 9912),
    ("trait", 959),
    ("furniture", 660),
    ("ascii_art", 399),
    ("profession", 320),
    ("vehicle", 249),
    ("proficiency", 235),
    ("material", 193),
    ("event_statistic", 170),
    ("event_transformation", 158),
    ("npc_class", 155),
    ("bionic", 128),
    ("trap", 125),
    ("scenario", 60),
    ("ter_furn_transform", 55),
    ("fault", 54),
    ("fault_fix", 40),
    ("score", 40),
    ("oter_vision", 34),
    ("weather_type", 20),
    ("climbing_aid", 17),
    ("jmath_function", 15),
    ("recipe_category", 14),
    ("behavior", 13),
    ("end_screen", 10),
    ("harvest_drop_type", 9),
    ("fault_group", 8),
    ("mood_face", 8),
    ("relic_procgen_data", 7),
    ("scent_type", 4),
    ("shopkeeper_consumption_rates", 3),
    ("anatomy", 2),
    ("disease_type", 2),
    ("shopkeeper_blacklist", 2),
    ("profession_group", 1),
    ("speed_description", 1),
];

/// The JSON `type` of a registry whose name differs from it.
fn json_type(registry: &str) -> &str {
    match registry {
        "trait" => "mutation",
        other => other,
    }
}

/// Collects `(registry -> ids)` by the shared rule.
fn shared_rule() -> BTreeMap<String, BTreeSet<String>> {
    let mut by_type: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for root in crate::corpus::dirs() {
        let Ok(files) = cdda_content::json_files_in_load_order(&root) else {
            continue;
        };
        for path in files {
            let Some(value) = cdda_content::read_json(&path) else {
                continue;
            };
            walk(&value, &mut by_type);
        }
    }
    let mut out = BTreeMap::new();
    for registry in GATED.iter().map(|(registry, _)| *registry) {
        if let Some(ids) = by_type.get(json_type(registry)) {
            out.insert(registry.to_owned(), ids.clone());
        }
    }
    out
}

fn walk(value: &Value, out: &mut BTreeMap<String, BTreeSet<String>>) {
    match value {
        Value::Array(items) => {
            for item in items {
                walk(item, out);
            }
        }
        Value::Object(object) => {
            if let (Some(Value::Str(kind)), Some(Value::Str(id))) =
                (object.get("type"), object.get("id"))
                && object.get("abstract").is_none()
            {
                out.entry(kind.clone()).or_default().insert(id.clone());
            }
            for (name, member) in object.members() {
                if name == "id" || name == "type" {
                    continue;
                }
                walk(member, out);
            }
        }
        _ => {}
    }
}

#[allow(clippy::expect_used, clippy::panic)]
fn digests() -> BTreeMap<String, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/prototypes/digest.json");
    let text = std::fs::read_to_string(&path).expect("the digest artifact");
    let parsed = cdda_json::parse(text.as_bytes()).expect("it parses");
    let Value::Object(root) = parsed else {
        panic!("the digest artifact is an object");
    };
    let Some(Value::Array(registries)) = root.get("registries") else {
        panic!("it holds a registry list");
    };
    let mut out = BTreeMap::new();
    for entry in registries {
        let Value::Object(entry) = entry else {
            continue;
        };
        let (Some(Value::Str(kind)), Some(Value::Str(digest))) =
            (entry.get("type"), entry.get("digest"))
        else {
            continue;
        };
        out.insert(kind.clone(), digest.clone());
    }
    out
}

#[allow(clippy::panic)]
#[test]
fn thirty_six_registries_hold_exactly_the_ids_the_reference_holds() {
    let ours = shared_rule();
    let theirs = digests();
    let mut checked = 0;
    for (registry, count) in GATED {
        let Some(ids) = ours.get(registry) else {
            panic!("{registry}: the shared rule collected nothing");
        };
        assert_eq!(
            ids.len(),
            count,
            "{registry}: the count this gate was written for"
        );
        let reference = theirs
            .get(registry)
            .unwrap_or_else(|| panic!("{registry}: no reference digest"));
        assert_eq!(
            cdda_content::digest::digest_hex(cdda_content::digest::digest_ids(
                ids.iter().map(String::as_str)
            )),
            *reference,
            "{registry}: the id set the reference holds"
        );
        checked += 1;
    }
    assert_eq!(checked, 36, "every registry in the list was checked");
}

#[allow(clippy::expect_used)]
#[test]
fn an_abstract_registers_nothing_and_a_type_name_can_differ_from_its_registry() {
    // The two wrinkles, on a document small enough to read. `mutation` is the registry the game calls `trait`, and an entry with `abstract` exists only to be
    // copied from.
    let text = r#"[
        {"type": "mutation", "id": "BEAUTIFUL"},
        {"type": "mutation", "abstract": "BASE"},
        {"type": "GENERIC", "id": "nested_is_collected_too"}
    ]"#;
    let value = cdda_json::parse(text.as_bytes()).expect("it parses");
    let mut by_type = BTreeMap::new();
    walk(&value, &mut by_type);
    assert_eq!(
        by_type.get("mutation"),
        Some(&BTreeSet::from(["BEAUTIFUL".to_owned()])),
        "the abstract is not a trait"
    );
    assert_eq!(
        by_type.get("GENERIC"),
        Some(&BTreeSet::from(["nested_is_collected_too".to_owned()]))
    );
}
