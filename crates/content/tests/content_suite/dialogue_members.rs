//! The port reads no dialogue member the reference does not define.
//!
//! Reference: the effect table in `npctalk.cpp` and the condition table in `condition.cpp`, which map each data member to the rule that carries it out.
//! A reader that spells a member differently from the table reads **nothing**, refuses every entry, and looks exactly like a rule the port has not
//! written yet - which is how `u_lose_effect` came to be blamed on the effect for months while the reader asked for `u_remove_effect`.

use std::collections::BTreeSet;

/// The reference's sources: the oracle's patched copy when it is here, the pinned checkout when it is not.
///
/// The harness patches forty-five files and neither of the two read here, so the tables are the reference's own either way - and a runner that fetched only the
/// pinned checkout can still prove the rule instead of skipping it.
fn reference_sources() -> std::path::PathBuf {
    let oracle = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/src");
    if oracle.is_dir() {
        oracle
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/src")
    }
}

/// The member names the reference's tables define.
fn reference_members() -> BTreeSet<String> {
    let root = reference_sources();
    let mut names = BTreeSet::new();
    for file in ["npctalk.cpp", "condition.cpp"] {
        let Ok(text) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        for line in text.lines() {
            // A table entry names its members first: `{ "u_lose_effect", "npc_lose_effect", jarg::member, … }`.
            let Some(body) = line
                .split_once('{')
                .and_then(|(_, rest)| rest.split_once('}'))
            else {
                continue;
            };
            // Either table's shape: the effect table names a `jarg::` kind, the condition table names a rule function - and both start with the
            // member's two spellings, which is the part this test is about.
            if !body.0.contains("jarg::") && !body.0.contains('&') {
                continue;
            }
            for (index, part) in body.0.split(',').enumerate() {
                if index > 1 {
                    break;
                }
                let name = part.trim().trim_matches('"');
                if !name.is_empty()
                    && name.chars().all(|character| {
                        character.is_ascii_lowercase()
                            || character == '_'
                            || character.is_ascii_digit()
                    })
                {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    names
}

/// The member names the port's readers ask for.
fn port_members() -> BTreeSet<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut names = BTreeSet::new();
    for file in ["dialogue.rs", "conditions.rs"] {
        let Ok(text) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        for line in text.lines() {
            // A member is a key looked up in the data - `object.get("u_lose_effect")` - or one half of a table's two spellings,
            // `("u_lose_effect", false, false)`. A **math function** is neither: it is parsed out of a text expression, so counting every
            // `u_`-prefixed string in the file would make this audit a test about prose.
            for (at, piece) in line.split('"').enumerate() {
                if at % 2 == 0 || !looks_like_a_member(piece) {
                    continue;
                }
                let lookup = line.contains(&format!(".get(\"{piece}\")"));
                let pair = line.contains(&format!("(\"{piece}\","));
                if lookup || pair {
                    names.insert(piece.to_owned());
                }
            }
        }
    }
    names
}

/// Whether a string looks like a member name rather than prose: lowercase words joined by underscores.
fn looks_like_a_member(text: &str) -> bool {
    // Only the **dialogue members**: a name the data writes to reach a rule. An argument inside one (`target_var`, `min_radius`) is not a member, and
    // comparing those would make the test about structures rather than about names.
    (text.starts_with("u_") || text.starts_with("npc_"))
        && text.len() > 3
        && text.contains('_')
        && !text.contains(' ')
        && text.chars().all(|character| {
            character.is_ascii_lowercase() || character == '_' || character.is_ascii_digit()
        })
}

/// Names the port reads that the reference's tables do not define.
///
/// Each entry says why: an **alias** the reference accepts elsewhere, or a **member of a structure** the tables do not list (a variable's scope, a
/// target, a body part).
const EXPLAINED: &[(&str, &str)] = &[
    (
        "u_remove_effect",
        "the spelling the port used to read; kept as an alias after the data proved it wrong",
    ),
    ("npc_remove_effect", "the same alias for the listener"),
    (
        "u_val",
        "a variable's scope inside `{\"u_val\": \"x\"}`, defined by the variable reader rather than the effect table",
    ),
    ("npc_val", "the listener's variable scope"),
    (
        "unique_ids",
        "a member *inside* `u_run_npc_eocs`, which narrows the people it runs on, not an effect of its own",
    ),
    (
        "npc_range",
        "a member inside `u_run_npc_eocs`: how far from the actor a person may be",
    ),
    (
        "z_min",
        "a member inside `u_run_npc_eocs`, refused by the port until it can filter by level",
    ),
    ("z_max", "the same, for the top of the level filter"),
    (
        "npc_must_see",
        "a member inside `u_run_npc_eocs`, refused until the port has a sight test per person",
    ),
    // The four **name prefixes** the math reader splits out of a bare variable name (`u_x`, `n_x`, `npc_x`, `global_x`). They are not members: they are how the
    // reference's `read_var_value` turns a name into a scope and a name, and the scanner sees them because it looks for `u_`/`npc_`-style strings.
    ("npc_", "a prefix inside a bare math name, not a member"),
    ("n_", "the same, shorter spelling"),
    ("global_", "the same, for the globals"),
    ("var_", "the same, for the dialogue's own scope"),
];

#[test]
fn every_dialogue_member_the_port_reads_is_one_the_reference_defines() {
    let reference = reference_members();
    assert!(
        reference.len() > 100,
        "the reference's tables were read: {} names, which is far too few to be the whole tables",
        reference.len()
    );
    let port = port_members();
    assert!(
        port.len() > 20,
        "the port's readers were read: {} names",
        port.len()
    );

    let explained: BTreeSet<&str> = EXPLAINED.iter().map(|(name, _)| *name).collect();
    let unknown: Vec<&String> = port
        .iter()
        .filter(|name| !reference.contains(*name) && !explained.contains(name.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "the port reads members the reference does not define, so those entries refuse and look like unwritten rules: {unknown:?}"
    );
}
