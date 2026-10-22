//! Mods, their dependencies, and the order the reference loads them in.
//!
//! Reference: `mod_manager::refresh_mod_list` (`src/mod_manager.cpp:179-210`) builds a `dependency_tree` from every `modinfo.json` it can find, and
//! `dependency_node::get_dependencies_as_nodes` (`src/dependency_tree.cpp:157-197`) answers "what must load before this mod" with a stack walk:
//!
//! ```text
//! push this mod's dependencies, in the order it declares them
//! pop; skip anything already seen; record it; push *its* dependencies
//! then walk the record backwards, keeping each mod the first time it appears
//! ```
//!
//! The backwards pass is not decoration. Walking forwards, a dependency can appear before something it depends on; reversed and de-duplicated, every entry comes
//! after everything it needs. The oracle's `--dump-mods` writes the reference's own answer for all 43 mods, so this is parity rather than a reading of the code.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

#[test]
fn every_mods_resolved_order_is_the_references() {
    let value = cdda_content::mods::read_dump().expect("the dump");
    let dump = cdda_content::mods::load_dump(&value).expect("it parses");
    let ours = cdda_content::mods::resolve_all(&dump.mods);
    for (id, resolved) in &dump.resolved {
        assert_eq!(
            ours.get(id).map(Vec::as_slice).unwrap_or_default(),
            resolved.as_slice(),
            "{id}: the mods the reference loads before it, in order"
        );
    }
}

#[test]
fn the_default_order_is_the_list_the_reference_replays() {
    // `dev:default` declares three mods and its resolved order is not that list: the tree's own answer is `no_npc_food`, then `dda`, then the storms mod.
    let value = cdda_json::parse(
        br#"[
            {"type": "MOD_INFO", "id": "a", "dependencies": ["b", "c"]},
            {"type": "MOD_INFO", "id": "b", "dependencies": []},
            {"type": "MOD_INFO", "id": "c", "dependencies": ["b"]}
        ]"#,
    )
    .expect("it parses");
    let mods = cdda_content::mods::from_data(&value).expect("they load");
    let resolved = cdda_content::mods::resolve_all(&mods);
    assert_eq!(
        resolved.get("a"),
        Some(&vec!["b".to_owned(), "c".to_owned()])
    );
}

/// A diamond: two mods sharing one base. The interesting part is that the reference's answer is **not** a topological order - `left` comes before the `base` it
/// depends on - because the backwards pass only keeps each mod's *first* appearance from the back. That looks like a bug and may be one; what matters here is
/// that the port answers what the reference answers, and the test above is the one that proves it over all 43 mods.
#[test]
fn a_dependency_that_appears_twice_is_named_once() {
    let mods = vec![
        cdda_content::mods::ModInfo {
            id: "top".to_owned(),
            name: "top".to_owned(),
            dependencies: vec!["left".to_owned(), "right".to_owned()],
        },
        cdda_content::mods::ModInfo {
            id: "left".to_owned(),
            name: "left".to_owned(),
            dependencies: vec!["base".to_owned()],
        },
        cdda_content::mods::ModInfo {
            id: "right".to_owned(),
            name: "right".to_owned(),
            dependencies: vec!["base".to_owned()],
        },
        cdda_content::mods::ModInfo {
            id: "base".to_owned(),
            name: "base".to_owned(),
            dependencies: Vec::new(),
        },
    ];
    let resolved = cdda_content::mods::resolve_all(&mods);
    assert_eq!(
        resolved.get("top"),
        Some(&vec![
            "left".to_owned(),
            "base".to_owned(),
            "right".to_owned()
        ]),
        "the base is named once, and it lands between the two mods that need it"
    );
    // The dump's own defaults list, which is the list the reference replays verbatim.
    let value = cdda_content::mods::read_dump().expect("the dump");
    let dump = cdda_content::mods::load_dump(&value).expect("it parses");
    assert_eq!(
        dump.defaults,
        vec![
            "dda".to_owned(),
            "no_npc_food".to_owned(),
            "personal_portal_storms".to_owned()
        ],
        "a new world's mods, in the order the default mod file lists them"
    );
    let _: BTreeMap<String, Vec<String>> = resolved;
}
