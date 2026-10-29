//! The reference's own frame, as the gate will compare against it.
//!
//! Source: `--dump-frame` from the instrumented reference (`harness.cpp`), which walks the loaded map around the avatar and writes, per square, the glyph the game would print,
//! the terrain and furniture ids behind it, and the light level it computed. That is the reference half of the semantic frame gate: the port's `Frame` produces the other half.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_json::{Value, parse};
use std::path::Path;

fn squares() -> Vec<(i32, i32, char, f64)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/frames/reference_frame.json");
    let text = std::fs::read_to_string(path).expect("the reference frame exists");
    let value = parse(text.as_bytes()).expect("the frame is valid JSON");
    let Value::Object(root) = value else {
        panic!("the frame's root is not an object");
    };
    let Some(Value::Array(entries)) = root.get("squares") else {
        panic!("the frame has no squares");
    };
    entries
        .iter()
        .map(|entry| {
            let Value::Object(square) = entry else {
                panic!("a square is not an object");
            };
            let number = |key: &str| match square.get(key) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("a coordinate fits"),
                _ => panic!("a square has no {key}"),
            };
            let light = match square.get("light") {
                Some(Value::Float(value)) => *value,
                Some(Value::Int(value)) => f64::from(i32::try_from(*value).expect("small")),
                _ => panic!("a square has no light"),
            };
            let glyph = match square.get("glyph") {
                Some(Value::Str(text)) => text.chars().next().expect("a glyph is a character"),
                _ => panic!("a square has no glyph"),
            };
            (number("x"), number("y"), glyph, light)
        })
        .collect()
}

#[test]
fn the_reference_frame_is_a_square_of_squares_around_the_avatar() {
    // A radius of twenty is forty-one squares across, so forty-one squared: 1,681. The count is the first thing a comparison needs, because a dump that lost a row would
    // otherwise look like a different map rather than a broken dump.
    let squares = squares();
    assert_eq!(squares.len(), 1681, "forty-one by forty-one");
}

#[test]
fn every_square_says_what_is_there_and_how_lit_it_is() {
    // Both halves the gate compares: a glyph to draw, and the light the reference computed for it. A square with no light in the dump would compare equal to a dark square in the
    // port and hide a light that never got computed, so the dump's own light range is pinned: daylight over the trees.
    let squares = squares();
    assert!(squares.iter().all(|(_, _, glyph, _)| *glyph != '\0'));
    let brightest = squares
        .iter()
        .map(|(_, _, _, light)| *light)
        .fold(0.0_f64, f64::max);
    assert!(
        brightest > 50.0,
        "open daylight is bright in the reference's own numbers: {brightest}"
    );
}

#[test]
fn most_glyphs_are_the_terrain_s_symbol_and_the_rest_are_something_drawn_on_top() {
    // A gate needs to know **what its own glyph column means** before comparing it to anything. Of the 1,681 squares, 1,638 print exactly the symbol the terrain prototype carries,
    // and the other 43 print something else: a floor with `6` or `)` on it is a floor with an item, and a forest floor with `f` or `*` is ground with a creature or a field standing
    // on it. So the reference's glyph is the **drawn** glyph, not the terrain's, and the port can only be compared square by square where nothing is drawn on top - until items and
    // creatures exist.
    //
    // The count is pinned rather than the ids: a loader that dropped symbols, or a draw rule that stopped covering something, would move this number.
    use cdda_json::{Value, parse};
    use std::collections::BTreeMap;
    use std::path::Path;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let terrain_text = std::fs::read_to_string(root.join("prototypes/terrain.json")).expect("the terrain dump");
    let Value::Object(terrain_root) = parse(terrain_text.as_bytes()).expect("valid JSON") else {
        panic!("the terrain dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = terrain_root.get("prototypes") else {
        panic!("no prototypes");
    };
    let mut symbols: BTreeMap<&str, char> = BTreeMap::new();
    for entry in prototypes {
        let Value::Object(object) = entry else { continue };
        let (Some(Value::Str(id)), Some(Value::Array(bytes))) = (object.get("id"), object.get("symbol"))
        else {
            continue;
        };
        let decoded: Vec<u8> = bytes
            .iter()
            .filter_map(|byte| match byte {
                Value::Int(value) => u8::try_from(*value).ok(),
                _ => None,
            })
            .collect();
        symbols.insert(id.as_str(), String::from_utf8_lossy(&decoded).chars().next().unwrap_or(' '));
    }

    let frame_text = std::fs::read_to_string(root.join("frames/reference_frame.json")).expect("the frame");
    let Value::Object(frame_root) = parse(frame_text.as_bytes()).expect("valid JSON") else {
        panic!("the frame's root is not an object");
    };
    let Some(Value::Array(squares)) = frame_root.get("squares") else {
        panic!("no squares");
    };
    let mut plain = 0;
    let mut overlaid = 0;
    let mut unknown = 0;
    for entry in squares {
        let Value::Object(square) = entry else { continue };
        let (Some(Value::Str(terrain_id)), Some(Value::Str(glyph))) =
            (square.get("ter"), square.get("glyph"))
        else {
            continue;
        };
        match symbols.get(terrain_id.as_str()) {
            None => unknown += 1,
            Some(symbol) if symbol.to_string() == *glyph => plain += 1,
            Some(_) => overlaid += 1,
        }
    }
    assert_eq!(unknown, 0, "every terrain in the frame is in the terrain dump");
    assert_eq!(plain, 1638, "squares printing their terrain's own symbol");
    assert_eq!(overlaid, 43, "squares with something drawn on top");
}

#[test]
#[allow(clippy::too_many_lines)]
fn the_port_prints_every_plain_square_the_way_the_reference_prints_it() {
    // The join: the port's own draw path, given the reference's terrain for each square, must print the reference's glyph - over the 1,638 squares where nothing is drawn on top.
    // The 43 that carry an item, creature or field are named out of the comparison rather than tolerated, because the port has none of those yet.
    use bevy_ecs::prelude::*;
    use cdda_core_types::level::Level;
    use cdda_sim::frame::frame;
    use cdda_sim::terrain_table::TerrainTable;
    use cdda_sim::Prototypes;
    use cdda_json::{Value, parse};
    use std::collections::BTreeMap;
    use std::path::Path;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let read = |name: &str| std::fs::read_to_string(root.join(name)).expect("a corpus file");

    // The terrain prototypes: id to code, code to symbol and transparency.
    let Value::Object(terrain_root) = parse(read("prototypes/terrain.json").as_bytes()).expect("valid") else {
        panic!("root is not an object");
    };
    let Some(Value::Array(prototypes)) = terrain_root.get("prototypes") else {
        panic!("no prototypes");
    };
    let mut codes: BTreeMap<String, u32> = BTreeMap::new();
    let mut terrain = TerrainTable::new();
    for entry in prototypes {
        let Value::Object(object) = entry else { continue };
        let Some(Value::Str(id)) = object.get("id") else { continue };
        let Some(Value::Array(bytes)) = object.get("symbol") else { continue };
        let decoded: Vec<u8> = bytes
            .iter()
            .filter_map(|byte| match byte {
                Value::Int(value) => u8::try_from(*value).ok(),
                _ => None,
            })
            .collect();
        let symbol = String::from_utf8_lossy(&decoded).chars().next().unwrap_or(' ');
        #[allow(clippy::cast_possible_truncation)]
        let code = u32::try_from(codes.len()).expect("few") + 1;
        codes.insert(id.clone(), code);
        terrain.insert(code, true);
        terrain.insert_symbol(code, symbol);
    }

    // The reference's squares, as a level and the glyphs to compare against.
    let Value::Object(frame_root) = parse(read("frames/reference_frame.json").as_bytes()).expect("valid") else {
        panic!("root is not an object");
    };
    let radius = match frame_root.get("radius") {
        Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
        _ => panic!("no radius"),
    };
    let centre = match frame_root.get("centre") {
        Some(Value::Array(pair)) => (
            match pair.first() {
                Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
                _ => panic!("no x"),
            },
            match pair.get(1) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
                _ => panic!("no y"),
            },
        ),
        _ => panic!("no centre"),
    };
    let Some(Value::Array(squares)) = frame_root.get("squares") else {
        panic!("no squares");
    };
    let mut level = Level::new();
    level.ensure(centre.0 - radius, centre.1 - radius, 1);
    let mut expected: BTreeMap<(i32, i32), char> = BTreeMap::new();
    for entry in squares {
        let Value::Object(square) = entry else { continue };
        let (Some(Value::Int(x)), Some(Value::Int(y))) = (square.get("x"), square.get("y")) else {
            continue;
        };
        let (Some(Value::Str(terrain_id)), Some(Value::Str(glyph))) = (square.get("ter"), square.get("glyph"))
        else {
            continue;
        };
        let (Ok(x), Ok(y)) = (i32::try_from(*x), i32::try_from(*y)) else { continue };
        let code = codes[terrain_id.as_str()];
        level.set_terrain(x, y, code, code);
        // Only the plain squares are the port's to reproduce: the others carry something drawn on top.
        if terrain.symbol(code) == glyph.chars().next().unwrap_or(' ') {
            expected.insert((x, y), glyph.chars().next().unwrap_or(' '));
        }
    }

    let mut world = World::new();
    cdda_sim::load_level(&mut world, level);
    cdda_sim::set_prototypes(&mut world, Prototypes {
        terrain,
        furniture: TerrainTable::new(),
    });
    // **The frame is drawn in daylight, which it has to be.** The reference recorded this frame outdoors in the middle of a day: its own light column reads the sky's
    // hundred-odd lumens on every open square. A port that drew it in the dark would be comparing its own blank frames with the reference's glyphs - and, since a dark square
    // is exactly the square the draw path takes from memory, it would also be silently measuring the memory path instead of the terrain's.
    // **The light is the recording's own, planted square by square.** The reference's frame sits inside a reality bubble and its light column reads the sky's hundred-odd
    // lumens on every open square; a level built from the recorded window alone has edges the port's sun cannot reach, because `build_sunlight_cache` lights a square from
    // its neighbours. Planting the recorded value as a light source on each square gives the draw loop exactly the light the reference had, which is the only input this gate
    // is about - the glyphs. The sun's own cascade is gated in `sunlight_system`.
    {
        for entry in squares {
            let Value::Object(square) = entry else { continue };
            let (Some(Value::Int(x)), Some(Value::Int(y))) = (square.get("x"), square.get("y")) else {
                continue;
            };
            let Some(Value::Float(light)) = square.get("light") else {
                continue;
            };
            let (Ok(x), Ok(y)) = (i32::try_from(*x), i32::try_from(*y)) else { continue };
            #[allow(clippy::cast_possible_truncation)]
            world.spawn((
                cdda_sim::Position { x, y, z: 0 },
                cdda_sim::LightSource { luminance: *light as f32 },
            ));
        }
        cdda_sim::cast_lights_with(&mut world);
    }
    let drawn = frame(&mut world, centre, radius, 100.0, 3.0);
    let text = drawn.text();
    let rows: Vec<&str> = text.lines().collect();
    let mut checked = 0;
    for ((x, y), glyph) in &expected {
        let row = usize::try_from(y - centre.1 + radius).expect("in range");
        let column = usize::try_from(x - centre.0 + radius).expect("in range");
        let Some(line) = rows.get(row) else { continue };
        assert_eq!(
            line.chars().nth(column),
            Some(*glyph),
            "square ({x}, {y}) in the reference's frame, cell {:?}:\n{text}",
            drawn.at(*x, *y)
        );
        checked += 1;
    }
    assert_eq!(checked, 1638, "the plain squares, all of them");
}
