//! The mapgen registry's own key list, as the generator gate will index it.
//!
//! Source: `--dump-mapgens` from the instrumented reference, which writes every `mapgen` key the game can generate a chunk from. The palette a chunk is built from is looked up
//! by these keys, so the port cannot generate anything until it has the same list.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::{Value, parse};
use std::collections::BTreeSet;
use std::path::Path;

fn keys() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/mapgens/palette_keys.json");
    let text = std::fs::read_to_string(path).expect("the mapgen key dump exists");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(entries)) = root.get("keys") else {
        panic!("the dump has no keys");
    };
    entries
        .iter()
        .filter_map(|entry| match entry {
            Value::Str(key) => Some(key.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_reference_knows_thousands_of_mapgen_keys_and_the_port_must_not_invent_one() {
    // **6,306 keys**, and every one is a chunk a player can walk into. The count is the gate: a loader that silently skipped a family - furniture-only mapgens, nested palettes,
    // modded ones - would still pass a "more than zero" test, and the first missing key is a chunk the port cannot generate.
    let keys = keys();
    assert_eq!(keys.len(), 6306, "mapgen keys");
    assert!(keys.contains("forest_thick"), "the chunk the map dump came from is one of them");
    assert!(
        keys.contains("2StoryModern03_1"),
        "and the list is not sorted by family, so a reader that assumed an order would be wrong"
    );
}
