//! What a round does when it hits, for every ammunition type.
//!
//! `corpus/items/ammo-effects.json` is `--dump-ammo-effects`: 736 ammo types and their resolved effects. The reference reads an ammunition's `"effects"` member
//! into `islot_ammo::ammo_effects` with an **`auto_flags_reader`** (`src/item_factory.cpp:2990`), and that reader is a container reader like any other, so three
//! rules apply at once: the member is **cleared when stated**, `extend` **adds** to what was inherited, and `delete` **takes names away**. 231 items `extend` their
//! effects and 177 `delete` some, which is why a port that only reads a stated list quietly loses the effects those rounds inherited.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_json::Value;
use std::collections::BTreeMap;

fn recorded() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/ammo-effects.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

fn loaded() -> BTreeMap<String, ItemType> {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect()
}

#[test]
fn every_ammunition_has_the_effects_the_reference_has() {
    let Value::Object(root) = recorded() else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    assert_eq!(types.len(), 736, "every ammo type was recorded");
    let items = loaded();

    let mut differing: Vec<String> = Vec::new();
    let mut with_effects = 0;
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("a type id")
        };
        let Some(Value::Array(effects)) = entry.get("effects") else {
            panic!("{id} has effects")
        };
        let expected: std::collections::BTreeSet<String> = effects
            .iter()
            .map(|effect| match effect {
                Value::Str(effect) => effect.clone(),
                other => panic!("an effect: {other:?}"),
            })
            .collect();
        if !expected.is_empty() {
            with_effects += 1;
        }
        seen.extend(expected.iter().cloned());
        let Some(item) = items.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        if item.ammo_effects != expected {
            differing.push(format!(
                "{id}: the port says {:?}, the reference says {expected:?}",
                item.ammo_effects
            ));
        }
    }
    assert!(
        with_effects > 200,
        "most rounds do something when they land: {with_effects}"
    );
    assert!(seen.len() > 10, "and they do a variety of things: {seen:?}");
    assert!(
        differing.is_empty(),
        "{} of 736 types disagree; the first ten are {:#?}",
        differing.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

/// A fixture that copies from an ammunition and states **only** an `extend`.
///
/// This is the case the content does not contain but the reference still has to answer: `islot_ammo::deserialize` runs for **every** type that holds an ammo slot,
/// and a `copy-from` child holds the one it inherited, so the child's `effects` is the parent's list plus its own `extend`. Asking the object whether it declares
/// `"subtypes": ["AMMO"]` instead answers "no" for this child and drops the whole block.
#[test]
fn a_child_that_states_only_an_extend_still_reads_the_effects_it_inherits() {
    use cdda_content::items;

    let dir = std::env::temp_dir().join("cdda-ammo-effect-inheritance");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("items.json"),
        r#"[
        { "id": "test_round_base", "type": "ITEM", "subtypes": [ "AMMO" ],
          "name": { "str": "round" }, "symbol": "=", "color": "yellow",
          "weight": "1 g", "volume": "1 ml", "price": "1 USD", "material": [ "steel" ],
          "ammo_type": "test_round", "effects": [ "COOKOFF" ] },
        { "id": "test_round_fmj", "type": "ITEM", "copy-from": "test_round_base",
          "extend": { "effects": [ "RECYCLED" ] } }
        ]"#,
    )
    .expect("fixture");
    let loaded = items::load_paths(&[&dir]).expect("fixture loads");
    let round = loaded
        .iter()
        .find(|item| item.id == "test_round_fmj")
        .expect("the child loaded");
    assert_eq!(
        round.ammo_effects,
        ["COOKOFF".to_owned(), "RECYCLED".to_owned()]
            .into_iter()
            .collect(),
        "the child keeps what it inherited and adds what it extended"
    );
}
