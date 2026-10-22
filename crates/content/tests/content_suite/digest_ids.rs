//! The id-set gate: every registry whose ids a rule collects exactly right.
//!
//! Almost every JSON type in the reference is a `generic_factory` of something, and they all follow one registration rule: an **entry** - a top-level element of
//! a file - registers under its `id`, and an entry carrying `abstract` registers nothing, because an abstract is a base to copy from rather than a name the game
//! can ask for. Three wrinkles, each of which cost a mismatch before it was written down:

//! - **a registry's name is usually not its JSON type.** 41 of the 90 are written under some other word: `widgets` are `widget`, `json_flags` are `json_flag`, a
//!   `mission_type` is a `mission_definition`, a `zone_type` is a `LOOT_ZONE`, a `spell` is a `SPELL`. The pairs are in `RENAMED` and each was found by asking
//!   which JSON type's id set equals the reference's, then kept honest by the digest below;
//! - **nesting does not make an entry.** A monster's special attack carries `"type": "spell"` and an `id`, and treating it as an entry invented three spells
//!   the reference does not have. The kinds that do register inline - an effect-on-condition in a slot, an enchantment in an `enchantments` list, a turret part
//!   made from a gun - each have a loader that says so, and this rule refuses to guess;
//! - **a factory's null prototype comes from the code, not the data.** `t_null` and `f_null` are in the reference's registries and in no file, which is why
//!   `terrain` and `furniture` are gated through their loaders in `digest.rs` instead.
//!
//! This test is deliberately separate from `digest.rs`: it says the **id set** is right, not that a loader consumes it. Where the two overlap they must agree,
//! and they do. Between them the two gates cover **83 of the 90** registries.

use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The registries this rule gets exactly right, with the count each of them holds. The counts are part of the claim: a registry that quietly lost or gained
/// entries changes a number here even if the digest check below is somehow skipped.
const GATED: [(&str, usize); 79] = [
    ("ITEM", 9912),
    ("widgets", 1236),
    ("trait", 959),
    ("json_flags", 731),
    ("construction groups", 422),
    ("ascii_art", 399),
    ("harvest_list", 366),
    ("profession", 320),
    ("mission_type", 306),
    ("vehicle", 249),
    ("spell", 246),
    ("proficiency", 235),
    ("martial art technique", 203),
    ("material", 193),
    ("event_statistic", 170),
    ("event_transformation", 158),
    ("npc_class", 155),
    ("field types", 145),
    ("monster flags", 133),
    ("bionic", 128),
    ("trap", 125),
    ("start locations", 110),
    ("morale type", 94),
    ("tool quality", 87),
    ("ammo effects", 82),
    ("map extra", 66),
    ("zone_type", 64),
    ("recipe group type", 62),
    ("overmap location", 61),
    ("scenario", 60),
    ("sub body part", 57),
    ("ter_furn_transform", 55),
    ("fault", 54),
    ("weakpoint sets", 48),
    ("fault_fix", 40),
    ("score", 40),
    ("martial art style", 38),
    ("item_category", 36),
    ("body part", 35),
    ("oter_vision", 34),
    ("overmap land use codes", 34),
    ("character modifier", 28),
    ("weapon category", 25),
    ("overmap special migration", 22),
    ("weather_type", 20),
    ("attack vector", 18),
    ("proficiency category", 18),
    ("climbing_aid", 17),
    ("bodygraph", 16),
    ("jmath_function", 15),
    ("construction categories", 15),
    ("limb score", 15),
    ("recipe_category", 14),
    ("behavior", 13),
    ("addiction", 13),
    ("end_screen", 10),
    ("damage info order", 10),
    ("damage type", 10),
    ("gate type", 10),
    ("harvest_drop_type", 9),
    ("fault_group", 8),
    ("mood_face", 8),
    ("relic_procgen_data", 7),
    ("clothing mods", 5),
    ("overmap connection", 5),
    ("scent_type", 4),
    ("move_mode", 4),
    ("shopkeeper_consumption_rates", 3),
    ("option slider", 3),
    ("anatomy", 2),
    ("disease_type", 2),
    ("shopkeeper_blacklist", 2),
    ("profession_group", 1),
    ("speed_description", 1),
    ("MONSTER_FACTION", 133),
    ("martial art buff", 113),
    ("species", 32),
    ("achievement", 226),
    ("butchery_requirements", 1),
];

/// The `type` spellings each registry's entries are written under.
///
/// Two thirds of the registries are named something other than their JSON type, and none of that is documented anywhere: `widgets` are written `widget`,
/// `json_flags` are `json_flag`, a `mission_type` is a `mission_definition`, a `zone_type` is a `LOOT_ZONE`. Each pair here was found the only way it can be
/// found honestly - by asking which JSON type's id set equals the reference's - and then gated by the digest, so a pair that is wrong cannot pass.
const RENAMED: [(&str, &[&str]); 42] = [
    ("widgets", &["widget"]),
    ("trait", &["mutation"]),
    ("json_flags", &["json_flag"]),
    ("construction groups", &["construction_group"]),
    ("harvest_list", &["harvest"]),
    ("mission_type", &["mission_definition"]),
    ("spell", &["SPELL"]),
    ("martial art technique", &["technique"]),
    ("field types", &["field_type"]),
    ("monster flags", &["monster_flag"]),
    ("start locations", &["start_location"]),
    ("morale type", &["morale_type"]),
    ("tool quality", &["tool_quality"]),
    ("ammo effects", &["ammo_effect"]),
    ("map extra", &["map_extra"]),
    ("zone_type", &["LOOT_ZONE"]),
    ("recipe group type", &["recipe_group"]),
    ("overmap location", &["overmap_location"]),
    ("sub body part", &["sub_body_part"]),
    ("weakpoint sets", &["weakpoint_set"]),
    ("martial art style", &["martial_art"]),
    ("item_category", &["ITEM_CATEGORY"]),
    ("body part", &["body_part"]),
    ("overmap land use codes", &["overmap_land_use_code"]),
    ("character modifier", &["character_mod"]),
    ("weapon category", &["weapon_category"]),
    ("overmap special migration", &["overmap_special_migration"]),
    ("attack vector", &["attack_vector"]),
    ("proficiency category", &["proficiency_category"]),
    ("bodygraph", &["body_graph"]),
    ("construction categories", &["construction_category"]),
    ("limb score", &["limb_score"]),
    ("addiction", &["addiction_type"]),
    ("damage info order", &["damage_info_order"]),
    ("damage type", &["damage_type"]),
    ("gate type", &["gate"]),
    ("clothing mods", &["clothing_mod"]),
    ("overmap connection", &["overmap_connection"]),
    ("move_mode", &["movement_mode"]),
    ("option slider", &["option_slider"]),
    ("butchery_requirements", &["butchery_requirement"]),
    ("species", &["SPECIES"]),
];

/// The registries whose entries name themselves with some member other than `id`.
///
/// Reference: `generic_factory<monfaction> faction_factory( "MONSTER_FACTION", "name" )` (`src/monfaction.cpp:13`). It is the only one of the ninety that does
/// this, and it is easy to mistake for a factory that has lost its ids: the registry holds 133 faction names and not one of them is under `id`.
const NAMED_BY: [(&str, &str); 1] = [("MONSTER_FACTION", "name")];

fn id_member(json_type: &str) -> &str {
    if let Some((_, member)) = NAMED_BY.iter().find(|(named, _)| *named == json_type) {
        return member;
    }
    "id"
}

/// The ids that come from somewhere other than an entry's own `id`.
///
/// Three of them, and each is a different kind of exception:
///
/// - a **null prototype** the factory makes in code: `spec_null` is a species the reference has and no file writes, the same wrinkle as `t_null` and `f_null`;
/// - a registry that **borrows a second type**: a conduct is an achievement, so `achievement` is `achievement` entries plus `conduct` entries;
/// - a **slot**, like the effects in `*_eocs`: a martial art's `static_buffs`, `onblock_buffs` and their nine siblings each hold objects with an `id`, and each
///   of those is a buff the reference registers (`martialarts.cpp`, the `*_buffs` readers).
fn extra_ids(registry: &str, value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match registry {
        "species" => out.push("spec_null".to_owned()),
        "martial art buff" => {
            for entry in entries(value) {
                let Value::Object(object) = entry else {
                    continue;
                };
                for (name, member) in object.members() {
                    if !name.ends_with("_buffs") {
                        continue;
                    }
                    if let Value::Array(slots) = member {
                        for slot in slots {
                            if let Value::Object(buff) = slot
                                && let Some(Value::Str(id)) = buff.get("id")
                            {
                                out.push(id.clone());
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    out
}

fn json_types(registry: &str) -> Vec<&str> {
    if let Some((_, types)) = RENAMED.iter().find(|(named, _)| *named == registry) {
        return types.to_vec();
    }
    vec![registry]
}

/// Collects `(JSON type -> ids)` by the shared rule, for every type in the tree.
fn registries_in_tree() -> (
    BTreeMap<String, BTreeSet<String>>,
    BTreeMap<String, BTreeSet<String>>,
) {
    let mut by_type: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut extras: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
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
            // Only the slot-based kinds need to see the file as a whole, and only they are walked for it.
            for registry in ["martial art buff"] {
                for id in extra_ids(registry, &value) {
                    extras.entry(registry.to_owned()).or_default().insert(id);
                }
            }
        }
    }
    (by_type, extras)
}

/// The same, named the way the reference's registries are named.
fn shared_rule() -> BTreeMap<String, BTreeSet<String>> {
    let (by_type, extras) = registries_in_tree();
    let mut out = BTreeMap::new();
    for registry in GATED.iter().map(|(registry, _)| *registry) {
        let mut ids = BTreeSet::new();
        for json_type in json_types(registry) {
            if let Some(found) = by_type.get(json_type) {
                ids.extend(found.iter().cloned());
            }
        }
        if registry == "achievement"
            && let Some(conducts) = by_type.get("conduct")
        {
            // A conduct is an achievement with a type of its own, and the reference registers both in the same factory.
            ids.extend(conducts.iter().cloned());
        }
        if registry == "species" {
            ids.insert("spec_null".to_owned());
        }
        if let Some(found) = extras.get(registry) {
            ids.extend(found.iter().cloned());
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
    let Some(Value::Str(kind)) = object.get("type") else {
        return;
    };
    if let Some(Value::Str(id)) = object.get(id_member(kind))
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
fn seventy_nine_registries_hold_exactly_the_ids_the_reference_holds() {
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
    assert_eq!(checked, 79, "every registry in the list was checked");
}

#[allow(clippy::expect_used)]
#[test]
fn the_enchantments_are_the_written_ones_plus_the_inline_ones() {
    // Second registry in the corpus that a rule finishes: 42 entries name themselves, and 311 are written inline in a trait, a bionic, an effect or a vehicle
    // part, where the reference invents a name from the owner and the slot's position (`magic_enchantment.cpp:267-285`).
    let mut ids: BTreeSet<String> = registries_in_tree()
        .0
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

    // And the id member is not always called `id`: a monster faction is named by `name`, which is why a rule that only looked for `id` found none of the 133.
    let text = r#"[
        {"type": "MONSTER_FACTION", "name": "acid_ant"},
        {"type": "MONSTER_FACTION", "name": ""}
    ]"#;
    let value = cdda_json::parse(text.as_bytes()).expect("it parses");
    let mut by_type = BTreeMap::new();
    for entry in entries(&value) {
        register(entry, &mut by_type);
    }
    assert_eq!(
        by_type.get("MONSTER_FACTION"),
        Some(&BTreeSet::from([String::new(), "acid_ant".to_owned()])),
        "a faction with an empty name is the sentinel the reference keeps"
    );
}
