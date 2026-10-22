//! The NPC class table: every class the reference defines, and nothing else.
//!
//! `npc_class` decides what an NPC is before they are given a personality - which skills they start with, what they carry,
//! what they wear. The port had no loader for it, so the whole family of NPC-bearing mapgen was untranslatable. This gate is
//! the same one the other eighty-odd registries have: the ids the reference reports, exactly, in both directions.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

fn reference_ids(registry: &str) -> BTreeSet<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/ids.json");
    let text = std::fs::read_to_string(&path).expect("the prototype artifact");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the artifact is an object")
    };
    let Some(cdda_json::Value::Array(registries)) = root.get("registries") else {
        panic!("registries")
    };
    for entry in registries {
        let cdda_json::Value::Object(fields) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(name)) = fields.get("type") else {
            continue;
        };
        if name != registry {
            continue;
        }
        let Some(cdda_json::Value::Array(ids)) = fields.get("ids") else {
            panic!("ids")
        };
        return ids
            .iter()
            .map(|id| match id {
                cdda_json::Value::Str(text) => text.clone(),
                other => panic!("an id that is not a string: {other:?}"),
            })
            .collect();
    }
    panic!("the artifact has no registry {registry}");
}

#[test]
fn the_class_registry_is_the_json_ids_and_nothing_else() {
    let reference = reference_ids("npc_class");
    assert_eq!(
        reference.len(),
        155,
        "the reference defines this many classes"
    );
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let classes = cdda_content::npc_classes::load_paths(&dirs).expect("the classes load");
    let ours: BTreeSet<String> = classes.iter().map(|class| class.id.clone()).collect();
    let invented: Vec<&String> = ours.difference(&reference).collect();
    let missing: Vec<&String> = reference.difference(&ours).collect();
    assert!(invented.is_empty(), "invented ids: {invented:?}");
    assert!(missing.is_empty(), "missing ids: {missing:?}");
    assert_eq!(ours.len(), reference.len());
}

#[test]
fn a_class_carries_the_name_the_data_gives_it() {
    // The name is a translated string object in the data (`{ "str": "No class" }`), not a bare string, which is the sort of
    // shape that silently loads as empty if the reader only looks for one form.
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let classes = cdda_content::npc_classes::load_paths(&dirs).expect("the classes load");
    let no_class = classes
        .iter()
        .find(|class| class.id == "NC_NONE")
        .expect("the default class");
    assert_eq!(no_class.name, "No class");
    let named = classes
        .iter()
        .filter(|class| !class.name.is_empty())
        .count();
    assert!(
        named > 140,
        "{named} classes of {} have a name",
        classes.len()
    );
}
