//! The id-set gate: every registry whose ids the shared rule collects exactly right.
//!
//! Almost every JSON type in the reference is a `generic_factory` of something, and all of them follow one registration rule: an entry registers under its
//! `id`, and an entry carrying an `abstract` key registers nothing at all - an abstract is a base other entries copy from, not a thing the game can name.
//! Two wrinkles:

//! - one registry's JSON type is not its registry name: `mutation` registers as `trait` (959 of them, and the reference's own dump calls the registry
//!   `trait`, which is why a rule that trusted the type name missed every one);
//! - a factory's **null prototype** is created by the code, not by the data, so `t_null` and `f_null` are the two ids the reference's registries hold and no
//!   file writes. That is why `terrain` and `furniture` are gated through their loaders in `digest.rs` rather than through this rule.
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
    ("spell", 246),
];

/// The `type` spellings a registry's entries are written under. Two registries need this: a trait is a `mutation`, and a spell is a `SPELL` in modern data and a
/// `spell` in the few files that predate the uppercase convention.
fn json_types(registry: &str) -> Vec<&str> {
    match registry {
        "trait" => vec!["mutation"],
        "spell" => vec!["SPELL", "spell"],
        other => vec![other],
    }
}

/// Collects `(JSON type -> ids)` by the shared rule, for every type in the tree.
fn registries_in_tree() -> BTreeMap<String, BTreeSet<String>> {
    let mut by_type: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for root in crate::corpus::dirs() {
        let Ok(files) = cdda_content::json_files_in_load_order(&root) else {
            continue;
        };
        for path in files {
            let Some(value) = cdda_content::read_json(&path) else {
                continue;
            };
            for entry in entries(&value) {
                register(entry, &mut by_type);
            }
        }
    }
    by_type
}

/// The same, named the way the reference's registries are named.
fn shared_rule() -> BTreeMap<String, BTreeSet<String>> {
    let by_type = registries_in_tree();
    let mut out = BTreeMap::new();
    for registry in GATED.iter().map(|(registry, _)| *registry) {
        let mut ids = BTreeSet::new();
        for json_type in json_types(registry) {
            if let Some(found) = by_type.get(json_type) {
                ids.extend(found.iter().cloned());
            }
        }
        if !ids.is_empty() {
            out.insert(registry.to_owned(), ids);
        }
    }
    out
}

/// The entries of one file: a data file is a list of entries, and an object on its own is one entry.
fn entries(value: &Value) -> &[Value] {
    match value {
        Value::Array(items) => items,
        single => std::slice::from_ref(single),
    }
}

/// Registers one entry under its `id`, unless it is an abstract.
///
/// Nested objects are deliberately **not** entries: a monster's special attack carries `"type": "spell"` and an `id` of its own, and treating it as an entry
/// invented three spells the reference does not have (`star_vampire_invisibility` and two of its siblings). The kinds that do register inline - an
/// effect-on-condition in a slot, an enchantment in an `enchantments` list, a turret part from a gun - each have a loader that says so, and this rule refuses to
/// guess.
fn register(entry: &Value, out: &mut BTreeMap<String, BTreeSet<String>>) {
    let Value::Object(object) = entry else {
        return;
    };
    if let (Some(Value::Str(kind)), Some(Value::Str(id))) = (object.get("type"), object.get("id"))
        && object.get("abstract").is_none()
    {
        out.entry(kind.clone()).or_default().insert(id.clone());
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
fn the_enchantments_are_the_written_ones_plus_the_inline_ones() {
    // Second registry in the corpus that a rule finishes: 42 entries name themselves, and 311 are written inline in a trait, a bionic, an effect or a vehicle
    // part, where the reference invents a name from the owner and the slot's position (`magic_enchantment.cpp:267-285`).
    let mut ids: BTreeSet<String> = registries_in_tree()
        .remove("enchantment")
        .expect("the tree defines enchantments by name");
    let written = ids.len();
    ids.extend(crate::corpus::inline_enchantments_loaded());
    assert_eq!(written, 42, "the ones a file names");
    assert_eq!(ids.len(), 353, "the reference's own enchantment registry");
    assert_eq!(
        cdda_content::digest::digest_hex(cdda_content::digest::digest_ids(
            ids.iter().map(String::as_str)
        )),
        *digests().get("enchantment").expect("enchantment digest"),
        "enchantment ids"
    );
}

#[allow(clippy::expect_used)]
#[test]
fn the_vehicle_parts_are_the_ones_written_plus_the_turrets_the_guns_earn() {
    // The one registry in the ninety that the id-set rule cannot finish on its own: `vehicle_part` holds 462 entries that data files write and 239 that the
    // reference invents from the guns at finalisation (`veh_type.cpp:565-640`). Both halves have to be there for the digest to match, which is what makes this
    // the first gate in the corpus that depends on a rule rather than on a file.
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let vehicles = crate::corpus::vehicles_dump_loaded();
    ids.extend(vehicles.parts.iter().cloned());
    let turrets = cdda_content::vehicles::turret_parts(&crate::corpus::items_loaded());
    let mut invented = 0;
    for part in turrets {
        if ids.insert(part) {
            invented += 1;
        }
    }
    assert_eq!(invented, 0, "the dump already lists the turret parts");
    assert_eq!(ids.len(), 701, "the reference's own part registry");
    assert_eq!(
        cdda_content::digest::digest_hex(cdda_content::digest::digest_ids(
            ids.iter().map(String::as_str)
        )),
        *digests().get("vehicle_part").expect("vehicle part digest"),
        "vehicle part ids"
    );
}

#[allow(clippy::expect_used)]
#[test]
fn an_abstract_registers_nothing_and_a_type_name_can_differ_from_its_registry() {
    // The two wrinkles, on a document small enough to read. `mutation` is the registry the game calls `trait`, and an entry with `abstract` exists only to be
    // copied from.
    let text = r#"[
        {"type": "mutation", "id": "BEAUTIFUL"},
        {"type": "mutation", "abstract": "BASE"},
        {"type": "MONSTER", "id": "zombie", "special_attacks": [
            {"type": "spell", "id": "not_a_spell_entry"}
        ]}
    ]"#;
    let value = cdda_json::parse(text.as_bytes()).expect("it parses");
    let mut by_type = BTreeMap::new();
    for entry in entries(&value) {
        register(entry, &mut by_type);
    }
    assert_eq!(
        by_type.get("mutation"),
        Some(&BTreeSet::from(["BEAUTIFUL".to_owned()])),
        "the abstract is not a trait"
    );
    assert!(
        !by_type.contains_key("spell"),
        "a monster's special attack is not an entry: three spells in the real data are exactly this and the reference has none of them"
    );
}
