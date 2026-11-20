//! Traps against the reference's own resolve.
//!
//! The oracle is `corpus/prototypes/traps.json`. Traps are the smallest loader
//! that still has a mandatory field set, a one-character symbol rule, and
//! inheritance, so a mistake here is a mistake the item loader would repeat.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::traps;
use cdda_json::{Value, parse};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn a_trap_takes_its_glyph_and_its_numbers() {
    let dir = std::env::temp_dir().join("cdda-trap-fixture");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_test", "name": "a test trap", "color": "red",
          "symbol": "^", "visibility": 3, "avoidance": 2, "difficulty": 1,
          "trap_radius": 1, "benign": true, "flags": [ "UNDODGEABLE" ] }
        ]"#,
    )
    .expect("fixture");
    let loaded = traps::load_paths(&[&dir]).expect("loads");
    let trap = loaded
        .iter()
        .find(|entry| entry.id == "tr_test")
        .expect("prototype");
    assert_eq!(trap.name, "a test trap");
    assert_eq!(trap.symbol, i32::from(b'^'));
    assert_eq!(trap.color_name(), "c_red");
    assert_eq!(trap.visibility, 3);
    assert_eq!(trap.avoidance, 2);
    assert_eq!(trap.difficulty, 1);
    assert_eq!(trap.radius, 1);
    assert!(trap.benign);
    assert!(trap.flags.iter().any(|flag| flag == "UNDODGEABLE"));
}

#[test]
fn a_symbol_that_is_not_one_character_is_rejected() {
    let dir = std::env::temp_dir().join("cdda-trap-bad-symbol");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_bad", "name": "x", "color": "red", "symbol": "ab",
          "visibility": 1, "avoidance": 0, "difficulty": 0 }
        ]"#,
    )
    .expect("fixture");
    let error = traps::load_paths(&[&dir]).expect_err("two characters are not a glyph");
    assert!(error.to_string().contains("one character"), "{error}");
}

#[test]
fn every_trap_matches_the_reference() {
    let text =
        std::fs::read_to_string(root().join("corpus/prototypes/traps.json")).expect("artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let Value::Object(root_object) = value else {
        panic!("root")
    };
    let Some(Value::Array(entries)) = root_object.get("prototypes") else {
        panic!("prototypes")
    };
    let loaded = traps::load_paths(&[
        &root().join("references/Cataclysm-DDA/data/core"),
        &root().join("references/Cataclysm-DDA/data/json"),
    ])
    .expect("loads");
    println!(
        "loaded {} traps, reference has {}",
        loaded.len(),
        entries.len()
    );
    let mut mismatches = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        let text_of = |member: &str| match object.get(member) {
            Some(Value::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let number_of = |member: &str| match object.get(member) {
            Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(i32::MAX),
            _ => 0,
        };
        let flag_of = |member: &str| match object.get(member) {
            Some(Value::Bool(value)) => *value,
            _ => false,
        };
        let id = text_of("id");
        let Some(ours) = loaded.iter().find(|trap| trap.id == id) else {
            mismatches.push(format!("{id}: missing"));
            continue;
        };
        for (field, ours_value, theirs) in [
            ("name", ours.name.clone(), text_of("name")),
            ("color", ours.color_name().to_owned(), text_of("color")),
        ] {
            if ours_value != theirs {
                mismatches.push(format!("{id}: {field} {ours_value:?} != {theirs:?}"));
            }
        }
        for (field, ours_value, theirs) in [
            ("symbol", ours.symbol, number_of("symbol")),
            ("visibility", ours.visibility, number_of("visibility")),
            ("avoidance", ours.avoidance, number_of("avoidance")),
            ("difficulty", ours.difficulty, number_of("difficulty")),
            ("radius", ours.radius, number_of("radius")),
        ] {
            if ours_value != theirs {
                mismatches.push(format!("{id}: {field} {ours_value} != {theirs}"));
            }
        }
        for (field, ours_value, theirs) in [
            ("benign", ours.benign, flag_of("benign")),
            (
                "always_invisible",
                ours.always_invisible,
                flag_of("always_invisible"),
            ),
        ] {
            if ours_value != theirs {
                mismatches.push(format!("{id}: {field} {ours_value} != {theirs}"));
            }
        }
        // The reference keeps flags in a sorted set whose order is the flag's
        // registration order, which is not something a port should reproduce;
        // the set is what has to match.
        let ours_flags: Vec<String> = ours.flags.iter().cloned().collect();
        let theirs_flags: Vec<String> = match object.get("flags") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| match item {
                    Value::Str(text) => Some(text.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let mut theirs_flags = theirs_flags;
        theirs_flags.sort();
        if ours_flags != theirs_flags {
            mismatches.push(format!("{id}: flags {ours_flags:?} != {theirs_flags:?}"));
        }
    }
    println!("{} mismatches", mismatches.len());
    for line in mismatches.iter().take(15) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

/// **What a trap does is part of the trap.** Reference: `trap::load` reads `action` (`trap.cpp:150-175`), `spell_data` only beside `action: "spell"`, `eocs` only beside
/// `action: "eocs"`, and `map_regen` (`trap.cpp:154`) for the action that rebuilds its tile. The port reads the action, the programs, the two trigger messages and the
/// mapgen, and refuses a `eocs` list that names no `eocs` action - the reference's own refusal.
#[test]
fn a_trap_carries_what_it_does() {
    let dir = std::env::temp_dir().join("cdda-trap-actions");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_regen", "name": "a regen trap", "color": "red",
          "symbol": "^", "visibility": 0, "avoidance": 0, "difficulty": 0,
          "action": "map_regen", "map_regen": "some_update" },
        { "type": "trap", "id": "tr_eoc", "name": "an eoc trap", "color": "red",
          "symbol": "^", "visibility": 0, "avoidance": 0, "difficulty": 0,
          "action": "eocs", "eocs": [ "EOC_ONE", "EOC_TWO" ],
          "trigger_message_u": "You stepped on it!", "trigger_message_npc": "<npcname> stepped on it!" }
    ]"#,
    )
    .expect("write");

    let loaded = traps::load_paths(&[dir.as_path()]).expect("the traps load");
    let regen = loaded
        .iter()
        .find(|one| one.id == "tr_regen")
        .expect("the regen trap");
    assert_eq!(regen.action.as_deref(), Some("map_regen"));
    assert_eq!(regen.map_regen.as_deref(), Some("some_update"));
    assert!(regen.eocs.is_empty(), "and it runs no programs");

    let eoc = loaded
        .iter()
        .find(|one| one.id == "tr_eoc")
        .expect("the eoc trap");
    assert_eq!(eoc.action.as_deref(), Some("eocs"));
    assert_eq!(eoc.eocs, vec!["EOC_ONE".to_owned(), "EOC_TWO".to_owned()]);
    assert_eq!(eoc.trigger_u.as_deref(), Some("You stepped on it!"));
    assert_eq!(eoc.trigger_npc.as_deref(), Some("<npcname> stepped on it!"));
    assert!(eoc.map_regen.is_none(), "and it rebuilds nothing");
}

/// **An `eocs` list without `action: "eocs"` is refused**, which is what the reference throws for (`trap.cpp:168-171`).
#[test]
fn an_eocs_list_without_the_action_is_refused() {
    let dir = std::env::temp_dir().join("cdda-trap-bad-eocs");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_bad", "name": "a bad trap", "color": "red",
          "symbol": "^", "visibility": 0, "avoidance": 0, "difficulty": 0,
          "eocs": [ "EOC_ONE" ] }
    ]"#,
    )
    .expect("write");
    let refused = traps::load_paths(&[dir.as_path()]);
    assert!(
        refused.is_err(),
        "**the loader refuses it**, as the reference does"
    );
}

/// **A `spell` trap carries its spell**, the action thirty-four traps in the data name - the most common of all. Reference: `trap.h:165` holds a `fake_spell`
/// (`spell_data`), and `trap.cpp:160-167` refuses that member beside any action but `"spell"`.
#[test]
fn a_spell_trap_carries_the_spell_it_casts() {
    let dir = std::env::temp_dir().join("cdda-trap-spell");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_spell", "name": "a spell trap", "color": "red",
          "symbol": "^", "visibility": 0, "avoidance": 0, "difficulty": 0,
          "action": "spell", "spell_data": { "id": "summon_trapspider", "level": 3, "max_level": 5 } },
        { "type": "trap", "id": "tr_plain_spell", "name": "a plain spell trap", "color": "red",
          "symbol": "^", "visibility": 0, "avoidance": 0, "difficulty": 0,
          "action": "spell", "spell_data": { "id": "summon_trapspider" } }
    ]"#,
    )
    .expect("write");
    let loaded = traps::load_paths(&[dir.as_path()]).expect("the traps load");
    let spelled = loaded
        .iter()
        .find(|one| one.id == "tr_spell")
        .expect("the spell trap");
    let spell = spelled.spell.as_ref().expect("its spell");
    assert_eq!(spell.id, "summon_trapspider");
    assert_eq!(spell.level, 3, "the level the data names");
    assert_eq!(spell.max_level, Some(5), "and its ceiling");
    let plain = loaded
        .iter()
        .find(|one| one.id == "tr_plain_spell")
        .expect("the plain spell trap");
    let spell = plain.spell.as_ref().expect("its spell");
    assert_eq!(spell.level, 0, "**a spell with no level casts at zero**");
    assert_eq!(spell.max_level, None, "and has no ceiling of its own");
}

/// **`spell_data` without `action: "spell"` is refused**, which is the reference's own throw (`trap.cpp:160-165`).
#[test]
fn spell_data_without_the_action_is_refused() {
    let dir = std::env::temp_dir().join("cdda-trap-bad-spell");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("traps.json"),
        r#"[
        { "type": "trap", "id": "tr_bad_spell", "name": "a bad spell trap", "color": "red",
          "symbol": "^", "visibility": 0, "avoidance": 0, "difficulty": 0,
          "action": "eocs", "eocs": [ "EOC_ONE" ], "spell_data": { "id": "summon_trapspider" } }
    ]"#,
    )
    .expect("write");
    assert!(
        traps::load_paths(&[dir.as_path()]).is_err(),
        "**the loader refuses it**, as the reference does"
    );
}

/// **What each trap asks for, ranked.** The census that keeps the trap subsystem honest the way the dialogue census keeps the EOC runtime honest: every trap in the data,
/// bucketed by the action it names, with what the port answers and what it does not.
///
/// The point is the **shape of what is left**, not today's numbers: the buckets say whether the next round should port a rule (a named function with no meaning in the port)
/// or is looking at chain links (an action that names another program).
#[test]
fn the_reference_tree_holds_the_trap_actions_the_port_answers() {
    let loaded = traps::load_paths(&[
        &root().join("references/Cataclysm-DDA/data/core"),
        &root().join("references/Cataclysm-DDA/data/json"),
    ])
    .expect("the traps load");
    let mut counts = std::collections::BTreeMap::new();
    for trap in &loaded {
        let action = trap
            .action
            .clone()
            .unwrap_or_else(|| "(no action)".to_owned());
        *counts.entry(action).or_insert(0usize) += 1;
    }
    if std::env::var("CDDA_SHOW_TRAPS").is_ok() {
        let mut ranked: Vec<_> = counts.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (action, count) in ranked {
            println!("TRAP {count:4} {action}");
        }
    }
    // The port answers these: `eocs` runs its programs, `map_regen` rebuilds the tile, `spell` casts, and `none` is the reference's own do-nothing.
    for answered in ["eocs", "map_regen", "spell", "none"] {
        assert!(
            counts.contains_key(answered),
            "**the data names {answered}**, so a census that does not see it is reading the wrong tree"
        );
    }
    let total: usize = counts.values().sum();
    // 125: the data/core tree holds one as well, which a scan of `data/json` alone does not see.
    assert_eq!(total, 125, "every trap in the reference tree");
    assert_eq!(
        counts.get("spell").copied(),
        Some(34),
        "the most common action"
    );
    // **Twenty-four, not six.** The raw data writes `action: "eocs"` six times; the other eighteen inherit it through `copy-from`, which is only visible after the loader
    // resolves the copies - the same lesson the sizes test taught, now with a count.
    assert_eq!(counts.get("eocs").copied(), Some(24));
    assert_eq!(counts.get("map_regen").copied(), Some(17));
    assert_eq!(
        counts.get("none").copied(),
        Some(16),
        "the reference's own do-nothing"
    );
    // A hundred and fifteen of a hundred and twenty-five traps therefore have an answer; the remaining ten name ten named functions. Pinned so the frontier cannot
    // drift without the number moving with it.
    let answered: usize = [
        "eocs",
        "map_regen",
        "spell",
        "none",
        "glass",
        "shotgun",
        "beartrap",
        "landmine",
        "caltrops_glass",
        "cot",
        "hum",
        "bubble",
        "drain",
        "caltrops",
        "board",
        "pit",
        "snare_light",
        "tripwire",
        "pit_spikes",
        "pit_glass",
        "blade",
        "lava",
    ]
    .iter()
    .map(|action| counts.get(*action).copied().unwrap_or(0))
    .sum();
    assert_eq!(answered, 115, "traps whose action the port answers");
    assert_eq!(
        total - answered,
        10,
        "**and ten that name a function the port has no rule for**"
    );
    // **Every unported action is a single trap**: what is left is one trap per function, so the frontier is a list of singletons and each round can take several.
    assert_eq!(
        counts.get("caltrops").copied(),
        Some(1),
        "the largest of the actions still unported is a single trap"
    );
}
