//! The mapgen registry's own key list, as the generator gate will index it.
//!
//! Source: `--dump-mapgens` from the instrumented reference, which writes every `mapgen` key the game can generate a chunk from. The palette a chunk is built from is looked up
//! by these keys, so the port cannot generate anything until it has the same list.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

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

#[test]
fn the_port_carries_the_code_keys_and_refuses_the_selectors_in_its_own_source() {
    // The two lists live in the library now, not only in this test, because a registry has to be built from them at run time. Duplicates would hide a missing key by making the list
    // look complete, and a selector appearing among the chunks would generate something the game cannot.
    use std::collections::BTreeSet;

    let code: BTreeSet<&str> = cdda_content::mapgen::CODE_REGISTERED.into_iter().collect();
    assert_eq!(code.len(), 26, "no duplicates");
    let selectors = cdda_content::mapgen::NOT_CHUNKS;
    assert_eq!(selectors.len(), 4);
    let reference = keys();
    for key in &code {
        assert!(reference.contains(*key), "{key} is in the reference's registry");
    }
    for key in selectors {
        assert!(!reference.contains(key), "{key} is not a chunk");
    }
    assert!(BTreeSet::from_iter(code).is_disjoint(&BTreeSet::from_iter(selectors)));
}

/// Every `mapgen` key written in the data, derived the way a registry would be built.
fn keys_in_the_data() -> BTreeSet<String> {
    fn walk(directory: &Path, into: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, into);
            } else if path.extension().is_some_and(|extension| extension == "json") {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(value) = parse(text.as_bytes()) else {
                    continue;
                };
                let Value::Array(entries) = value else { continue };
                for entry in &entries {
                    let Value::Object(object) = entry else { continue };
                    if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "mapgen") {
                        continue;
                    }
                    let mut stack = vec![object.get("om_terrain")];
                    while let Some(value) = stack.pop() {
                        match value {
                            Some(Value::Str(key)) => {
                                into.insert(key.clone());
                            }
                            Some(Value::Array(items)) => stack.extend(items.iter().map(Some)),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    let mut keys = BTreeSet::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    walk(&root, &mut keys);
    keys
}

#[test]
fn the_registry_from_the_data_plus_the_code_keys_is_exactly_the_reference_s_list() {
    // The gate the mapgen family starts from. Three sets, one answer: what the data writes, plus what the code registers, minus what is not a chunk at all. Anything else means the
    // port would generate a chunk the game cannot, or fail to generate one it can - and both look identical from a green suite.
    let mut registry = keys_in_the_data();
    for key in cdda_content::mapgen::CODE_REGISTERED {
        registry.insert(key.to_string());
    }
    for key in cdda_content::mapgen::NOT_CHUNKS {
        registry.remove(key);
    }
    let reference = keys();
    let missing: Vec<&String> = reference.difference(&registry).collect();
    let extra: Vec<&String> = registry.difference(&reference).collect();
    assert!(missing.is_empty(), "the port would miss {} chunks: {:?}", missing.len(), &missing[..missing.len().min(8)]);
    assert!(extra.is_empty(), "the port would offer {} chunks: {:?}", extra.len(), &extra[..extra.len().min(8)]);
    assert_eq!(registry.len(), 6306, "and the answer is the reference's own count");
}

#[test]
fn the_loader_s_own_extractor_agrees_with_the_registry_walk() {
    // Two pieces of code now read `om_terrain`: the test's walker, which builds the registry, and `mapgen::keys_of`, which a loader uses on one object at a time. If they disagree,
    // the registry gate passes while the loader that feeds the generator gets a different set - so they are compared over the whole data rather than trusted to match by inspection.
    fn walk(directory: &Path, into: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(directory) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, into);
            } else if path.extension().is_some_and(|extension| extension == "json") {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Ok(Value::Array(entries)) = parse(text.as_bytes()) else { continue };
                for entry in entries {
                    let Value::Object(object) = &entry else { continue };
                    for key in cdda_content::mapgen::keys_of(object).terrains {
                        into.push((text.clone(), key));
                    }
                }
            }
        }
    }

    let mut from_loader: BTreeSet<String> = BTreeSet::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    walk(&root, &mut pairs);
    for (_, key) in pairs {
        from_loader.insert(key);
    }
    let expected = keys_in_the_data();
    assert_eq!(from_loader.len(), expected.len(), "the same number of keys");
    assert_eq!(from_loader, expected, "and the same keys, not just the same count");
}

#[test]
fn the_chunk_the_map_dump_came_from_is_a_key_the_registry_has() {
    // Ties the two corpora together: the map dump names the overmap terrain it was generated for, and that name has to be a key in the registry - otherwise the port knows the
    // chunk's terrain without being able to ask for that chunk again. `forest_thick` is one of the twenty-six registered in code, which makes this the one case where the two
    // lists have to agree.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/maps/reference_map.json");
    let text = std::fs::read_to_string(path).expect("the reference map exists");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else {
        panic!("the map's root is not an object");
    };
    let Some(Value::Str(oter)) = root.get("oter") else {
        panic!("the map names no overmap terrain");
    };
    assert!(keys().contains(oter), "{oter} is the chunk the map dump came from");
    assert!(
        cdda_content::mapgen::CODE_REGISTERED.contains(&oter.as_str()),
        "and it is one of the keys the data does not write"
    );
}

#[test]
fn the_data_writes_four_keys_fewer_than_the_reference_registers() {
    // The arithmetic of the registry, pinned on its own so a change in either list is visible without reading a diff: the data writes 6,284 keys, the code registers 26, four of the
    // data's keys are selectors rather than chunks, and the reference ends with 6,306.
    let in_data = keys_in_the_data().len();
    assert_eq!(in_data, 6284, "keys written in the data");
    assert_eq!(in_data + cdda_content::mapgen::CODE_REGISTERED.len() - cdda_content::mapgen::NOT_CHUNKS.len(), 6306);
    assert_eq!(keys().len(), 6306, "and the reference agrees");
}

#[test]
fn a_mapgen_entry_keeps_its_map_under_object_and_never_at_the_top_level() {
    // The trap this pins: reading `entry["rows"]` looks right and finds nothing. Of the 6,216 mapgen entries, **none** declare rows or a terrain palette at the top level; the map
    // lives inside `object` in all 6,216, and `object` itself varies - some carry only `place_liquids`, some a `rows` grid, some nested mapgen ids. A generator written against the obvious shape
    // would compile, run, and draw nothing.
    fn shapes(directory: &Path, counts: &mut std::collections::BTreeMap<&'static str, usize>) {
        let Ok(entries) = std::fs::read_dir(directory) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                shapes(&path, counts);
            } else if path.extension().is_some_and(|extension| extension == "json") {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Ok(Value::Array(entries)) = parse(text.as_bytes()) else { continue };
                for entry in &entries {
                    let Value::Object(object) = entry else { continue };
                    if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "mapgen") {
                        continue;
                    }
                    *counts.entry("entries").or_default() += 1;
                    if object.get("rows").is_some() {
                        *counts.entry("rows at the top").or_default() += 1;
                    }
                    if object.get("terrain").is_some() {
                        *counts.entry("palette at the top").or_default() += 1;
                    }
                    if object.get("object").is_some() {
                        *counts.entry("under object").or_default() += 1;
                    }
                }
            }
        }
    }

    let mut counts = std::collections::BTreeMap::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    shapes(&root, &mut counts);
    assert_eq!(counts.get("entries"), Some(&6216), "mapgen entries");
    assert_eq!(counts.get("rows at the top"), None, "none writes rows at the top level");
    assert_eq!(counts.get("palette at the top"), None, "none writes a palette there either");
    assert_eq!(counts.get("under object"), Some(&6216), "and **every** entry keeps its map under object");
}

#[test]
fn the_eight_ways_the_data_declares_a_chunk_s_terrain_are_stable() {
    // Every combination of "draws a grid", "declares terrain inline" and "names palettes", counted over the whole data. A generator has to satisfy all eight, and four of them are the
    // ones that are easy to get wrong: no grid at all, a palette lookup that reading `terrain` would skip, a grid with no terrain anywhere, and placement-only entries.
    fn count(directory: &Path, into: &mut std::collections::BTreeMap<(bool, bool, bool), usize>) {
        let Ok(entries) = std::fs::read_dir(directory) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count(&path, into);
            } else if path.extension().is_some_and(|extension| extension == "json") {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Ok(Value::Array(entries)) = parse(text.as_bytes()) else { continue };
                for entry in &entries {
                    let Value::Object(object) = entry else { continue };
                    if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "mapgen") {
                        continue;
                    }
                    let Some(Value::Object(inner)) = object.get("object") else { continue };
                    let shape = (
                        inner.get("rows").is_some(),
                        inner.get("terrain").is_some(),
                        inner.get("palettes").is_some(),
                    );
                    *into.entry(shape).or_default() += 1;
                }
            }
        }
    }

    let mut counts = std::collections::BTreeMap::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    count(&root, &mut counts);
    let expected = [
        ((true, false, true), 2411),
        ((true, true, true), 1701),
        ((false, false, false), 1302),
        ((true, true, false), 627),
        ((true, false, false), 150),
        ((false, false, true), 17),
        ((false, true, false), 7),
        ((false, true, true), 1),
    ];
    for (shape, number) in expected {
        assert_eq!(counts.get(&shape).copied().unwrap_or(0), number, "shape {shape:?}");
    }
    assert_eq!(counts.values().sum::<usize>(), 6216, "and the eight partition the data");
}
