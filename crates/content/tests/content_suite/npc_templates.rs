//! The NPC templates the data writes by hand: what a placed person starts as.
//!
//! Reference: `npc_template` (`npc.h`), read by `npc_template::load` and copied onto a person by `npc::load_npc_template` (`npc.cpp:520-568`).

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

/// **The templates are the data's, exactly.** The oracle's prototype artifact has no `npc` registry (it covers classes and types, not the hand-written people), so this gate
/// reads the **raw data files** itself and compares the id set with the loader's, both ways. Two readers of the same files disagreeing is the failure this catches - and it
/// caught one on the way in: the first version of this test asked the artifact for a registry that is not there.
#[test]
fn the_templates_are_the_datas_templates() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json");
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|kind| kind.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = cdda_json::parse(text.as_bytes()) else {
                continue;
            };
            let cdda_json::Value::Array(objects) = value else {
                continue;
            };
            for object in objects {
                let cdda_json::Value::Object(fields) = object else {
                    continue;
                };
                let is_a_template = matches!(fields.get("type"), Some(cdda_json::Value::Str(kind)) if kind == "npc");
                if !is_a_template {
                    continue;
                }
                if let Some(cdda_json::Value::Str(id)) = fields.get("id") {
                    wanted.insert(id.clone());
                }
            }
        }
    }
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let templates = cdda_content::npc_templates::load_paths(&dirs).expect("the templates load");
    let ours: BTreeSet<String> = templates
        .iter()
        .map(|template| template.id.clone())
        .collect();
    let invented: Vec<&String> = ours.difference(&wanted).collect();
    let missing: Vec<&String> = wanted.difference(&ours).collect();
    assert!(invented.is_empty(), "invented ids: {invented:?}");
    assert!(missing.is_empty(), "missing ids: {missing:?}");
    assert!(
        ours.len() > 100,
        "the corpus has templates to read: {}",
        ours.len()
    );
}

/// **A template says who the person is.** `SEER_Brigitte_LaCroix` is the test case because it names every piece: a fixed name, a suffix, a gender, a class, a chat topic and a
/// mission it offers - and its attitude and mission are **enum numbers** (7 and 3), which is exactly the shape that silently reads as zero if the loader only looks for words.
#[test]
fn a_template_names_the_person_it_places() {
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let templates = cdda_content::npc_templates::load_paths(&dirs).expect("the templates load");
    let seer = templates
        .iter()
        .find(|template| template.id == "SEER_Brigitte_LaCroix")
        .expect("the bone seer");
    assert_eq!(seer.name_unique, "Brigitte LaCroix");
    assert_eq!(seer.name_suffix, "Bone Seer");
    assert_eq!(seer.gender, "female");
    assert_eq!(seer.class, "NC_BONE_SEER");
    assert_eq!(seer.chat, "TALK_BONE_SEER");
    assert_eq!(seer.faction, "no_faction");
    assert_eq!(seer.mission_offered, "MISSION_SEER_GATHER_BONE");
    // **Seven is the sixth legacy slot, not "wait"**: the numbers are fixed by the data, and a port that renumbered them would give the seer the wrong attitude.
    assert_eq!(
        seer.attitude, "legacy_6",
        "**attitude 7 is NPCATT_LEGACY_6**"
    );
    assert_eq!(
        seer.mission, "shopkeep",
        "**mission 3 is NPC_MISSION_SHOPKEEP**"
    );
}
