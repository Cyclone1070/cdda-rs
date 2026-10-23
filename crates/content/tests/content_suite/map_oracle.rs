//! A map chunk the reference generated, as the map-generation gate will compare against.
//!
//! Source: `--dump-map` from the instrumented reference, which writes one chunk's terrain ids exactly as the generator left them. Before this round the dump hung thirteen
//! rows in, because a read past the loaded chunk pulls in a submap and then waits for overmap generation that a headless run never provides; the harness now asks only for
//! squares the map holds and writes an empty id otherwise.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_json::{Value, parse};
use std::collections::BTreeSet;
use std::path::Path;

fn dump() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/maps/reference_map.json");
    let text = std::fs::read_to_string(path).expect("the reference map exists");
    parse(text.as_bytes()).expect("the map is valid JSON")
}

fn ids() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/terrain.json");
    let text = std::fs::read_to_string(path).expect("the terrain dump exists");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("no prototypes");
    };
    prototypes
        .iter()
        .filter_map(|entry| match entry {
            Value::Object(object) => match object.get("id") {
                Some(Value::Str(id)) => Some(id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn the_reference_map_is_one_chunk_of_the_overmap_it_names() {
    // A chunk is two submaps a side: twenty-four by twenty-four, with the overmap terrain it belongs to and the overmap square it sits on. A dump that lost a row would otherwise
    // read as a different chunk rather than a broken dump.
    let value = dump();
    let Value::Object(root) = &value else { panic!("root") };
    let Some(Value::Array(rows)) = root.get("rows") else {
        panic!("no rows");
    };
    assert_eq!(rows.len(), 24, "twenty-four rows");
    for (index, row) in rows.iter().enumerate() {
        let Value::Array(squares) = row else { panic!("row {index} is not an array") };
        assert_eq!(squares.len(), 24, "twenty-four squares in row {index}");
    }
    assert!(matches!(root.get("oter"), Some(Value::Str(_))), "the chunk names its overmap terrain");
}

#[test]
fn every_terrain_in_the_generated_chunk_is_a_terrain_the_loader_knows() {
    // The point of the map oracle: a generated chunk uses only ids that exist. An id the loader has never seen would be a generator inventing terrain rather than reading it, and
    // it is the first thing a port's own generation would get wrong.
    let known = ids();
    let value = dump();
    let Value::Object(root) = &value else { panic!("root") };
    let Some(Value::Array(rows)) = root.get("rows") else {
        panic!("no rows");
    };
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for row in rows {
        let Value::Array(squares) = row else { continue };
        for square in squares {
            match square {
                Value::Str(id) => {
                    assert!(known.contains(id), "{id} is not a terrain the reference defines");
                    seen.insert(id);
                }
                _ => panic!("a square is not an id"),
            }
        }
    }
    assert!(seen.len() > 1, "a generated chunk is not one flat terrain: {} kinds", seen.len());
}
