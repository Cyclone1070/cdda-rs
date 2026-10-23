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

#[test]
fn the_keys_the_data_does_not_write_are_registered_in_code() {
    // The data's `om_terrain` ids come to 6,284 and the reference knows 6,306. The twenty-six that are left are generators built in **code**, and this is all of them: forests,
    // rivers, lakes and oceans, ravines, and the subway pieces. A port that builds its registry by scanning the data comes up short by exactly these, so the list is pinned - not
    // summarised - because each missing key is a chunk that cannot be generated, and a count would not say which.
    let keys = keys();
    let in_code = [
        "forest",
        "forest_thick",
        "forest_water",
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
        "lab_subway_curved",
        "lab_subway_end",
        "lab_subway_four_way",
        "lab_subway_straight",
        "lab_subway_tee",
        "subway_curved",
        "subway_end",
        "subway_four_way",
        "subway_straight",
        "subway_tee",
    ];
    assert_eq!(in_code.len(), 26, "no more and no fewer");
    for key in in_code {
        assert!(keys.contains(key), "{key} is registered in code, not in the data");
    }
}

#[test]
fn the_four_data_keys_the_reference_never_registers_stay_out() {
    // The other direction of the same comparison. Four `om_terrain` ids appear in the data and not in the reference's registry, and the names say why: `lab_1side`, `lab_4side` and
    // `lab_finale_1level` are **floorplan selectors** rather than chunk ids - the lab generator picks among them - and `null` is the plaza's placeholder. A port that trusted the
    // files alone would offer four chunks the game itself cannot generate, which is the mirror of the twenty-six it would miss.
    let keys = keys();
    for key in ["lab_1side", "lab_4side", "lab_finale_1level", "null"] {
        assert!(!keys.contains(key), "{key} is a selector, not a chunk");
    }
}
