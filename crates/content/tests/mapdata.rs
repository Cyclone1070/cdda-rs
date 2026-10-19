//! Loading terrain and furniture, against the reference's own rules.
//!
//! The oracle for this is `corpus/prototypes/terrain.json`: every prototype the
//! reference produced after loading, field by field. The fixture tests here pin
//! the rules one at a time; the corpus test at the end runs all 1,769 through
//! both and compares.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::mapdata::{MapData, MapDataKind};
use cdda_json::parse;

/// Loads one list of objects, as a file would deliver them.
fn load(text: &str) -> Vec<MapData> {
    let value = parse(text.as_bytes()).expect("fixture parses");
    cdda_content::mapdata::load_objects(&value, "core").expect("loads")
}

fn only(text: &str) -> MapData {
    let mut loaded = load(text);
    assert_eq!(loaded.len(), 1, "expected one prototype");
    loaded.remove(0)
}

#[test]
fn a_terrain_takes_its_name_symbol_colour_and_move_cost() {
    let terrain = only(
        r##"[
        {
          "type": "terrain",
          "id": "t_test",
          "name": "a test tile",
          "symbol": "#",
          "color": "light_gray",
          "move_cost": 2,
          "flags": [ "TRANSPARENT" ]
        }
        ]"##,
    );
    assert_eq!(terrain.kind, MapDataKind::Terrain);
    assert_eq!(terrain.id, "t_test");
    assert_eq!(terrain.name, "a test tile");
    assert_eq!(terrain.symbol, [i32::from(b'#'); 4]);
    assert_eq!(terrain.color_name(0), "c_light_gray");
    assert_eq!(terrain.move_cost, 2);
    assert!(terrain.flags.iter().any(|flag| flag == "TRANSPARENT"));
}

#[test]
fn a_symbol_list_is_per_season_and_a_single_entry_fills_all_of_them() {
    let seasons = only(
        r#"[
        { "type": "terrain", "id": "t_seasons", "name": "x", "symbol": [ "a", "b", "c", "d" ],
          "color": [ "red", "green", "blue", "white" ], "move_cost": 2 }
        ]"#,
    );
    assert_eq!(seasons.symbol, [97, 98, 99, 100]);
    assert_eq!(
        [
            seasons.color_name(0),
            seasons.color_name(1),
            seasons.color_name(2),
            seasons.color_name(3),
        ],
        ["c_red", "c_green", "c_blue", "c_white"]
    );

    let one = only(
        r#"[
        { "type": "terrain", "id": "t_one", "name": "x", "symbol": [ "z" ], "color": "red",
          "move_cost": 2 }
        ]"#,
    );
    assert_eq!(one.symbol, [122; 4]);
}

#[test]
fn a_child_inherits_what_it_does_not_mention() {
    let loaded = load(
        r#"[
        { "type": "terrain", "abstract": "base", "name": "base name", "symbol": ".",
          "color": "brown", "move_cost": 2, "flags": [ "DIGGABLE", "TRANSPARENT" ] },
        { "type": "terrain", "id": "t_child", "copy-from": "base", "symbol": "-" }
        ]"#,
    );
    // The abstract is a template, not a prototype; the child is the prototype.
    let child = loaded
        .iter()
        .find(|entry| entry.id == "t_child")
        .expect("child");
    assert_eq!(child.name, "base name");
    assert_eq!(child.move_cost, 2);
    assert_eq!(child.symbol, [i32::from(b'-'); 4]);
    assert_eq!(child.color_name(0), "c_brown");
    assert_eq!(
        child.flags.iter().cloned().collect::<Vec<_>>(),
        vec!["DIGGABLE", "TRANSPARENT"]
    );
}

#[test]
fn naming_flags_replaces_them_and_extend_and_delete_adjust_them() {
    let loaded = load(
        r#"[
        { "type": "terrain", "abstract": "base", "name": "base", "symbol": ".",
          "color": "brown", "move_cost": 2, "flags": [ "DIGGABLE", "TRANSPARENT" ] },
        { "type": "terrain", "id": "t_replace", "copy-from": "base", "symbol": "-",
          "flags": [ "FLAT" ] },
        { "type": "terrain", "id": "t_extend", "copy-from": "base", "symbol": "-",
          "extend": { "flags": [ "FLAT" ] } },
        { "type": "terrain", "id": "t_delete", "copy-from": "base", "symbol": "-",
          "delete": { "flags": [ "DIGGABLE" ] } }
        ]"#,
    );
    let flags_of = |id: &str| {
        loaded
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.flags.iter().cloned().collect::<Vec<_>>())
            .expect("prototype")
    };
    assert_eq!(flags_of("t_replace"), vec!["FLAT"]);
    assert_eq!(
        flags_of("t_extend"),
        vec!["DIGGABLE", "FLAT", "TRANSPARENT"]
    );
    assert_eq!(flags_of("t_delete"), vec!["TRANSPARENT"]);
}

/// Every terrain and furniture prototype the reference produced, keyed by kind
/// and id.
#[allow(clippy::type_complexity)]
fn reference_prototypes() -> std::collections::BTreeMap<
    (String, String),
    (String, [i32; 4], String, i32, i32, i32, Vec<String>),
> {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/prototypes/terrain.json"),
    )
    .expect("oracle artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let cdda_json::Value::Object(root) = value else {
        panic!("root object")
    };
    let Some(cdda_json::Value::Array(entries)) = root.get("prototypes") else {
        panic!("prototype list")
    };
    let mut out = std::collections::BTreeMap::new();
    for entry in entries {
        let cdda_json::Value::Object(object) = entry else {
            continue;
        };
        let text_of = |member: &str| match object.get(member) {
            Some(cdda_json::Value::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let number_of = |member: &str| match object.get(member) {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(i32::MAX),
            _ => 0,
        };
        let mut symbol = [0i32; 4];
        if let Some(cdda_json::Value::Array(items)) = object.get("symbol") {
            for (slot, item) in symbol.iter_mut().zip(items) {
                if let cdda_json::Value::Int(value) = item {
                    *slot = i32::try_from(*value).unwrap_or(i32::MAX);
                }
            }
        }
        // The dump carries one colour: the colour of the season the game starts
        // in. That is spring, so index 0 is what it reports.
        let color = text_of("color");
        let mut flags = Vec::new();
        if let Some(cdda_json::Value::Array(items)) = object.get("flags") {
            for item in items {
                if let cdda_json::Value::Str(text) = item {
                    flags.push(text.clone());
                }
            }
        }
        out.insert(
            (text_of("kind"), text_of("id")),
            (
                text_of("name"),
                symbol,
                color,
                number_of("move_cost"),
                number_of("light"),
                number_of("coverage"),
                flags,
            ),
        );
    }
    out
}

#[test]
fn the_whole_content_set_matches_the_reference_field_by_field() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA");
    let loaded =
        cdda_content::mapdata::load_paths(&[&root.join("data/core"), &root.join("data/json")])
            .expect("loads the content set");
    println!("loaded {} prototypes", loaded.len());
    let reference = reference_prototypes();
    let mut mismatches = Vec::new();
    for prototype in &loaded {
        let key = (
            match prototype.kind {
                MapDataKind::Terrain => "terrain".to_owned(),
                MapDataKind::Furniture => "furniture".to_owned(),
            },
            prototype.id.clone(),
        );
        let Some((name, ref_symbol, ref_color, move_cost, light, coverage, flags)) =
            reference.get(&key)
        else {
            mismatches.push(format!("{}: not in the reference", prototype.id));
            continue;
        };
        if prototype.symbol != *ref_symbol {
            mismatches.push(format!(
                "{}: symbol {:?} != {ref_symbol:?}",
                prototype.id, prototype.symbol
            ));
        }
        if prototype.color_name(0) != ref_color {
            mismatches.push(format!(
                "{}: color {:?} != {ref_color:?}",
                prototype.id,
                prototype.color_name(0)
            ));
        }
        if &prototype.name != name {
            mismatches.push(format!(
                "{}: name {:?} != {name:?}",
                prototype.id, prototype.name
            ));
        }
        if prototype.move_cost != *move_cost {
            mismatches.push(format!(
                "{}: move_cost {} != {move_cost}",
                prototype.id, prototype.move_cost
            ));
        }
        if prototype.light != *light {
            mismatches.push(format!(
                "{}: light {} != {light}",
                prototype.id, prototype.light
            ));
        }
        if prototype.coverage != *coverage {
            mismatches.push(format!(
                "{}: coverage {} != {coverage}",
                prototype.id, prototype.coverage
            ));
        }
        let ours: Vec<String> = prototype.flags.iter().cloned().collect();
        if &ours != flags {
            mismatches.push(format!("{}: flags {ours:?} != {flags:?}", prototype.id));
        }
    }
    println!("{} mismatches", mismatches.len());
    for line in mismatches.iter().take(15) {
        println!("  {line}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
