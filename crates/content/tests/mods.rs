//! Mods: the reference's own modinfo files, and an order that is actually an order.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::mods::{ModInfo, dependency_order, from_data};
use cdda_json::parse;
use std::path::Path;

#[test]
fn a_mod_carries_its_id_name_and_dependencies() {
    let value = parse(
        br#"[{"type":"MOD_INFO","id":"dda","name":"Dark Days Ahead"},
             {"type":"MOD_INFO","id":"aftershock","name":"Aftershock","dependencies":["dda"]}]"#,
    )
    .expect("valid JSON");
    let mods = from_data(&value).expect("the file loads");
    assert_eq!(mods.len(), 2);
    assert_eq!(mods[1].dependencies, vec!["dda".to_string()]);
}

#[test]
fn other_object_types_in_the_file_are_ignored() {
    let value = parse(br#"[{"type":"MONSTER","id":"zombie"},{"type":"MOD_INFO","id":"m"}]"#)
        .expect("valid JSON");
    assert_eq!(
        from_data(&value).expect("loads").len(),
        1,
        "only MOD_INFO counts"
    );
}

#[test]
fn dependencies_come_before_the_mods_that_need_them() {
    let mods = vec![
        ModInfo {
            id: "dda".into(),
            name: "core".into(),
            dependencies: vec![],
        },
        ModInfo {
            id: "aftershock".into(),
            name: "A".into(),
            dependencies: vec!["dda".into()],
        },
        ModInfo {
            id: "exoplanet".into(),
            name: "E".into(),
            dependencies: vec!["aftershock".into()],
        },
    ];
    let order = dependency_order(&mods).expect("an order exists");
    assert_eq!(order, vec!["dda", "aftershock", "exoplanet"]);
}

#[test]
fn a_cycle_is_an_error_and_names_the_mods_in_it() {
    let mods = vec![
        ModInfo {
            id: "a".into(),
            name: "A".into(),
            dependencies: vec!["b".into()],
        },
        ModInfo {
            id: "b".into(),
            name: "B".into(),
            dependencies: vec!["a".into()],
        },
    ];
    let error = dependency_order(&mods).expect_err("no order exists");
    assert!(
        error.contains('a') && error.contains('b'),
        "the error names them: {error}"
    );
}

#[test]
fn every_modinfo_file_the_reference_ships_can_be_read_and_ordered() {
    // The reference's own data, not a fixture: 42 files, every one parseable, and the whole set
    // orderable. A cycle here would mean the reference could not start either.
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/mods");
    let mut files = 0;
    let mut all = Vec::new();
    for entry in std::fs::read_dir(&root).expect("the mods directory exists") {
        let path = entry
            .expect("a directory entry")
            .path()
            .join("modinfo.json");
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("the modinfo file reads");
        let value = parse(text.as_bytes()).expect("the modinfo file is valid JSON");
        all.extend(from_data(&value).expect("the modinfo file loads"));
        files += 1;
    }
    assert_eq!(files, 42, "modinfo files in the pinned data");
    assert!(
        all.iter().any(|m| m.id == "dda"),
        "the core mod is among them"
    );
    let order = dependency_order(&all).expect("the mod set is acyclic");
    assert_eq!(order.len(), all.len(), "every mod appears exactly once");
    let position = |id: &str| order.iter().position(|m| m == id).expect("present");
    for m in &all {
        for dep in &m.dependencies {
            if order.contains(dep) {
                assert!(
                    position(dep) < position(&m.id),
                    "{dep} must load before {}",
                    m.id
                );
            }
        }
    }
    println!("mods: {files} files, {} mods, order is acyclic", all.len());
}
