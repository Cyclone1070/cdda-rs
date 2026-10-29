//! The light session replayed: a fire, a wall, and what each square can see by.
//!
//! The record (`corpus/fields/light-session.json`) carries every square's `lit` level, the light source of its own, and the ambient the sky gives it, on four levels every
//! turn. This replays **the fire's own level** against it: the terrain of the level comes from the record's grid, the fire's intensity comes from the record's fields, the
//! port's own light cast runs over them, and each square's answer is compared with the reference's.
//!
//! **What it compares: the answer and the emitter.** Every square's `lit` level and its own light source, from a fixture whose columns now agree with each other - the
//! first cut of the dump built every level before reading any, so it read whichever level had been built last and recorded a 160-lumen fire as a dark room (round 555).
//!
//! Two things are **given** rather than ported here, and both are declared:
//!
//! * the sky - each level's natural light is taken from the record, the same staging the gas sessions use for the wind, so a disagreement is about the fire and
//!   not about the sun;
//! * the other three levels - the reference casts a lightmap **per level** (`generate_lightmap`, `src/lightmap.cpp:443`), and so does this: the level below is dark
//!   because nothing there emits, not because the cast crosses floors.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldId;
use cdda_core_types::level::Level;
use cdda_core_types::light::{light_at, LitLevel};
use cdda_sim::field_table::FieldTable;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Prototypes, add_field_strength, cast_lights_with, Map, set_field_table};
use cdda_sim::{light};

/// Where the recorded window starts, so the level can be built from zero.
const ORIGIN: (i32, i32) = (60, 69);

/// One recorded square's light, once the port has cast its own.
struct Square {
    x: i32,
    y: i32,
    /// The level's natural light: what the sun offers, before anything reflects it.
    sky: f32,
    reference_lit: i32,
    /// The reference's own light source for the square, which is what the port's field light has to match.
    reference_source: f32,
}

/// One recorded turn: the squares of the fire's level, and the fields standing on it.
struct Turn {
    squares: Vec<Square>,
    fields: Vec<(i32, i32, String, i32)>,
}

fn session() -> Vec<Turn> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields/light-session.json");
    let text = std::fs::read_to_string(&path).expect("the recorded light session");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(turns)) = root.get("turns") else {
        panic!("turns")
    };
    turns
        .iter()
        .map(|turn| {
            let cdda_json::Value::Object(turn) = turn else {
                panic!("a turn")
            };
            let mut squares = Vec::new();
            if let Some(cdda_json::Value::Array(light)) = turn.get("light") {
                for entry in light {
                    let cdda_json::Value::Object(entry) = entry else {
                        panic!("a square")
                    };
                    let whole = |key: &str| match entry.get(key) {
                        Some(cdda_json::Value::Int(value)) => *value,
                        other => panic!("{key} is a whole number: {other:?}"),
                    };
                    if whole("z") != 0 {
                        continue;
                    }
                    let sky = match entry.get("sky") {
                        Some(cdda_json::Value::Float(value)) => *value as f32,
                        other => panic!("sky is a number: {other:?}"),
                    };
                    squares.push(Square {
                        x: i32::try_from(whole("x")).expect("x"),
                        y: i32::try_from(whole("y")).expect("y"),
                        sky,
                        reference_lit: i32::try_from(whole("lit")).expect("lit"),
                        reference_source: f32::from(i16::try_from(whole("source")).expect("source")),
                    });
                }
            }
            if let Some(cdda_json::Value::Array(grid)) = turn.get("grid") {
                let _ = grid;
            }
            let mut fields = Vec::new();
            if let Some(cdda_json::Value::Array(list)) = turn.get("fields") {
                for entry in list {
                    let cdda_json::Value::Object(entry) = entry else {
                        panic!("a field")
                    };
                    let whole = |key: &str| match entry.get(key) {
                        Some(cdda_json::Value::Int(value)) => *value,
                        other => panic!("{key} is a whole number: {other:?}"),
                    };
                    if whole("z") != 0 {
                        continue;
                    }
                    let kind = match entry.get("type") {
                        Some(cdda_json::Value::Str(kind)) => kind.clone(),
                        other => panic!("a field's type: {other:?}"),
                    };
                    fields.push((
                        i32::try_from(whole("x")).expect("x"),
                        i32::try_from(whole("y")).expect("y"),
                        kind,
                        i32::try_from(whole("intensity")).expect("intensity"),
                    ));
                }
            }
            Turn { squares, fields }
        })
        .collect()
}

/// The terrain of the window, from the record's own grid.
fn terrain_of_window() -> Vec<(i32, i32, String, String)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields/light-session.json");
    let text = std::fs::read_to_string(&path).expect("the recorded light session");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(grid)) = root.get("grid") else {
        panic!("grid")
    };
    let mut out = Vec::new();
    for entry in grid {
        let cdda_json::Value::Object(entry) = entry else {
            panic!("a square")
        };
        let whole = |key: &str| match entry.get(key) {
            Some(cdda_json::Value::Int(value)) => *value,
            other => panic!("{key} is a whole number: {other:?}"),
        };
        if whole("z") != 0 {
            continue;
        }
        let text = |key: &str| match entry.get(key) {
            Some(cdda_json::Value::Str(value)) => value.clone(),
            other => panic!("{key} is text: {other:?}"),
        };
        out.push((
            i32::try_from(whole("x")).expect("x"),
            i32::try_from(whole("y")).expect("y"),
            text("ter"),
            text("furn"),
        ));
    }
    out
}

/// The terrain and furniture tables the cast reads, built from the prototype dump.
fn prototypes() -> (Prototypes, std::collections::BTreeMap<String, u32>, std::collections::BTreeMap<String, u32>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/terrain.json");
    let text = std::fs::read_to_string(&path).expect("the terrain dump");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(list)) = root.get("prototypes") else {
        panic!("prototypes")
    };
    let mut terrain = TerrainTable::new();
    let mut furniture = TerrainTable::new();
    let mut terrain_codes = std::collections::BTreeMap::new();
    let mut furniture_codes = std::collections::BTreeMap::new();
    for (index, entry) in list.iter().enumerate() {
        let cdda_json::Value::Object(entry) = entry else {
            panic!("a prototype")
        };
        let id = match entry.get("id") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            other => panic!("an id: {other:?}"),
        };
        let kind = match entry.get("kind") {
            Some(cdda_json::Value::Str(kind)) => kind.clone(),
            other => panic!("a kind: {other:?}"),
        };
        let transparent = matches!(entry.get("transparent"), Some(cdda_json::Value::Bool(true)));
        let light = match entry.get("light") {
            Some(cdda_json::Value::Int(value)) => *value as f32,
            Some(cdda_json::Value::Float(value)) => *value as f32,
            _ => 0.0,
        };
        let code = u32::try_from(index + 1).expect("a code");
        if kind == "furniture" {
            furniture.insert(code, transparent);
            furniture.insert_light(code, light);
            furniture_codes.insert(id, code);
        } else {
            terrain.insert(code, transparent);
            terrain.insert_light(code, light);
            terrain_codes.insert(id, code);
        }
    }
    (Prototypes { terrain, furniture }, terrain_codes, furniture_codes)
}

/// The light each field type emits, by intensity, from the field prototype dump.
fn field_table() -> (FieldTable, std::collections::BTreeMap<String, u32>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(&path).expect("the field dump");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(list)) = root.get("prototypes") else {
        panic!("prototypes")
    };
    let mut table = FieldTable::new();
    let mut codes = std::collections::BTreeMap::new();
    for (index, entry) in list.iter().enumerate() {
        let cdda_json::Value::Object(entry) = entry else {
            panic!("a type")
        };
        let id = match entry.get("id") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            other => panic!("an id: {other:?}"),
        };
        let code = u32::try_from(index + 1).expect("a code");
        let Some(cdda_json::Value::Array(levels)) = entry.get("levels") else {
            panic!("levels")
        };
        let mut light = Vec::new();
        let mut translucency = Vec::new();
        for level in levels {
            let cdda_json::Value::Object(level) = level else {
                panic!("a level")
            };
            let number = |key: &str| match level.get(key) {
                Some(cdda_json::Value::Int(value)) => *value as f32,
                Some(cdda_json::Value::Float(value)) => *value as f32,
                _ => 0.0,
            };
            light.push(number("light"));
            translucency.push(number("translucency") / 10.0);
        }
        table.insert(code, &translucency);
        table.insert_light(code, &light);
        codes.insert(id, code);
    }
    (table, codes)
}

fn lit_name(level: LitLevel) -> &'static str {
    match level {
        LitLevel::Dark => "dark",
        LitLevel::Low => "low",
        LitLevel::BrightOnly => "bright-only",
        LitLevel::Lit => "lit",
        LitLevel::Bright => "bright",
        LitLevel::Memorized => "memorized",
        LitLevel::Blank => "blank",
    }
}

#[test]
fn the_port_s_cast_puts_the_fire_s_light_where_the_reference_put_it() {
    let turns = session();
    let window = terrain_of_window();
    let (prototypes_table, terrain_codes, furniture_codes) = prototypes();
    let (table, field_codes) = field_table();

    let mut disagreements: Vec<String> = Vec::new();
    let mut compared = 0_usize;
    for (index, turn) in turns.iter().enumerate() {
        let mut world = World::new();
        let mut level = Level::new();
        level.ensure(0, 0, 0);
        for (x, y, ter, furn) in &window {
            let terrain = terrain_codes.get(ter).copied().unwrap_or(0);
            let furniture = furniture_codes.get(furn).copied().unwrap_or(0);
            level.set_terrain(x - ORIGIN.0, y - ORIGIN.1, terrain, terrain);
            level.set_furniture(x - ORIGIN.0, y - ORIGIN.1, furniture, furniture);
        }
        world.insert_resource(Map(level));
        cdda_sim::set_prototypes(&mut world, Prototypes {
            terrain: prototypes_table.terrain.clone(),
            furniture: prototypes_table.furniture.clone(),
        });
        set_field_table(&mut world, table.clone());
        for (x, y, kind, intensity) in &turn.fields {
            let code = field_codes.get(kind).copied().unwrap_or(0);
            add_field_strength(
                &mut world,
                (x - ORIGIN.0, y - ORIGIN.1, 0),
                FieldId(code),
                *intensity,
            );
        }
        cast_lights_with(&mut world);
        let light = light(&world).clone();
        for square in &turn.squares {
            let (lx, ly) = (square.x - ORIGIN.0, square.y - ORIGIN.1);
            // The port's own light for the square: its cast, or the sky it was given, whichever is brighter.
            let cast = light.max_at(lx, ly);
            let brightest = cast.max(square.sky);
            let port_source = turn
                .fields
                .iter()
                .filter(|(x, y, _, _)| (*x, *y) == (square.x, square.y))
                .map(|(_, _, kind, intensity)| {
                    let code = field_codes.get(kind).copied().unwrap_or(0);
                    table.light(code, *intensity)
                })
                .fold(0.0_f32, f32::max);
            let answer = light_at(port_source, brightest);
            let reference = match square.reference_lit {
                0 => LitLevel::Dark,
                1 => LitLevel::Low,
                3 => LitLevel::Lit,
                4 => LitLevel::Bright,
                other => panic!("no such lit level: {other}"),
            };
            // **Both halves now.** The oracle fix (round 555) made the `lit` column agree with its own `source` column, so the answer is compared as well as the emitter:
            // a port that placed the light correctly and classified it differently would pass the source check alone.
            if (answer != reference || (port_source - square.reference_source).abs() > 0.5)
                && disagreements.len() < 12
            {
                {
                    disagreements.push(format!(
                        "turn {index} ({}, {}): ours {} source {port_source} cast {cast}, theirs {} source {} (sky {})",
                        square.x,
                        square.y,
                        lit_name(answer),
                        square.reference_source,
                        lit_name(reference),
                        square.sky
                    ));
                }
            }
            compared += 1;
        }
    }
    assert_eq!(compared, 7 * 25, "seven turns of a twenty-five-square level");
    assert!(disagreements.is_empty(), "the port's light disagrees: {disagreements:#?}");
}
