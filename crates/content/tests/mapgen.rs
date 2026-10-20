//! Mapgen keys against the reference's own registry.
//!
//! The oracle is `corpus/mapgens.json`: every overmap terrain mapgen key the reference had after
//! loading the whole game data, deduplicated.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::mapgen;
use cdda_json::{Value, parse};
use std::path::{Path, PathBuf};

const BUILT_IN: [&str; 26] = [
    "forest",
    "forest_thick",
    "forest_water",
    "lab_subway_curved",
    "lab_subway_end",
    "lab_subway_four_way",
    "lab_subway_straight",
    "lab_subway_tee",
    "lake_shore",
    "ocean_shore",
    "ravine_edge",
    "ravine_floor_edge",
    "river",
    "river_c_not_ne",
    "river_c_not_nw",
    "river_c_not_se",
    "river_c_not_sw",
    "river_ne",
    "river_nw",
    "river_se",
    "river_sw",
    "subway_curved",
    "subway_end",
    "subway_four_way",
    "subway_straight",
    "subway_tee",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference() -> Vec<String> {
    let text = std::fs::read_to_string(root().join("corpus/mapgens.json")).expect("artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let Value::Object(object) = value else {
        panic!("root")
    };
    let Some(Value::Array(keys)) = object.get("keys") else {
        panic!("keys")
    };
    keys.iter()
        .map(|key| match key {
            Value::Str(text) => text.clone(),
            other => panic!("a key that is not a string: {other:?}"),
        })
        .collect()
}

#[test]
fn every_mapgen_the_reference_registered_is_found_in_the_data() {
    let reference = reference();
    let index =
        mapgen::load_paths(&[&root().join("references/Cataclysm-DDA/data/json")]).expect("loads");
    println!(
        "reference has {} keys, the data has {}",
        reference.len(),
        index.terrains.len()
    );
    let ours: Vec<&String> = index.terrains.iter().collect();
    // Twenty-six keys are built in C++ rather than written in JSON (`src/mapgen.cpp`, the
    // `register_mapgen_function` calls for rivers, subways, forests and lake shores). They are
    // expected to be missing from a scan of the data files, and nothing else is.

    let missing: Vec<&String> = reference
        .iter()
        .filter(|key| !index.terrains.contains(*key))
        .collect();
    let unexpected: Vec<&String> = missing
        .iter()
        .copied()
        .filter(|key| !BUILT_IN.contains(&key.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{} keys are neither in the data nor known built-ins: {:?}",
        unexpected.len(),
        unexpected
    );
    assert_eq!(
        missing.len(),
        BUILT_IN.len(),
        "and every built-in is accounted for: {missing:?}"
    );
    // Four keys exist in the data and not in the registry: the three `lab_1side`-style maps are
    // used from C++ only, and `null` is the dummy the factory erases on setup.
    let extra: Vec<&String> = ours
        .iter()
        .copied()
        .filter(|key| !reference.contains(key))
        .collect();
    for key in &extra {
        assert!(
            ["lab_1side", "lab_4side", "lab_finale_1level", "null"].contains(&key.as_str()),
            "{key} is in the data but not the registry, and is not one of the four known cases"
        );
    }
}

#[test]
fn a_mapgen_key_can_be_a_string_a_list_or_a_grid() {
    let parse_object = |text: &str| {
        let value = parse(text.as_bytes()).expect("parses");
        let Value::Object(object) = value else {
            panic!("object")
        };
        object
    };
    let single = mapgen::keys_of(&parse_object(
        r#"{"type":"mapgen","om_terrain":"house","object":{}}"#,
    ));
    assert_eq!(single.terrains, vec!["house"]);
    let list = mapgen::keys_of(&parse_object(
        r#"{"type":"mapgen","om_terrain":["house_north","house_south"],"object":{}}"#,
    ));
    assert_eq!(list.terrains, vec!["house_north", "house_south"]);
    let grid = mapgen::keys_of(&parse_object(
        r#"{"type":"mapgen","om_terrain":[["a","b"],["c"]],"object":{}}"#,
    ));
    assert_eq!(grid.terrains, vec!["a", "b", "c"]);
}

#[test]
fn something_that_is_not_a_mapgen_registers_nothing() {
    let value = parse(br#"{"type":"item","id":"rock","om_terrain":"house"}"#).expect("parses");
    let Value::Object(object) = value else {
        panic!("object")
    };
    assert!(mapgen::keys_of(&object).terrains.is_empty());
}
